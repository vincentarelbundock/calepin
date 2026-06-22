mod fingerprint;
mod image_meta;
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
use crate::typst::paths::{
    artifact_reference, project_relative_path, resolve_layout, resolve_layout_in_dir, slash_path,
    CALEPIN_DIR,
};
use crate::typst::query::{parse_chunks_with_warnings, parse_setup_config};
use crate::typst::results::{
    build_results_document, refresh_cached_results_metadata, write_results,
};
use crate::typst::runtime::{
    write_runtime_with_syntax_theme, write_runtime_with_syntax_theme_in_dir,
};
use crate::typst::source_rewrite::write_staged_source;
use crate::typst::sync::write_page_sync;
use crate::typst::version::assert_supported_typst;
use crate::utils::progress::Progress;

const PAGE_META_FILE: &str = "page-meta.json";

use fingerprint::{preprocess_cache_hit, preprocess_fingerprint, write_preprocess_fingerprint};
use image_meta::write_image_meta;
use staging::{notebook_template_context, write_query_source, write_render_wrapper};

pub(crate) use image_meta::image_meta_relative_path;

#[derive(Debug, Clone)]
pub struct PreprocessOptions {
    pub input: PathBuf,
    pub root: Option<PathBuf>,
    pub config: Option<PathBuf>,
    pub display_root: Option<PathBuf>,
    pub quiet: bool,
    pub status: bool,
    pub progress: bool,
    pub timeout: Option<u64>,
    pub sync_pages: bool,
    /// CLI-level theme selection. `None` allows document setup and then
    /// `fallback_theme` to decide.
    pub theme: Option<crate::theme::ThemeSelection>,
    pub fallback_theme: crate::theme::ThemeSelection,
    pub html_syntax_theme: Option<crate::html::HtmlSyntaxTheme>,
    /// Optional custom generated asset directory relative to the input root.
    pub asset_dir: Option<PathBuf>,
    /// `key=value` document-variable overrides from the CLI (`--var`).
    pub var_overrides: Vec<String>,
}

#[derive(Debug)]
pub struct PreprocessOutput {
    pub layout: LayoutPaths,
    pub executables: ExecutablePaths,
    pub theme: crate::theme::ThemeSelection,
    /// Resolved document variables (`[vars]` config < `setup(vars:)` < CLI),
    /// reused as the `vars` context for the single-document HTML theme step.
    pub vars: serde_json::Value,
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
    status: bool,
    progress: bool,
    sync_pages: bool,
    display_root: Option<PathBuf>,
    vars: serde_json::Value,
    theme: crate::theme::ThemeSelection,
}

pub fn preprocess_cached(options: PreprocessOptions) -> Result<PreprocessOutput> {
    let plan = prepare_preprocess_plan(options)?;
    preprocess_cached_plan(plan)
}

pub fn preprocess_cached_plan(plan: PreprocessPlan) -> Result<PreprocessOutput> {
    if preprocess_plan_cache_hit(&plan)? {
        return refresh_cached_preprocess_output(plan);
    }
    execute_preprocess_plan(plan)
}

pub fn preprocess_plan_cache_hit(plan: &PreprocessPlan) -> Result<bool> {
    preprocess_cache_hit(&plan.layout, plan.fingerprint)
}

pub fn preprocess_plan_chunk_count(plan: &PreprocessPlan) -> usize {
    plan.chunks.len()
}

pub fn preprocess_cached_output(plan: PreprocessPlan) -> PreprocessOutput {
    PreprocessOutput {
        layout: plan.layout,
        executables: plan.executables,
        theme: plan.theme,
        vars: plan.vars,
    }
}

pub fn refresh_cached_preprocess_output(plan: PreprocessPlan) -> Result<PreprocessOutput> {
    refresh_cached_results_metadata(&plan.layout.results_path, &plan.chunks)?;
    if !plan.quiet && plan.status {
        eprintln!("[cache] {}", display_input(&plan));
    }
    Ok(preprocess_cached_output(plan))
}

