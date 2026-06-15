mod fingerprint;
mod staging;

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use xxhash_rust::xxh3::xxh3_64;

use crate::config::{CalepinConfig, ExecutablePaths};
use crate::typst::execute::{EnginePool, ExecutionConfig};
use crate::typst::introspect::preprocess_metadata;
use crate::typst::io::{ensure_parent, write_if_changed};
use crate::typst::model::{ChunkResultDocument, ChunkSpec, EngineName, LayoutPaths};
use crate::typst::paths::{artifact_reference, project_relative_path, resolve_layout};
use crate::typst::query::{parse_chunks_with_warnings, parse_setup_config};
use crate::typst::results::{build_results_document, write_results};
use crate::typst::runtime::write_runtime;
use crate::typst::source_rewrite::write_staged_source;
use crate::typst::sync::write_page_sync;
use crate::typst::version::assert_supported_typst;

const PAGE_META_FILE: &str = "page-meta.json";

use fingerprint::{preprocess_cache_hit, preprocess_fingerprint, write_preprocess_fingerprint};
use staging::{
    paged_template_context, write_query_source, write_render_wrapper, write_shared_typst_files,
};

#[derive(Debug, Clone)]
pub struct PreprocessOptions {
    pub input: PathBuf,
    pub root: Option<PathBuf>,
    pub config: Option<PathBuf>,
    pub display_root: Option<PathBuf>,
    pub quiet: bool,
    pub timeout: Option<u64>,
    pub sync_pages: bool,
    /// CLI-level theme selection. `None` allows document setup and then
    /// `fallback_theme` to decide.
    pub theme: Option<crate::theme::ThemeSelection>,
    pub fallback_theme: crate::theme::ThemeSelection,
    /// `key=value` document-parameter overrides from the CLI (`-P`).
    pub param_overrides: Vec<String>,
}

#[derive(Debug)]
pub struct PreprocessOutput {
    pub layout: LayoutPaths,
    pub executables: ExecutablePaths,
    pub fingerprint: u64,
    pub theme: crate::theme::ThemeSelection,
}

#[derive(Debug)]
pub struct PreprocessPlan {
    pub layout: LayoutPaths,
    pub executables: ExecutablePaths,
    pub fingerprint: u64,
    chunks: Vec<ChunkSpec>,
    cwd: PathBuf,
    timeout: Option<Duration>,
    quiet: bool,
    sync_pages: bool,
    display_root: Option<PathBuf>,
    params: serde_json::Value,
    theme: crate::theme::ThemeSelection,
}

pub fn preprocess(options: PreprocessOptions) -> Result<PreprocessOutput> {
    let plan = prepare_preprocess_plan(options)?;
    execute_preprocess_plan(plan)
}

pub fn preprocess_cached(options: PreprocessOptions) -> Result<PreprocessOutput> {
    let plan = prepare_preprocess_plan(options)?;
    if preprocess_cache_hit(&plan.layout, plan.fingerprint)? {
        if !plan.quiet {
            eprintln!("calepin [cache] {}", display_input(&plan));
        }
        return Ok(PreprocessOutput {
            layout: plan.layout,
            executables: plan.executables,
            fingerprint: plan.fingerprint,
            theme: plan.theme,
        });
    }
    execute_preprocess_plan(plan)
}

