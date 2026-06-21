use std::any::Any;
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;

use anyhow::{anyhow, Context, Result};

use crate::html::HtmlSyntaxTheme;
use crate::typst::paths::project_relative_path;
use crate::typst::preprocess::{
    execute_preprocess_plan_with_chunk_progress, prepare_preprocess_plan, preprocess_cached_output,
    preprocess_plan_cache_hit, preprocess_plan_chunk_count, PreprocessOptions, PreprocessOutput,
    PreprocessPlan,
};
use crate::utils::progress::{Progress, ProgressManager};

fn pluralize<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 {
        singular
    } else {
        plural
    }
}

fn chunk_count_label(count: usize) -> String {
    if count == 0 {
        "no chunks".to_string()
    } else {
        format!("{count} {}", pluralize(count, "chunk", "chunks"))
    }
}

fn no_chunk_pages_label(count: usize) -> String {
    if count == 1 {
        "1 page without chunks".to_string()
    } else {
        format!("{count} pages without chunks")
    }
}

fn panic_payload_to_string(panic: &(dyn Any + Send)) -> String {
    if let Some(message) = panic.downcast_ref::<&str>() {
        message.to_string()
    } else if let Some(message) = panic.downcast_ref::<String>() {
        message.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// Runs `task` over `items` on a small worker pool, failing on the first
/// error. Results are returned in completion order.
pub(super) fn run_parallel<I: Send, T: Send>(
    items: Vec<I>,
    parallelism: Option<usize>,
    progress: Option<&Progress>,
    task: impl Fn(I) -> Result<T> + Sync,
) -> Result<Vec<T>> {
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let worker_count = parallelism
        .unwrap_or_else(|| {
            thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1)
                .min(32)
        })
        .max(1)
        .min(items.len());
    let queue = Mutex::new(VecDeque::from(items));
    let results = Mutex::new(Vec::new());
    let abort = AtomicBool::new(false);

    thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..worker_count {
            handles.push(scope.spawn(|| -> Result<()> {
                loop {
                    if abort.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    let Some(item) = ({
                        let mut queue = queue.lock().unwrap();
                        if abort.load(Ordering::Relaxed) {
                            return Ok(());
                        }
                        queue.pop_front()
                    }) else {
                        return Ok(());
                    };
                    match task(item) {
                        Ok(value) => {
                            if let Some(progress) = progress {
                                progress.inc(1);
                            }
                            results.lock().unwrap().push(value);
                        }
                        Err(error) => {
                            abort.store(true, Ordering::Relaxed);
                            queue.lock().unwrap().clear();
                            return Err(error);
                        }
                    }
                }
            }));
        }

        for handle in handles {
            match handle.join() {
                Ok(result) => result?,
                Err(error) => {
                    return Err(anyhow!(
                        "website build worker panicked: {}",
                        panic_payload_to_string(&*error),
                    ));
                }
            }
        }
        Ok(())
    })?;
    Ok(results.into_inner().unwrap())
}

pub(super) struct WebsitePreprocessOptions<'a> {
    pub(super) typ_files: &'a [PathBuf],
    pub(super) src_dir: &'a Path,
    pub(super) config_path: &'a Path,
    pub(super) quiet: bool,
    pub(super) timeout: Option<u64>,
    pub(super) params: &'a [String],
    pub(super) fallback_theme: crate::theme::ThemeSelection,
    pub(super) html_syntax_theme: HtmlSyntaxTheme,
    pub(super) asset_dir: &'a Path,
    pub(super) parallelism: Option<usize>,
    pub(super) progress: ProgressManager,
}

enum WebsitePreprocessWork {
    Cached(PreprocessOutput),
    Pending(PreprocessPlan),
}