pub fn prepare_preprocess_plan(options: PreprocessOptions) -> Result<PreprocessPlan> {
    let initial_layout = resolve_layout(&options.input, options.root.as_deref())?;
    let config = CalepinConfig::load(&initial_layout.root, options.config.as_deref())?;
    let config_theme = config.theme_selection()?;
    assert_supported_typst(&config.executables.typst)?;

    let html_syntax_theme = options
        .html_syntax_theme
        .clone()
        .unwrap_or_else(crate::html::HtmlSyntaxTheme::builtin);
    let asset_dir = options
        .asset_dir
        .as_deref()
        .or(config.asset_dir.as_deref())
        .unwrap_or_else(|| Path::new(CALEPIN_DIR));
    let mut layout = if asset_dir == Path::new(CALEPIN_DIR) {
        initial_layout
    } else {
        resolve_layout_in_dir(&options.input, options.root.as_deref(), asset_dir)?
    };
    if asset_dir == Path::new(CALEPIN_DIR) {
        write_runtime_with_syntax_theme(&layout.root, &html_syntax_theme)?;
    } else {
        write_runtime_with_syntax_theme_in_dir(&layout.root, asset_dir, &html_syntax_theme)?;
    }
    let runtime_import = format!("/{}/calepin.typ", slash_path(asset_dir));
    let staged_input = write_staged_source(&layout, &runtime_import)?;
    let image_meta = write_image_meta(&layout)?;
    // Metadata collection runs before the final target is known. Use the
    // staged source directly; document-level HTML must be guarded by target
    // checks so paged/query passes never evaluate `html.*` calls.
    let query_source = write_query_source(&layout, &staged_input)?;
    let query_input = write_render_wrapper(&layout, &runtime_import, &query_source, &[], None)?;
    let results_input = artifact_reference(&layout.root, &layout.results_path)?;
    let metadata = preprocess_metadata(
        &config.executables.typst,
        &layout,
        &query_input,
        &results_input,
    )?;
    write_page_meta(&layout, metadata.page_meta.as_ref())?;
    let setup_config = parse_setup_config(&metadata.setup_json)?;
    let setup_config = setup_config.unwrap_or_default();
    let parsed_chunks = merge_chunk_parse_results(
        metadata
            .chunk_queries
            .iter()
            .map(|chunks_json| parse_chunks_with_warnings(chunks_json, Some(setup_config.clone())))
            .collect::<Result<Vec<_>>>()?,
    )?;
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
    let vars = resolve_vars(
        &config.vars,
        &setup_config.defaults.vars,
        &options.var_overrides,
    )?;

    let effective_theme = options
        .theme
        .clone()
        .or(setup_config.defaults.theme_selection(&layout.root)?)
        .or(config_theme)
        .unwrap_or_else(|| options.fallback_theme.clone());
    let notebook_context = notebook_template_context(
        &layout,
        &staged_input,
        metadata.page_meta.clone(),
        vars.clone(),
    );
    let notebook_theme = crate::theme::notebook_source(&effective_theme, &notebook_context)?;
    if !jupyter_kernels.is_empty() {
        let kernels: Vec<&str> = jupyter_kernels.into_iter().collect();
        layout.render_input = write_render_wrapper(
            &layout,
            &runtime_import,
            &staged_input,
            &kernels,
            notebook_theme.as_ref(),
        )?;
    } else {
        layout.render_input = write_render_wrapper(
            &layout,
            &runtime_import,
            &staged_input,
            &[],
            notebook_theme.as_ref(),
        )?;
    }

    let cwd = layout.work_dir.clone();
    let timeout = options.timeout.map(Duration::from_secs);
    let fingerprint = preprocess_fingerprint(
        &layout,
        &config.executables,
        &chunks,
        &cwd,
        timeout,
        &vars,
        &effective_theme,
        asset_dir,
        image_meta.signature()?,
    )?;

    Ok(PreprocessPlan {
        layout,
        executables: config.executables,
        fingerprint,
        chunks,
        cwd,
        timeout,
        quiet: options.quiet,
        status: options.status,
        progress: options.progress,
        sync_pages: options.sync_pages,
        display_root: options.display_root,
        vars,
        theme: effective_theme,
    })
}

fn merge_chunk_parse_results(
    results: Vec<crate::typst::query::ChunkParseResult>,
) -> Result<crate::typst::query::ChunkParseResult> {
    let mut label_index = std::collections::HashMap::new();
    let mut chunks = Vec::new();
    let mut warnings = Vec::new();

    for result in results {
        warnings.extend(result.warnings);
        for chunk in result.chunks {
            if let Some(existing_index) = label_index.get(&chunk.label).copied() {
                let existing = &chunks[existing_index];
                if !same_chunk_definition(existing, &chunk) {
                    // Auto labels are target-local: paged and HTML scans can
                    // both produce `chunk-1` for different hidden branches. The
                    // render runtime only asks for results by label, so allowing
                    // that collision would attach the wrong result. Require
                    // authors to disambiguate target-specific chunks explicitly.
                    return Err(anyhow!(
                        "chunk label `{}` resolves to different code or options across paged/html targets; add explicit labels to target-specific chunks",
                        chunk.label
                    ));
                }
                continue;
            }
            label_index.insert(chunk.label.clone(), chunks.len());
            chunks.push(chunk);
        }
    }

    Ok(crate::typst::query::ChunkParseResult { chunks, warnings })
}