pub fn prepare_preprocess_plan(options: PreprocessOptions) -> Result<PreprocessPlan> {
    let mut layout = resolve_layout(&options.input, options.root.as_deref())?;
    let config = CalepinConfig::load(&layout.root, options.config.as_deref())?;
    assert_supported_typst(&config.executables.typst)?;

    write_runtime(&layout.root)?;
    write_shared_typst_files(&layout.root)?;
    let staged_input = write_staged_source(&layout)?;
    // Metadata collection runs before the final target is known. Use a
    // query-only source so documents can contain `html.*` calls without hiding
    // chunks from Calepin's query pass.
    let query_source = write_query_source(&layout, &staged_input)?;
    let query_input = write_render_wrapper(&layout, &query_source, &[], None)?;
    let results_input = artifact_reference(&layout.root, &layout.results_path);
    let metadata = preprocess_metadata(
        &config.executables.typst,
        &layout,
        &query_input,
        &results_input,
    )?;
    write_page_meta(&layout, metadata.page_meta.as_ref())?;
    let setup_config = parse_setup_config(&metadata.setup_json)?;
    let setup_config = setup_config.unwrap_or_default();
    let parsed_chunks =
        parse_chunks_with_warnings(&metadata.chunks_json, Some(setup_config.clone()))?;
    let chunks = parsed_chunks.chunks;
    if !options.quiet {
        for warning in parsed_chunks.warnings {
            cwarn!("{}", warning);
        }
    }

    // Collect unique Jupyter kernel names so the render wrapper gets their show rules.
    let jupyter_kernels: std::collections::BTreeSet<&str> = chunks
        .iter()
        .filter_map(|c| {
            if let EngineName::Jupyter(k) = &c.engine {
                Some(k.as_str())
            } else {
                None
            }
        })
        .collect();
    let params = resolve_params(&setup_config.defaults.params, &options.param_overrides)?;

    let effective_theme = options
        .theme
        .clone()
        .or(setup_config.defaults.theme_selection(&layout.root)?)
        .unwrap_or_else(|| options.fallback_theme.clone());
    let paged_context = paged_template_context(
        &layout,
        &staged_input,
        metadata.page_meta.clone(),
        params.clone(),
    );
    let paged_theme = crate::theme::paged_source(&effective_theme, &paged_context)?;
    if !jupyter_kernels.is_empty() {
        let kernels: Vec<&str> = jupyter_kernels.into_iter().collect();
        layout.render_input =
            write_render_wrapper(&layout, &staged_input, &kernels, paged_theme.as_ref())?;
    } else {
        layout.render_input =
            write_render_wrapper(&layout, &staged_input, &[], paged_theme.as_ref())?;
    }

    let cwd = layout.work_dir.clone();
    let timeout = options.timeout.map(Duration::from_secs);
    let fingerprint = preprocess_fingerprint(
        &layout,
        &config.executables,
        &chunks,
        &cwd,
        timeout,
        &params,
        &effective_theme,
    )?;

    Ok(PreprocessPlan {
        layout,
        executables: config.executables,
        fingerprint,
        chunks,
        cwd,
        timeout,
        quiet: options.quiet,
        sync_pages: options.sync_pages,
        display_root: options.display_root,
        params,
        theme: effective_theme,
    })
}

pub fn execute_preprocess_plan(plan: PreprocessPlan) -> Result<PreprocessOutput> {
    let staged = tempfile::Builder::new()
        .prefix("calepin-figures-")
        .tempdir()
        .context("failed to create temporary figures directory")?;
    let staged_figures_dir = staged.path().join("figures");
    std::fs::create_dir_all(&staged_figures_dir)
        .with_context(|| format!("failed to create {}", staged_figures_dir.display()))?;

    // Always write params.json when there are parameters: it is the universal
    // transport for Jupyter kernels Calepin cannot auto-bind, and a useful
    // reproducibility record. Native R/Python read their literal prelude instead.
    let params_path = write_params_file(&plan.layout, &plan.params)?;

    let execution_config = ExecutionConfig {
        cwd: plan.cwd.clone(),
        executables: plan.executables.clone(),
        timeout: plan.timeout,
        params: plan.params.clone(),
        params_path,
    };
    let mut pool = EnginePool::new(execution_config);
    let mut chunk_results: Vec<Option<ChunkResultDocument>> = vec![None; plan.chunks.len()];

    if !plan.quiet {
        eprintln!(
            "calepin [run] {}: {} chunk{}",
            display_input(&plan),
            plan.chunks.len(),
            if plan.chunks.len() == 1 { "" } else { "s" },
        );
    }

    for chunk_index in chunks_in_engine_order(&plan.chunks) {
        let chunk = &plan.chunks[chunk_index];
        let result = execute_chunk_live(&mut pool, chunk, &staged_figures_dir, &plan.layout)?;
        chunk_results[chunk_index] = Some(result);
    }

    let chunk_results = chunk_results
        .into_iter()
        .map(|result| {
            result.context("chunk execution produced no result; this indicates a planner bug")
        })
        .collect::<Result<Vec<_>>>()?;

    publish_staged_figures(&staged_figures_dir, &plan.layout.figures_dir)?;
    let document = build_results_document(&plan.layout.input_rel, chunk_results);
    write_results(&plan.layout.results_path, &document)?;
    write_preprocess_fingerprint(&plan.layout, plan.fingerprint)?;
    if plan.sync_pages {
        if let Err(error) = write_page_sync(&plan.executables.typst, &plan.layout, &plan.chunks) {
            if !plan.quiet {
                cwarn!("page sync failed: {}", error);
            }
        }
    }

    Ok(PreprocessOutput {
        layout: plan.layout,
        executables: plan.executables,
        fingerprint: plan.fingerprint,
        theme: plan.theme,
    })
}