pub(super) fn preprocess_documents(
    options: WebsitePreprocessOptions<'_>,
) -> Result<BTreeMap<PathBuf, PreprocessOutput>> {
    let display_root =
        fs::canonicalize(options.src_dir).unwrap_or_else(|_| options.src_dir.to_path_buf());
    let scan_progress = options
        .progress
        .bar("[scan] pages", options.typ_files.len() as u64);
    let planned = run_parallel(
        options.typ_files.to_vec(),
        options.parallelism,
        Some(&scan_progress),
        |input| {
            let rel = project_relative_path(&display_root, &input);
            let page_progress = options.progress.spinner(format!("[scan] {rel}"));
            let plan = prepare_preprocess_plan(PreprocessOptions {
                input: input.to_path_buf(),
                root: Some(options.src_dir.to_path_buf()),
                config: Some(options.config_path.to_path_buf()),
                display_root: Some(display_root.clone()),
                quiet: options.quiet,
                status: false,
                progress: false,
                timeout: options.timeout,
                sync_pages: false,
                theme: None,
                fallback_theme: options.fallback_theme.clone(),
                html_syntax_theme: Some(options.html_syntax_theme.clone()),
                asset_dir: Some(options.asset_dir.to_path_buf()),
                param_overrides: options.params.to_vec(),
            })
            .with_context(|| format!("failed to scan {}", input.display()))?;
            let work = if preprocess_plan_cache_hit(&plan)? {
                page_progress.finish(format!("[cache] scan {rel}"));
                WebsitePreprocessWork::Cached(preprocess_cached_output(plan))
            } else {
                let chunk_count = preprocess_plan_chunk_count(&plan);
                let chunk_label = chunk_count_label(chunk_count);
                page_progress.finish(format!("[ready] run {rel}: {chunk_label}"));
                WebsitePreprocessWork::Pending(plan)
            };
            Ok((input, work))
        },
    )?;
    scan_progress.finish("[done] scan pages");

    let mut outputs = BTreeMap::new();
    let mut pending = Vec::new();
    let mut run_chunk_count = 0usize;
    let mut run_no_chunk_count = 0usize;
    for (input, work) in planned {
        match work {
            WebsitePreprocessWork::Cached(output) => {
                outputs.insert(input, output);
            }
            WebsitePreprocessWork::Pending(plan) => {
                let chunk_count = preprocess_plan_chunk_count(&plan);
                run_chunk_count += chunk_count;
                if chunk_count == 0 {
                    run_no_chunk_count += 1;
                }
                pending.push((input, plan));
            }
        }
    }

    if !pending.is_empty() {
        let pending_count = pending.len();
        let run_unit_count = (run_chunk_count + run_no_chunk_count) as u64;
        let run_label = if run_chunk_count == 0 {
            format!("[run] {}", no_chunk_pages_label(pending_count))
        } else if run_no_chunk_count == 0 {
            format!("[run] {}", chunk_count_label(run_chunk_count))
        } else {
            format!(
                "[run] {} and {}",
                chunk_count_label(run_chunk_count),
                no_chunk_pages_label(run_no_chunk_count),
            )
        };
        let run_progress = options.progress.bar(run_label, run_unit_count);
        let run_outputs = run_parallel(
            pending,
            options.parallelism,
            (run_chunk_count == 0).then_some(&run_progress),
            |(input, plan)| {
                let rel = project_relative_path(&display_root, &input);
                let page_progress = options.progress.spinner(format!("[run] {rel}"));
                let chunk_count = preprocess_plan_chunk_count(&plan);
                let output = execute_preprocess_plan_with_chunk_progress(
                    plan,
                    (run_chunk_count > 0).then_some(&run_progress),
                )
                .with_context(|| format!("failed to run chunks for {}", input.display()))?;
                if run_chunk_count > 0 && chunk_count == 0 {
                    run_progress.inc(1);
                }
                page_progress.finish(format!("[done] run {rel}"));
                Ok((input, output))
            },
        )?;
        let finish_label = if run_chunk_count == 0 {
            format!("[done] {}", no_chunk_pages_label(pending_count))
        } else if run_no_chunk_count == 0 {
            format!("[done] {}", chunk_count_label(run_chunk_count))
        } else {
            format!(
                "[done] {} and {}",
                chunk_count_label(run_chunk_count),
                no_chunk_pages_label(run_no_chunk_count),
            )
        };
        run_progress.finish(finish_label);
        outputs.extend(run_outputs);
    }

    Ok(outputs)
}