fn same_chunk_definition(left: &ChunkSpec, right: &ChunkSpec) -> bool {
    left.engine == right.engine
        && left.code == right.code
        && left.exec_options == right.exec_options
        && left.display_options == right.display_options
        && left.crossref_labels == right.crossref_labels
}

pub fn execute_preprocess_plan(plan: PreprocessPlan) -> Result<PreprocessOutput> {
    execute_preprocess_plan_with_chunk_progress(plan, None)
}

pub fn execute_preprocess_plan_with_chunk_progress(
    plan: PreprocessPlan,
    chunk_progress: Option<&Progress>,
) -> Result<PreprocessOutput> {
    let staged = tempfile::Builder::new()
        .prefix("calepin-figures-")
        .tempdir()
        .context("failed to create temporary figures directory")?;
    let staged_figures_dir = staged.path().join("figures");
    std::fs::create_dir_all(&staged_figures_dir)
        .with_context(|| format!("failed to create {}", staged_figures_dir.display()))?;

    // Always write vars.json when there are variables: it is the universal
    // transport for Jupyter kernels Calepin cannot auto-bind, and a useful
    // reproducibility record. Native R/Python read their literal prelude instead.
    let vars_path = write_vars_file(&plan.layout, &plan.vars)?;

    let execution_config = ExecutionConfig {
        cwd: plan.cwd.clone(),
        executables: plan.executables.clone(),
        timeout: plan.timeout,
        vars: plan.vars.clone(),
        vars_path,
    };
    let mut pool = EnginePool::new(execution_config);
    let mut chunk_results: Vec<Option<ChunkResultDocument>> = vec![None; plan.chunks.len()];
    let input = display_input(&plan);
    let chunk_count = plan.chunks.len();
    let chunk_word = if chunk_count == 1 { "chunk" } else { "chunks" };
    let progress = if plan.progress {
        Some(Progress::bar(
            format!("[run] {input}: {chunk_count} {chunk_word}"),
            chunk_count as u64,
            plan.quiet,
        ))
    } else {
        if !plan.quiet && plan.status {
            eprintln!("[run] {input}: {chunk_count} {chunk_word}");
        }
        None
    };

    for (position, chunk_index) in chunks_in_engine_order(&plan.chunks).into_iter().enumerate() {
        let chunk = &plan.chunks[chunk_index];
        if let Some(progress) = &progress {
            progress.set_message(format!(
                "[run] {input}: chunk {}/{} `{}`",
                position + 1,
                chunk_count,
                chunk.label
            ));
        }
        let result = execute_chunk_live(&mut pool, chunk, &staged_figures_dir, &plan.layout)?;
        if let Some(progress) = &progress {
            progress.inc(1);
        }
        if let Some(progress) = chunk_progress {
            progress.inc(1);
        }
        chunk_results[chunk_index] = Some(result);
    }

    let chunk_results = chunk_results
        .into_iter()
        .map(|result| {
            result.context("chunk execution produced no result; this indicates a planner bug")
        })
        .collect::<Result<Vec<_>>>()?;

    publish_staged_figures(&staged_figures_dir, &plan.layout.figures_dir)?;
    let document = build_results_document(&plan.layout.input_rel, chunk_results)?;
    write_results(&plan.layout.results_path, &document)?;
    write_preprocess_fingerprint(&plan.layout, plan.fingerprint)?;
    if plan.sync_pages {
        if let Err(error) = write_page_sync(&plan.executables.typst, &plan.layout, &plan.chunks) {
            if !plan.quiet {
                cwarn!("page sync failed: {}", error);
            }
        }
    }
    if let Some(progress) = progress {
        progress.finish(format!("[done] {input}: {chunk_count} {chunk_word}"));
    }

    Ok(PreprocessOutput {
        layout: plan.layout,
        executables: plan.executables,
        theme: plan.theme,
        vars: plan.vars,
    })
}