fn page_meta_path(layout: &LayoutPaths) -> PathBuf {
    layout.sibling_path(PAGE_META_FILE)
}

fn source_fingerprint(input: &Path) -> Result<String> {
    let bytes = fs::read(input).with_context(|| format!("failed to read {}", input.display()))?;
    Ok(format!("{:016x}", xxh3_64(&bytes)))
}

/// Persists the page's `<website-metadata>` value next to its results, tagged
/// with the source content hash so readers can detect staleness.
fn write_page_meta(layout: &LayoutPaths, value: Option<&serde_json::Value>) -> Result<()> {
    let document = serde_json::json!({
        "source_xxh3": source_fingerprint(&layout.input)?,
        "value": value,
    });
    let path = page_meta_path(layout);
    write_if_changed(&path, serde_json::to_string(&document)?)
}

/// Returns the `<website-metadata>` value persisted by the last preprocess of
/// `input`, or `None` when it is missing or stale for the current content.
pub fn read_page_meta_with_root(input: &Path, root: Option<&Path>) -> Option<serde_json::Value> {
    let layout = resolve_layout(input, root).ok()?;
    let contents = fs::read_to_string(page_meta_path(&layout)).ok()?;
    let document: serde_json::Value = serde_json::from_str(&contents).ok()?;
    let current = source_fingerprint(&layout.input).ok()?;
    if document
        .get("source_xxh3")
        .and_then(serde_json::Value::as_str)
        != Some(current.as_str())
    {
        return None;
    }
    document
        .get("value")
        .cloned()
        .filter(|value| !value.is_null())
}

fn execute_chunk_live(
    pool: &mut EnginePool,
    chunk: &ChunkSpec,
    execution_figures_dir: &Path,
    layout: &LayoutPaths,
) -> Result<ChunkResultDocument> {
    pool.execute_chunk(chunk, execution_figures_dir, |path| {
        execution_artifact_reference(
            &layout.root,
            execution_figures_dir,
            &layout.figures_dir,
            path,
        )
    })
}

fn chunks_in_engine_order(chunks: &[ChunkSpec]) -> Vec<usize> {
    let mut groups: Vec<(EngineName, Vec<usize>)> = Vec::new();

    for (index, chunk) in chunks.iter().enumerate() {
        if let Some((_, chunk_indexes)) = groups
            .iter_mut()
            .find(|(engine, _)| *engine == chunk.engine)
        {
            chunk_indexes.push(index);
            continue;
        }

        groups.push((chunk.engine.clone(), vec![index]));
    }

    groups
        .into_iter()
        .flat_map(|(_, chunk_indexes)| chunk_indexes)
        .collect()
}

