use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crate::typst::cache::CacheState;
use crate::typst::execute::{EnginePool, ExecutionConfig};
use crate::typst::model::{ChunkSpec, LayoutPaths};
use crate::typst::paths::{artifact_reference, resolve_layout};
use crate::typst::query::{parse_chunks, parse_setup_defaults};
use crate::typst::results::{build_results_document, write_results};
use crate::typst::runtime::write_runtime;

#[derive(Debug, Clone)]
pub struct PreprocessOptions {
    pub input: PathBuf,
    pub root: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub results: Option<PathBuf>,
    pub cache_override: Option<bool>,
    pub execute_override: Option<bool>,
    pub clean: bool,
    pub quiet: bool,
    pub typst: PathBuf,
    pub rscript: PathBuf,
    pub python: PathBuf,
    pub shell: PathBuf,
    pub timeout: Option<u64>,
}

#[derive(Debug)]
pub struct PreprocessOutput {
    pub layout: LayoutPaths,
}

pub fn preprocess(options: PreprocessOptions) -> Result<PreprocessOutput> {
    let layout = resolve_layout(
        &options.input,
        options.root.as_deref(),
        options.results.as_deref(),
    )?;

    if options.clean {
        clean_outputs(&layout)?;
    }

    write_runtime(&layout.root)?;
    let results_input = artifact_reference(&layout.root, &layout.results_path);
    let setup_json = typst_query(&options.typst, &layout, "<calepin-config>", &results_input)?;
    let defaults = parse_setup_defaults(&setup_json)?.unwrap_or_default();
    let chunks_json = typst_query(&options.typst, &layout, "<calepin-chunk>", &results_input)?;
    let mut chunks = parse_chunks(&chunks_json, Some(defaults))?;
    apply_cli_overrides(&mut chunks, options.cache_override, options.execute_override);

    std::fs::create_dir_all(&layout.figures_dir)
        .with_context(|| format!("failed to create {}", layout.figures_dir.display()))?;
    let execution_config = ExecutionConfig {
        cwd: options.cwd.unwrap_or_else(|| layout.work_dir.clone()),
        rscript: options.rscript,
        python: options.python,
        shell: options.shell,
        timeout: options.timeout.map(Duration::from_secs),
    };
    let mut pool = EnginePool::new(execution_config);
    let mut cache = CacheState::new(layout.cache_dir.clone(), true);
    let mut chunk_results = Vec::with_capacity(chunks.len());

    for chunk in &chunks {
        let result = cache.lookup_or_execute(chunk, &layout.root, || {
            pool.execute_chunk(chunk, &layout.figures_dir, |path| {
                artifact_reference(&layout.root, path)
            })
        })?;
        chunk_results.push(result);
    }

    let document = build_results_document(&layout.input_rel, chunk_results);
    write_results(&layout.results_path, &document)?;

    if !options.quiet {
        eprintln!(
            "preprocessed {} chunk{} -> {}",
            chunks.len(),
            if chunks.len() == 1 { "" } else { "s" },
            layout.results_path.display()
        );
    }

    Ok(PreprocessOutput { layout })
}

pub fn typst_query(typst: &PathBuf, layout: &LayoutPaths, selector: &str, results_input: &str) -> Result<String> {
    let output = Command::new(typst)
        .arg("query")
        .arg(&layout.input_rel)
        .arg(selector)
        .arg("--root")
        .arg(&layout.root)
        .arg("--input")
        .arg("calepin-mode=query")
        .arg("--input")
        .arg(format!("calepin-results={results_input}"))
        .current_dir(&layout.root)
        .output()
        .with_context(|| format!("failed to run {}", typst.display()))?;

    if !output.status.success() {
        return Err(anyhow!(
            "typst query {} failed:\n{}",
            selector,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    String::from_utf8(output.stdout).context("typst query output was not UTF-8")
}

fn apply_cli_overrides(
    chunks: &mut [ChunkSpec],
    cache_override: Option<bool>,
    execute_override: Option<bool>,
) {
    for chunk in chunks {
        if let Some(cache) = cache_override {
            chunk.exec_options.cache = cache;
        }
        if let Some(execute) = execute_override {
            chunk.exec_options.eval = execute;
        }
    }
}

fn clean_outputs(layout: &LayoutPaths) -> Result<()> {
    if layout.results_path.exists() {
        std::fs::remove_file(&layout.results_path)
            .with_context(|| format!("failed to remove {}", layout.results_path.display()))?;
    }
    if layout.figures_dir.exists() {
        std::fs::remove_dir_all(&layout.figures_dir)
            .with_context(|| format!("failed to remove {}", layout.figures_dir.display()))?;
    }
    Ok(())
}

pub fn reject_reserved_typst_inputs(args: &[String]) -> Result<()> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if let Some(value) = arg.strip_prefix("--input=") {
            reject_reserved_input_value(value)?;
        } else if arg == "--input" {
            let Some(value) = iter.next() else {
                return Err(anyhow!("`--input` in forwarded Typst args requires a value"));
            };
            reject_reserved_input_value(value)?;
        }
    }
    Ok(())
}

fn reject_reserved_input_value(value: &str) -> Result<()> {
    if value.starts_with("calepin-mode=") || value.starts_with("calepin-results=") {
        return Err(anyhow!(
            "forwarded Typst args may not override reserved Calepin input `{}`",
            value
        ));
    }
    Ok(())
}

pub fn compile_with_typst(
    typst: &PathBuf,
    layout: &LayoutPaths,
    output: Option<PathBuf>,
    typst_args: &[String],
) -> Result<()> {
    reject_reserved_typst_inputs(typst_args)?;
    let results_input = artifact_reference(&layout.root, &layout.results_path);
    let mut command = Command::new(typst);
    command
        .arg("compile")
        .arg("--root")
        .arg(&layout.root)
        .arg("--input")
        .arg("calepin-mode=render")
        .arg("--input")
        .arg(format!("calepin-results={results_input}"))
        .arg(&layout.input_rel);
    if let Some(output) = output {
        command.arg(output);
    }
    command.args(typst_args).current_dir(&layout.root);
    let output = command
        .output()
        .with_context(|| format!("failed to run {}", typst.display()))?;
    if !output.status.success() {
        return Err(anyhow!(
            "typst compile failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_reserved_forwarded_inputs() {
        let err = reject_reserved_typst_inputs(&[
            "--input".to_string(),
            "calepin-mode=query".to_string(),
        ])
        .unwrap_err()
        .to_string();
        assert!(err.contains("reserved Calepin input"));

        let err = reject_reserved_typst_inputs(&[
            "--input=calepin-results=.calepin/paper/results.json".to_string(),
        ])
        .unwrap_err()
        .to_string();
        assert!(err.contains("reserved Calepin input"));
    }

    #[test]
    fn accepts_unrelated_forwarded_inputs() {
        reject_reserved_typst_inputs(&[
            "--input".to_string(),
            "theme=dark".to_string(),
            "--font-path".to_string(),
            "fonts".to_string(),
        ])
        .unwrap();
    }

    #[test]
    fn query_command_uses_root_relative_input() {
        use crate::typst::paths::slash_path;

        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path()), None).unwrap();
        assert_eq!(slash_path(&layout.input_rel), "paper.typ");
    }
}