fn page_meta_path(layout: &LayoutPaths) -> PathBuf {
    layout.artifact_path(PAGE_META_FILE)
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
) -> Result<String> {
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

/// Write `vars.json` next to `results.json` when there are variables, and
/// return its path. Returns `None` (and removes any stale file) when empty.
fn write_vars_file(layout: &LayoutPaths, vars: &serde_json::Value) -> Result<Option<PathBuf>> {
    let path = layout.artifact_path("vars.json");
    let is_empty = vars.as_object().is_none_or(|map| map.is_empty());
    if is_empty {
        let _ = fs::remove_file(&path);
        return Ok(None);
    }
    ensure_parent(&path)?;
    let json = serde_json::to_string_pretty(vars)?;
    write_if_changed(&path, json)?;
    Ok(Some(path))
}

/// Resolve the document's variable map by merging three sources in increasing
/// precedence: `[vars]` from `calepin.toml`, then `calepin.setup(vars: ...)`,
/// then `--var key=value` CLI overrides. CLI values inherit the same scalar
/// typing as `#|` option values.
fn resolve_vars(
    config_vars: &std::collections::BTreeMap<String, toml::Value>,
    setup_vars: &serde_json::Value,
    overrides: &[String],
) -> Result<serde_json::Value> {
    let mut map = serde_json::Map::new();
    if let Ok(serde_json::Value::Object(config_map)) = serde_json::to_value(config_vars) {
        map.extend(config_map);
    }
    if let serde_json::Value::Object(setup_map) = setup_vars {
        for (key, value) in setup_map {
            map.insert(key.clone(), value.clone());
        }
    }
    for entry in overrides {
        let (key, raw_value) = entry
            .split_once('=')
            .ok_or_else(|| anyhow!("invalid --var `{entry}` (expected `key=value`)"))?;
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
    use crate::utils::testutil::command_available;

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
    fn cli_vars_override_setup_vars() {
        let setup = serde_json::json!({"region": "NY", "min_count": 10});
        let resolved = resolve_vars(
            &std::collections::BTreeMap::new(),
            &setup,
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
    fn vars_merge_config_then_setup_then_cli() {
        let config = std::collections::BTreeMap::from([
            ("region".to_string(), toml::Value::String("config".to_string())),
            ("source".to_string(), toml::Value::String("config".to_string())),
        ]);
        let setup = serde_json::json!({"region": "setup", "doc": "setup"});
        let resolved = resolve_vars(&config, &setup, &["region=cli".to_string()]).unwrap();
        assert_eq!(
            resolved,
            serde_json::json!({"region":"cli","source":"config","doc":"setup"})
        );
    }

    #[test]
    fn preprocess_theme_can_come_from_config() {
        if !command_available("typst") {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "#set document(title: [Paper])\nHello").unwrap();
        std::fs::write(dir.path().join("calepin.toml"), r#"theme = "academic""#).unwrap();

        let plan = prepare_preprocess_plan(PreprocessOptions {
            input,
            root: Some(dir.path().to_path_buf()),
            config: Some(dir.path().join("calepin.toml")),
            display_root: None,
            quiet: true,
            status: false,
            progress: false,
            timeout: None,
            sync_pages: false,
            theme: None,
            fallback_theme: crate::theme::ThemeSelection::Default,
            html_syntax_theme: None,
            asset_dir: None,
            var_overrides: Vec::new(),
        })
        .unwrap();

        assert_eq!(
            plan.theme,
            crate::theme::ThemeSelection::Builtin("academic")
        );
    }

    #[test]
    fn preprocess_uses_asset_dir_from_config() {
        if !command_available("typst") {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "#import \".calepin/calepin.typ\" as calepin\nHello").unwrap();
        std::fs::write(dir.path().join("calepin.toml"), r#"asset-dir = "_runtime""#).unwrap();

        let plan = prepare_preprocess_plan(PreprocessOptions {
            input,
            root: Some(dir.path().to_path_buf()),
            config: Some(dir.path().join("calepin.toml")),
            display_root: None,
            quiet: true,
            status: false,
            progress: false,
            timeout: None,
            sync_pages: false,
            theme: None,
            fallback_theme: crate::theme::ThemeSelection::Default,
            html_syntax_theme: None,
            asset_dir: None,
            var_overrides: Vec::new(),
        })
        .unwrap();

        let staged_source =
            std::fs::read_to_string(plan.layout.root.join("_runtime/paper/source.typ")).unwrap();
        assert!(staged_source.contains("#import \"/_runtime/calepin.typ\" as calepin"));
        assert_eq!(
            plan.layout.artifact_dir,
            plan.layout.root.join("_runtime/paper")
        );
        let wrapper =
            std::fs::read_to_string(plan.layout.artifact_path("calepin-wrapper.typ")).unwrap();
        assert!(wrapper.contains("#import \"/_runtime/calepin.typ\""));
        assert!(!wrapper.contains("#import \"/.calepin/calepin.typ\""));
        assert!(plan.layout.root.join("_runtime/calepin.typ").is_file());
    }

    #[test]
    fn cli_var_without_equals_is_rejected() {
        let err = resolve_vars(
            &std::collections::BTreeMap::new(),
            &serde_json::json!({}),
            &["bad".to_string()],
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("bad"), "{err}");
    }

    #[test]
    fn resolve_vars_with_no_overrides_returns_setup() {
        let setup = serde_json::json!({"a": 1});
        assert_eq!(
            resolve_vars(&std::collections::BTreeMap::new(), &setup, &[]).unwrap(),
            setup
        );
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
            Path::new(CALEPIN_DIR),
            0,
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
            Path::new(CALEPIN_DIR),
            0,
        )
        .unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn preprocess_fingerprint_tracks_vars() {
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
            Path::new(CALEPIN_DIR),
            0,
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
            Path::new(CALEPIN_DIR),
            0,
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
            Path::new(CALEPIN_DIR),
            0,
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
            Path::new(CALEPIN_DIR),
            0,
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
            Path::new(CALEPIN_DIR),
            0,
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
            Path::new(CALEPIN_DIR),
            0,
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
            Path::new(CALEPIN_DIR),
            0,
        )
        .unwrap();
        let changed = preprocess_fingerprint(
            &layout,
            &executables,
            &[chunk],
            dir.path(),
            Some(Duration::from_secs(5)),
            &serde_json::json!({}),
            &crate::theme::ThemeSelection::Typst,
            Path::new(CALEPIN_DIR),
            0,
        )
        .unwrap();
        assert_ne!(baseline, changed);
    }

    #[test]
    fn preprocess_fingerprint_tracks_asset_dir() {
        let dir = tempfile::tempdir().unwrap();
        let layout = test_layout(dir.path());
        let executables = ExecutablePaths::defaults();
        let chunk = test_chunk("print(1)");
        let default_asset_dir = preprocess_fingerprint(
            &layout,
            &executables,
            std::slice::from_ref(&chunk),
            dir.path(),
            Some(Duration::from_secs(5)),
            &serde_json::json!({}),
            &crate::theme::ThemeSelection::Default,
            Path::new(CALEPIN_DIR),
            0,
        )
        .unwrap();
        let custom_asset_dir = preprocess_fingerprint(
            &layout,
            &executables,
            &[chunk],
            dir.path(),
            Some(Duration::from_secs(5)),
            &serde_json::json!({}),
            &crate::theme::ThemeSelection::Default,
            Path::new("_runtime"),
            0,
        )
        .unwrap();
        assert_ne!(default_asset_dir, custom_asset_dir);
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
            )
            .unwrap(),
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
    fn preprocess_cached_plan_reuses_matching_cache() {
        let dir = tempfile::tempdir().unwrap();
        let layout = test_layout(dir.path());
        let fingerprint = 0x2a;
        let chunk = test_chunk("print(1)");
        let results = build_results_document(
            &layout.input_rel,
            vec![crate::typst::model::ChunkResultDocument {
                label: chunk.label.clone(),
                engine: chunk.engine.clone(),
                status: crate::typst::model::ChunkStatus::Ok,
                display_options: chunk.display_options.clone(),
                items: Vec::new(),
                crossref_labels: chunk.crossref_labels.clone(),
            }],
        )
        .unwrap();
        write_results(&layout.results_path, &results).unwrap();
        write_preprocess_fingerprint(&layout, fingerprint).unwrap();
        let mut executables = ExecutablePaths::defaults();
        executables.python = PathBuf::from("/no/such/python");

        let output = preprocess_cached_plan(PreprocessPlan {
            layout,
            executables,
            fingerprint,
            chunks: vec![chunk],
            cwd: dir.path().to_path_buf(),
            timeout: None,
            quiet: true,
            status: false,
            progress: false,
            sync_pages: false,
            display_root: None,
            vars: serde_json::json!({}),
            theme: crate::theme::ThemeSelection::Default,
        })
        .unwrap();

        assert_eq!(output.layout.input_rel, PathBuf::from("paper.typ"));
    }

    #[test]
    fn render_wrapper_rewrites_notebook_theme_runtime_import() {
        let dir = tempfile::tempdir().unwrap();
        let layout = test_layout(dir.path());
        let staged_input = PathBuf::from(".calepin/paper/source.typ");
        let notebook_theme = crate::theme::NotebookSource {
            source: "#import \"/.calepin/calepin.typ\": _html-themed-raw-block\n".to_string(),
        };

        let wrapper = write_render_wrapper(
            &layout,
            "/_runtime/calepin.typ",
            &staged_input,
            &[],
            Some(&notebook_theme),
        )
        .unwrap();
        let contents = std::fs::read_to_string(dir.path().join(wrapper)).unwrap();

        assert!(contents.contains("#import \"/_runtime/calepin.typ\": _html-themed-raw-block"));
        assert!(!contents.contains("#import \"/.calepin/calepin.typ\": _html-themed-raw-block"));
    }

    #[test]
    fn render_wrapper_includes_notebook_theme_before_source() {
        let dir = tempfile::tempdir().unwrap();
        let layout = test_layout(dir.path());
        let staged_input = PathBuf::from(".calepin/paper/source.typ");
        let notebook_theme = crate::theme::NotebookSource {
            source: "#let notebook-theme-marker = true\n".to_string(),
        };

        let wrapper = write_render_wrapper(
            &layout,
            "/.calepin/calepin.typ",
            &staged_input,
            &[],
            Some(&notebook_theme),
        )
        .unwrap();
        let contents = std::fs::read_to_string(dir.path().join(wrapper)).unwrap();

        let theme_marker = contents.find("#let notebook-theme-marker = true").unwrap();
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
        let notebook_theme = crate::theme::NotebookSource {
            source: "#include \"/.calepin/paper/source.typ\"\n[#emph[Appendix]]\n".to_string(),
        };

        let wrapper = write_render_wrapper(
            &layout,
            "/.calepin/calepin.typ",
            &staged_input,
            &[],
            Some(&notebook_theme),
        )
        .unwrap();
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
    fn render_wrapper_themes_generic_raw_blocks_in_paged_output() {
        let dir = tempfile::tempdir().unwrap();
        let layout = test_layout(dir.path());
        let staged_input = PathBuf::from(".calepin/paper/source.typ");

        let wrapper = write_render_wrapper(
            &layout,
            "/.calepin/calepin.typ",
            &staged_input,
            &["bash"],
            None,
        )
        .unwrap();
        let contents = std::fs::read_to_string(dir.path().join(wrapper)).unwrap();

        assert!(
            !contents.contains("else if not _is-html() {\n    it\n  }"),
            "generic raw fallback must not leave paged raw blocks unthemed:\n{contents}"
        );
        assert!(
            contents.contains("_raw-chunk-langs.contains(it.lang)"),
            "generic raw fallback should only defer to known executable chunk languages:\n{contents}"
        );
        assert!(
            contents.contains("\"bash\""),
            "jupyter kernels should be included among known executable chunk languages:\n{contents}"
        );
        assert!(
            contents.contains("_html-themed-raw-block(it)"),
            "generic raw fallback should route non-running fences through Calepin code styling:\n{contents}"
        );
    }

    #[test]
    fn notebook_theme_comes_from_theme_selection() {
        for selection in [
            crate::theme::ThemeSelection::Default,
            crate::theme::ThemeSelection::Builtin("academic"),
        ] {
            let source = crate::theme::notebook_source(
                &selection,
                &crate::theme::NotebookTemplateContext::default(),
            )
            .unwrap()
            .unwrap();
            assert!(source.source.contains("_html-themed-raw-block"));
            assert!(source.source.contains("_raw-chunk-langs.contains(it.lang)"));
            assert!(source.source.contains("_fenced-chunks-runs"));
        }
        assert!(crate::theme::notebook_source(
            &crate::theme::ThemeSelection::Typst,
            &crate::theme::NotebookTemplateContext::default(),
        )
        .unwrap()
        .is_none());
    }

    fn test_layout(root: &Path) -> LayoutPaths {
        testfixtures::layout(root)
    }

    fn test_chunk(code: &str) -> ChunkSpec {
        testfixtures::chunk("answer", code, ResultsMode::Verbatim)
    }
}