fn execution_artifact_reference(
    root: &Path,
    execution_figures_dir: &Path,
    final_figures_dir: &Path,
    path: &Path,
) -> String {
    let final_path = path
        .strip_prefix(execution_figures_dir)
        .map(|relative| final_figures_dir.join(relative))
        .unwrap_or_else(|_| path.to_path_buf());
    artifact_reference(root, &final_path)
}

fn display_input(plan: &PreprocessPlan) -> String {
    display_input_path(&plan.layout, plan.display_root.as_deref())
}

fn display_input_path(layout: &LayoutPaths, display_root: Option<&Path>) -> String {
    project_relative_path(display_root.unwrap_or(&layout.root), &layout.input)
}

fn publish_staged_figures(staged: &Path, final_dir: &Path) -> Result<()> {
    if !staged.exists() {
        return Ok(());
    }

    for entry in
        std::fs::read_dir(staged).with_context(|| format!("failed to read {}", staged.display()))?
    {
        let entry = entry.with_context(|| format!("failed to read {}", staged.display()))?;
        let path = entry.path();
        let target = final_dir.join(entry.file_name());
        if entry
            .file_type()
            .with_context(|| format!("failed to stat {}", path.display()))?
            .is_dir()
        {
            publish_staged_figures(&path, &target)?;
        } else {
            publish_staged_file(&path, &target)?;
        }
    }

    Ok(())
}

fn publish_staged_file(source: &Path, target: &Path) -> Result<()> {
    let bytes = std::fs::read(source)
        .with_context(|| format!("failed to read staged figure {}", source.display()))?;
    write_if_changed(target, bytes)
}

/// Write `params.json` next to `results.json` when there are parameters, and
/// return its path. Returns `None` (and removes any stale file) when empty.
fn write_params_file(layout: &LayoutPaths, params: &serde_json::Value) -> Result<Option<PathBuf>> {
    let path = layout.sibling_path("params.json");
    let is_empty = params.as_object().is_none_or(|map| map.is_empty());
    if is_empty {
        let _ = fs::remove_file(&path);
        return Ok(None);
    }
    ensure_parent(&path)?;
    let json = serde_json::to_string_pretty(params)?;
    write_if_changed(&path, json)?;
    Ok(Some(path))
}

/// Merge `-P key=value` CLI overrides onto the document's `setup(params: ...)`.
/// CLI values win, and inherit the same scalar typing as `#|` option values.
fn resolve_params(base: &serde_json::Value, overrides: &[String]) -> Result<serde_json::Value> {
    let mut map = match base {
        serde_json::Value::Object(map) => map.clone(),
        _ => serde_json::Map::new(),
    };
    for entry in overrides {
        let (key, raw_value) = entry
            .split_once('=')
            .ok_or_else(|| anyhow!("invalid --param `{entry}` (expected `key=value`)"))?;
        let value = crate::typst::chunk_options::parse_qmd_value(raw_value.trim())?;
        map.insert(key.trim().to_string(), value);
    }
    Ok(serde_json::Value::Object(map))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::model::ResultsMode;
    use crate::typst::paths::slash_path;
    use crate::typst::testfixtures;

    #[test]
    fn page_meta_roundtrips_and_detects_stale_source() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("page.typ");
        fs::write(&input, "= Home\n").unwrap();
        let layout = resolve_layout(&input, None).unwrap();
        let value = serde_json::json!({"title": "Home", "pdf": false});

        write_page_meta(&layout, Some(&value)).unwrap();
        assert_eq!(read_page_meta_with_root(&input, None), Some(value));

        fs::write(&input, "= Changed\n").unwrap();
        assert_eq!(read_page_meta_with_root(&input, None), None);
    }

    #[test]
    fn page_meta_absent_when_document_exposes_none() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("page.typ");
        fs::write(&input, "= Home\n").unwrap();
        let layout = resolve_layout(&input, None).unwrap();

        write_page_meta(&layout, None).unwrap();

        assert_eq!(read_page_meta_with_root(&input, None), None);
    }

    #[test]
    fn cli_params_override_setup_params() {
        let base = serde_json::json!({"region": "NY", "min_count": 10});
        let resolved = resolve_params(
            &base,
            &[
                "region=CA".to_string(),
                "alpha=0.5".to_string(),
                "active=true".to_string(),
            ],
        )
        .unwrap();
        assert_eq!(
            resolved,
            serde_json::json!({"region":"CA","min_count":10,"alpha":0.5,"active":true})
        );
    }

    #[test]
    fn cli_param_without_equals_is_rejected() {
        let err = resolve_params(&serde_json::json!({}), &["bad".to_string()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("bad"), "{err}");
    }

    #[test]
    fn resolve_params_with_no_overrides_returns_base() {
        let base = serde_json::json!({"a": 1});
        assert_eq!(resolve_params(&base, &[]).unwrap(), base);
    }

    #[test]
    fn query_command_uses_root_relative_input() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path())).unwrap();
        assert_eq!(slash_path(&layout.input_rel), "paper.typ");
    }

    #[test]
    fn display_input_uses_explicit_display_root() {
        let dir = tempfile::tempdir().unwrap();
        let docs = dir.path().join("docs");
        let language_dir = docs.join("fr");
        let input = language_dir.join("index.typ");
        std::fs::create_dir_all(&language_dir).unwrap();
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(&language_dir)).unwrap();

        assert_eq!(display_input_path(&layout, None), "index.typ");
        let docs = std::fs::canonicalize(docs).unwrap();
        assert_eq!(display_input_path(&layout, Some(&docs)), "fr/index.typ");
    }

    #[test]
    fn preprocess_fingerprint_ignores_render_only_display_options() {
        let dir = tempfile::tempdir().unwrap();
        let layout = test_layout(dir.path());
        let executables = ExecutablePaths::defaults();
        let mut chunk = test_chunk("print(1)");

        let first = preprocess_fingerprint(
            &layout,
            &executables,
            std::slice::from_ref(&chunk),
            dir.path(),
            Some(Duration::from_secs(5)),
            &serde_json::json!({}),
            &crate::theme::ThemeSelection::Default,
        )
        .unwrap();

        chunk.display_options.echo = false;
        chunk.display_options.output = false;
        chunk.display_options.results = ResultsMode::Hide;
        chunk.display_options.fig_caption = Some("New caption".to_string());

        let second = preprocess_fingerprint(
            &layout,
            &executables,
            &[chunk],
            dir.path(),
            Some(Duration::from_secs(5)),
            &serde_json::json!({}),
            &crate::theme::ThemeSelection::Default,
        )
        .unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn preprocess_fingerprint_tracks_params() {
        let dir = tempfile::tempdir().unwrap();
        let layout = test_layout(dir.path());
        let executables = ExecutablePaths::defaults();
        let chunk = test_chunk("print(1)");
        let baseline = preprocess_fingerprint(
            &layout,
            &executables,
            std::slice::from_ref(&chunk),
            dir.path(),
            Some(Duration::from_secs(5)),
            &serde_json::json!({"region": "NY"}),
            &crate::theme::ThemeSelection::Default,
        )
        .unwrap();
        let changed = preprocess_fingerprint(
            &layout,
            &executables,
            &[chunk],
            dir.path(),
            Some(Duration::from_secs(5)),
            &serde_json::json!({"region": "CA"}),
            &crate::theme::ThemeSelection::Default,
        )
        .unwrap();
        assert_ne!(baseline, changed);
    }

    #[test]
    fn preprocess_fingerprint_tracks_execution_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let layout = test_layout(dir.path());
        let executables = ExecutablePaths::defaults();
        let chunk = test_chunk("print(1)");
        let baseline = preprocess_fingerprint(
            &layout,
            &executables,
            std::slice::from_ref(&chunk),
            dir.path(),
            Some(Duration::from_secs(5)),
            &serde_json::json!({}),
            &crate::theme::ThemeSelection::Default,
        )
        .unwrap();

        let code_changed = preprocess_fingerprint(
            &layout,
            &executables,
            &[test_chunk("print(2)")],
            dir.path(),
            Some(Duration::from_secs(5)),
            &serde_json::json!({}),
            &crate::theme::ThemeSelection::Default,
        )
        .unwrap();
        assert_ne!(baseline, code_changed);

        let mut exec_changed = chunk.clone();
        exec_changed.exec_options.fig_device_dpi = 300;
        let exec_changed = preprocess_fingerprint(
            &layout,
            &executables,
            &[exec_changed],
            dir.path(),
            Some(Duration::from_secs(5)),
            &serde_json::json!({}),
            &crate::theme::ThemeSelection::Default,
        )
        .unwrap();
        assert_ne!(baseline, exec_changed);

        let mut executables_changed = executables.clone();
        executables_changed.python = PathBuf::from("python-custom");
        let executables_changed = preprocess_fingerprint(
            &layout,
            &executables_changed,
            &[chunk],
            dir.path(),
            Some(Duration::from_secs(5)),
            &serde_json::json!({}),
            &crate::theme::ThemeSelection::Default,
        )
        .unwrap();
        assert_ne!(baseline, executables_changed);
    }

    #[test]
    fn preprocess_fingerprint_tracks_theme() {
        let dir = tempfile::tempdir().unwrap();
        let layout = test_layout(dir.path());
        let executables = ExecutablePaths::defaults();
        let chunk = test_chunk("print(1)");
        let baseline = preprocess_fingerprint(
            &layout,
            &executables,
            std::slice::from_ref(&chunk),
            dir.path(),
            Some(Duration::from_secs(5)),
            &serde_json::json!({}),
            &crate::theme::ThemeSelection::Default,
        )
        .unwrap();
        let changed = preprocess_fingerprint(
            &layout,
            &executables,
            &[chunk],
            dir.path(),
            Some(Duration::from_secs(5)),
            &serde_json::json!({}),
            &crate::theme::ThemeSelection::Disabled,
        )
        .unwrap();
        assert_ne!(baseline, changed);
    }

    #[test]
    fn chunks_are_executed_grouped_by_engine() {
        let mut first_r = test_chunk("x <- 1");
        first_r.label = "r-first".to_string();
        first_r.engine = EngineName::R;

        let mut first_py = test_chunk("print(1)");
        first_py.label = "py-first".to_string();
        first_py.engine = EngineName::Python;

        let mut second_r = test_chunk("x <- 2");
        second_r.label = "r-second".to_string();
        second_r.engine = EngineName::R;

        let mut second_py = test_chunk("print(2)");
        second_py.label = "py-second".to_string();
        second_py.engine = EngineName::Python;

        let chunks = vec![first_r, first_py, second_r, second_py];
        let grouped_indices = chunks_in_engine_order(&chunks);
        let labels: Vec<&str> = grouped_indices
            .iter()
            .map(|&index| chunks[index].label.as_str())
            .collect();

        assert_eq!(labels, vec!["r-first", "r-second", "py-first", "py-second"]);
    }

    #[test]
    fn execution_artifacts_reference_final_figures_dir() {
        let root = tempfile::tempdir().unwrap();
        let staged = tempfile::tempdir().unwrap();
        let final_figures_dir = root.path().join(".calepin/paper/figures");
        let staged_artifact = staged.path().join("answer.svg");

        assert_eq!(
            execution_artifact_reference(
                root.path(),
                staged.path(),
                &final_figures_dir,
                &staged_artifact,
            ),
            "/.calepin/paper/figures/answer.svg"
        );
    }

    #[test]
    fn publish_staged_figures_copies_into_final_dir() {
        let staged = tempfile::tempdir().unwrap();
        let final_dir = tempfile::tempdir().unwrap();
        let staged_figures = staged.path().join("figures");
        std::fs::create_dir_all(staged_figures.join("nested")).unwrap();
        std::fs::write(staged_figures.join("answer.svg"), "<svg>answer</svg>").unwrap();
        std::fs::write(
            staged_figures.join("nested/detail.svg"),
            "<svg>detail</svg>",
        )
        .unwrap();

        publish_staged_figures(&staged_figures, final_dir.path()).unwrap();

        assert_eq!(
            std::fs::read_to_string(final_dir.path().join("answer.svg")).unwrap(),
            "<svg>answer</svg>"
        );
        assert_eq!(
            std::fs::read_to_string(final_dir.path().join("nested/detail.svg")).unwrap(),
            "<svg>detail</svg>"
        );
    }

    #[test]
    fn preprocess_cache_requires_results_and_matching_fingerprint() {
        let dir = tempfile::tempdir().unwrap();
        let layout = test_layout(dir.path());

        write_preprocess_fingerprint(&layout, 0x2a).unwrap();
        assert!(!preprocess_cache_hit(&layout, 0x2a).unwrap());

        std::fs::create_dir_all(layout.results_path.parent().unwrap()).unwrap();
        std::fs::write(&layout.results_path, "{}\n").unwrap();
        assert!(preprocess_cache_hit(&layout, 0x2a).unwrap());
        assert!(!preprocess_cache_hit(&layout, 0x2b).unwrap());
    }

    #[test]
    fn render_wrapper_includes_paged_theme_before_source() {
        let dir = tempfile::tempdir().unwrap();
        let layout = test_layout(dir.path());
        let staged_input = PathBuf::from(".calepin/paper/source.typ");
        let paged_theme = crate::theme::PagedSource {
            source: "#let paged-theme-marker = true\n".to_string(),
            owns_body: false,
        };

        let wrapper =
            write_render_wrapper(&layout, &staged_input, &[], Some(&paged_theme)).unwrap();
        let contents = std::fs::read_to_string(dir.path().join(wrapper)).unwrap();

        let theme_marker = contents.find("#let paged-theme-marker = true").unwrap();
        let source_include = contents
            .find("#include \"/.calepin/paper/source.typ\"")
            .unwrap();
        assert!(theme_marker < source_include);
    }

    #[test]
    fn render_wrapper_does_not_duplicate_template_owned_body() {
        let dir = tempfile::tempdir().unwrap();
        let layout = test_layout(dir.path());
        let staged_input = PathBuf::from(".calepin/paper/source.typ");
        let paged_theme = crate::theme::PagedSource {
            source: "#include \"/.calepin/paper/source.typ\"\n[#emph[Appendix]]\n".to_string(),
            owns_body: true,
        };

        let wrapper =
            write_render_wrapper(&layout, &staged_input, &[], Some(&paged_theme)).unwrap();
        let contents = std::fs::read_to_string(dir.path().join(wrapper)).unwrap();

        assert_eq!(
            contents
                .matches("#include \"/.calepin/paper/source.typ\"")
                .count(),
            1
        );
        assert!(contents.contains("[#emph[Appendix]]"));
    }

    #[test]
    fn paged_theme_comes_from_theme_selection() {
        let source = crate::theme::paged_source(
            &crate::theme::ThemeSelection::Default,
            &crate::theme::PagedTemplateContext::default(),
        )
        .unwrap()
        .unwrap();
        assert!(source.source.contains("code-block"));
        assert!(source.source.contains("_fenced-chunks-runs"));
        assert!(crate::theme::paged_source(
            &crate::theme::ThemeSelection::Disabled,
            &crate::theme::PagedTemplateContext::default(),
        )
        .unwrap()
        .is_none());
    }

    #[test]
    fn shared_typst_files_are_staged_under_calepin_dir() {
        let dir = tempfile::tempdir().unwrap();

        write_shared_typst_files(dir.path()).unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.path().join(".calepin/shared/typst/code-block.typ"))
                .unwrap(),
            staging::shared_typst_source("code-block.typ").unwrap()
        );
    }

    fn test_layout(root: &Path) -> LayoutPaths {
        testfixtures::layout(root)
    }

    fn test_chunk(code: &str) -> ChunkSpec {
        testfixtures::chunk("answer", code, ResultsMode::Verbatim)
    }
}
