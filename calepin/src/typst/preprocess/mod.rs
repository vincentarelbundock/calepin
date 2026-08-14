mod fingerprint;
mod image_meta;
mod staging;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use xxhash_rust::xxh3::xxh3_64;

use crate::config::{CalepinConfig, ExecutablePaths};
use crate::typst::execute::{EnginePool, ExecutionConfig};
use crate::typst::introspect::preprocess_metadata;
use crate::typst::io::{ensure_parent, write_if_changed};
use crate::typst::model::{
    ChunkResultDocument, ChunkSpec, EngineName, LayoutPaths, ResultsDocument, RESULT_SCHEMA_VERSION,
};
use crate::typst::paths::{
    artifact_reference, project_relative_path, resolve_layout, resolve_layout_in_dir, slash_path,
    CALEPIN_DIR,
};
use crate::typst::query::{parse_chunks_with_warnings, parse_setup_config};
use crate::typst::results::{
    build_results_document_with_store, refresh_cached_results_metadata, write_results,
};
use crate::typst::runtime::{
    write_notebook_binding, write_runtime_with_syntax_theme, write_runtime_with_syntax_theme_in_dir,
};
use crate::typst::source_rewrite::write_staged_source;
use crate::typst::sync::write_page_sync;
use crate::typst::version::assert_supported_typst;
use crate::utils::progress::Progress;

const PAGE_META_FILE: &str = "page-meta.json";
const EXPANSION_MANIFEST_FILE: &str = "expansion.json";
const EXPANSION_MANIFEST_SCHEMA: u8 = 1;

use fingerprint::{
    preprocess_cache_hit, preprocess_fingerprint, write_preprocess_fingerprint,
    ExecutionFingerprintInputs,
};
use image_meta::write_image_meta;
use staging::{
    notebook_template_context, raw_chunk_langs, write_query_wrapper,
    write_render_wrapper,
};

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
    /// `key=value` config overrides from the CLI (`--set`).
    pub config_overrides: Vec<String>,
    /// Re-execute chunks even when the preprocessing fingerprint matches.
    pub force: bool,
}

#[derive(Debug)]
pub struct PreprocessOutput {
    pub layout: LayoutPaths,
    pub executables: ExecutablePaths,
    pub theme: crate::theme::ThemeSelection,
    /// Completed document store, reused by the HTML theme step.
    pub store: serde_json::Value,
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
    force: bool,
    display_root: Option<PathBuf>,
    store: serde_json::Value,
    theme: crate::theme::ThemeSelection,
    raw_languages: Vec<String>,
    query_input: PathBuf,
    query_theme: crate::theme::ThemeSelection,
    query_store_path: PathBuf,
    results_input: String,
    store_input: String,
    setup_config: crate::typst::query::SetupConfig,
    initializers: serde_json::Map<String, serde_json::Value>,
    staged_input: PathBuf,
    runtime_import: String,
    page_meta: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExpansionManifest {
    schema: u8,
    fingerprint: u64,
    generation: String,
    completed_store: serde_json::Map<String, Value>,
    stabilized_chunks: Vec<ChunkSpec>,
    writers: std::collections::BTreeMap<String, String>,
}

pub fn preprocess_cached(options: PreprocessOptions) -> Result<PreprocessOutput> {
    let plan = prepare_preprocess_plan(options)?;
    preprocess_cached_plan(plan)
}

pub fn preprocess_cached_plan(mut plan: PreprocessPlan) -> Result<PreprocessOutput> {
    if preprocess_plan_cache_hit(&mut plan)? {
        return refresh_cached_preprocess_output(plan);
    }
    execute_preprocess_plan(plan)
}

pub fn preprocess_plan_cache_hit(plan: &mut PreprocessPlan) -> Result<bool> {
    if plan.force {
        return Ok(false);
    }
    if !preprocess_cache_hit(&plan.layout, plan.fingerprint)? {
        return Ok(false);
    }
    apply_expansion_cache(plan)
}

pub fn preprocess_plan_chunk_count(plan: &PreprocessPlan) -> usize {
    plan.chunks.len()
}

/// Consume a prepared plan without executing its chunks.
pub fn preprocess_plan_into_chunks(plan: PreprocessPlan) -> Vec<ChunkSpec> {
    plan.chunks
}

pub fn preprocess_cached_output(plan: PreprocessPlan) -> PreprocessOutput {
    PreprocessOutput {
        layout: plan.layout,
        executables: plan.executables,
        theme: plan.theme,
        store: plan.store,
    }
}

pub fn refresh_cached_preprocess_output(plan: PreprocessPlan) -> Result<PreprocessOutput> {
    refresh_cached_results_metadata(&plan.layout.results_path, &plan.chunks)?;
    let _ = fs::remove_file(&plan.query_store_path);
    write_notebook_binding(&plan.layout, &plan.raw_languages)?;
    if !plan.quiet && plan.status {
        eprintln!("[cache] {}", display_input(&plan));
    }
    Ok(preprocess_cached_output(plan))
}

pub fn prepare_preprocess_plan(options: PreprocessOptions) -> Result<PreprocessPlan> {
    let initial_layout = resolve_layout(&options.input, options.root.as_deref())?;
    let config = CalepinConfig::load_with_overrides(
        &initial_layout.root,
        options.config.as_deref(),
        &options.config_overrides,
    )?;
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
    // Metadata collection runs before the final target is known. The query pass
    // includes the same staged source as the render pass; document-level HTML
    // must be guarded by target checks so paged/query passes never evaluate
    // `html.*` calls.
    // Theme the query pass with the same theme + inlined body as render, so the
    // document body shares the theme's scope during metadata extraction too (it
    // is evaluated in both passes). The theme must be knowable before the query:
    // CLI > config > fallback. In-document `setup(theme:)` cannot participate
    // here because it is itself extracted by this pass, so a theme that exports
    // a vocabulary for the body must be selected via config or the CLI.
    let pre_query_theme = options
        .theme
        .clone()
        .or_else(|| config_theme.clone())
        .unwrap_or_else(|| options.fallback_theme.clone());
    let external_store = resolve_store(&config.store, &serde_json::Map::new())?;
    let mut query_input = write_store_aware_query_wrapper(
        &layout,
        &runtime_import,
        &staged_input,
        &pre_query_theme,
        &external_store,
    )?;
    let results_input = artifact_reference(&layout.root, &layout.results_path)?;
    let query_store_path = layout.artifact_path("query-store.json");
    ensure_parent(&query_store_path)?;
    write_if_changed(&query_store_path, serde_json::to_vec(&external_store)?)?;
    let store_input = artifact_reference(&layout.root, &query_store_path)?;
    let mut metadata = preprocess_metadata(
        &config.executables.typst,
        &layout,
        &query_input,
        &results_input,
        &store_input,
    )?;
    let initializers = parse_typst_initializers(&metadata.store_initializer_queries)?;
    let store = merge_initial_store(external_store, &initializers, &options.config_overrides)?;
    write_if_changed(&query_store_path, serde_json::to_vec(&store)?)?;
    if !initializers.is_empty() {
        query_input = write_store_aware_query_wrapper(
            &layout,
            &runtime_import,
            &staged_input,
            &pre_query_theme,
            &store,
        )?;
        metadata = preprocess_metadata(
            &config.executables.typst,
            &layout,
            &query_input,
            &results_input,
            &store_input,
        )?;
        let repeated = parse_typst_initializers(&metadata.store_initializer_queries)?;
        if repeated != initializers {
            return Err(anyhow!(
                "calepin.store.set() declarations changed after the initial store was resolved"
            ));
        }
    }
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
    validate_store_plan(
        &chunks,
        store.as_object().expect("resolved store is an object"),
    )?;
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
    let kernel_names = jupyter_kernels.iter().copied().collect::<Vec<_>>();
    let raw_languages = raw_chunk_langs(&kernel_names);
    let effective_theme = options
        .theme
        .clone()
        .or(setup_config.defaults.theme_selection(&layout.root)?)
        .or(config_theme)
        .unwrap_or_else(|| options.fallback_theme.clone());
    // Query passes use a separate wrapper. Keep the active render wrapper
    // untouched until results and the completed store are ready to publish.
    layout.render_input = layout.entry_relative_path("wrapper.typ");

    let cwd = layout.work_dir.clone();
    let timeout = options.timeout.map(Duration::from_secs);
    let fingerprint = preprocess_fingerprint(
        &layout,
        &config.executables,
        &chunks,
        ExecutionFingerprintInputs {
            cwd: &cwd,
            timeout,
            store: &store,
        },
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
        force: options.force,
        display_root: options.display_root,
        store,
        theme: effective_theme,
        raw_languages,
        query_input,
        query_theme: pre_query_theme,
        query_store_path,
        results_input,
        store_input,
        setup_config,
        initializers,
        staged_input,
        runtime_import,
        page_meta: metadata.page_meta,
    })
}

fn write_store_aware_query_wrapper(
    layout: &LayoutPaths,
    runtime_import: &str,
    staged_input: &Path,
    theme: &crate::theme::ThemeSelection,
    store: &Value,
) -> Result<PathBuf> {
    let context = notebook_template_context(layout, staged_input, None, store.clone())?;
    let query_theme = crate::theme::notebook_source(theme, &context)?;
    write_query_wrapper(
        layout,
        runtime_import,
        if query_theme.is_some() {
            None
        } else {
            Some(staged_input)
        },
        query_theme.as_ref(),
    )
}

fn completed_generation(
    fingerprint: u64,
    store: &serde_json::Map<String, Value>,
) -> Result<String> {
    let store_hash = xxh3_64(&serde_json::to_vec(store)?);
    Ok(format!("{fingerprint:016x}-{store_hash:016x}"))
}

fn expansion_manifest_path(layout: &LayoutPaths) -> PathBuf {
    layout.artifact_path(EXPANSION_MANIFEST_FILE)
}

fn write_expansion_manifest(
    plan: &PreprocessPlan,
    generation: &str,
    completed_store: &serde_json::Map<String, Value>,
) -> Result<()> {
    let manifest = ExpansionManifest {
        schema: EXPANSION_MANIFEST_SCHEMA,
        fingerprint: plan.fingerprint,
        generation: generation.to_string(),
        completed_store: completed_store.clone(),
        stabilized_chunks: plan.chunks.clone(),
        writers: writer_provenance(&plan.chunks),
    };
    let bytes = serde_json::to_vec_pretty(&manifest)?;
    write_if_changed(expansion_manifest_path(&plan.layout).as_path(), bytes)
}

fn read_expansion_manifest(layout: &LayoutPaths) -> Option<ExpansionManifest> {
    let bytes = fs::read(expansion_manifest_path(layout)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn restore_initial_query_state(plan: &mut PreprocessPlan) -> Result<()> {
    write_if_changed(&plan.query_store_path, serde_json::to_vec(&plan.store)?)?;
    plan.query_input = write_store_aware_query_wrapper(
        &plan.layout,
        &plan.runtime_import,
        &plan.staged_input,
        &plan.query_theme,
        &plan.store,
    )?;
    Ok(())
}

fn apply_expansion_cache(plan: &mut PreprocessPlan) -> Result<bool> {
    let Some(manifest) = read_expansion_manifest(&plan.layout) else {
        return Ok(false);
    };
    if manifest.schema != EXPANSION_MANIFEST_SCHEMA || manifest.fingerprint != plan.fingerprint {
        return Ok(false);
    }
    if crate::typst::store::validate_store(&manifest.completed_store).is_err() {
        return Ok(false);
    }
    let expected_generation = completed_generation(plan.fingerprint, &manifest.completed_store)?;
    if manifest.generation != expected_generation {
        return Ok(false);
    }

    write_if_changed(
        &plan.query_store_path,
        serde_json::to_vec(&manifest.completed_store)?,
    )?;
    plan.query_input = write_store_aware_query_wrapper(
        &plan.layout,
        &plan.runtime_import,
        &plan.staged_input,
        &plan.query_theme,
        &Value::Object(manifest.completed_store.clone()),
    )?;
    let metadata = preprocess_metadata(
        &plan.executables.typst,
        &plan.layout,
        &plan.query_input,
        &plan.results_input,
        &plan.store_input,
    )?;
    let repeated = parse_typst_initializers(&metadata.store_initializer_queries)?;
    if repeated != plan.initializers {
        restore_initial_query_state(plan)?;
        return Ok(false);
    }
    let setup_config = parse_setup_config(&metadata.setup_json)?.unwrap_or_default();
    let parsed = merge_chunk_parse_results(
        metadata
            .chunk_queries
            .iter()
            .map(|json| parse_chunks_with_warnings(json, Some(setup_config.clone())))
            .collect::<Result<Vec<_>>>()?,
    )?;
    validate_store_plan(
        &parsed.chunks,
        plan.store
            .as_object()
            .expect("resolved initial store is an object"),
    )?;
    if parsed.chunks.len() != manifest.stabilized_chunks.len()
        || parsed
            .chunks
            .iter()
            .zip(&manifest.stabilized_chunks)
            .any(|(current, cached)| !same_executed_chunk(current, cached))
    {
        restore_initial_query_state(plan)?;
        return Ok(false);
    }
    if writer_provenance(&parsed.chunks) != manifest.writers {
        restore_initial_query_state(plan)?;
        return Ok(false);
    }

    let results = fs::read_to_string(&plan.layout.results_path)
        .ok()
        .and_then(|text| serde_json::from_str::<ResultsDocument>(&text).ok());
    let Some(results) = results else {
        restore_initial_query_state(plan)?;
        return Ok(false);
    };
    if results.schema != RESULT_SCHEMA_VERSION
        || results.generation != manifest.generation
        || results.store != manifest.completed_store
        || results.chunks.len() != parsed.chunks.len()
        || parsed
            .chunks
            .iter()
            .any(|chunk| !results.chunks.contains_key(&chunk.label))
    {
        restore_initial_query_state(plan)?;
        return Ok(false);
    }

    plan.chunks = parsed.chunks;
    plan.store = Value::Object(manifest.completed_store);
    plan.setup_config = setup_config;
    plan.page_meta = metadata.page_meta;
    plan.raw_languages = raw_languages_for_chunks(&plan.chunks);
    write_final_render_wrapper(plan, &manifest.generation)?;
    let _ = fs::remove_file(&plan.query_store_path);
    Ok(true)
}

fn writer_provenance(chunks: &[ChunkSpec]) -> std::collections::BTreeMap<String, String> {
    chunks
        .iter()
        .flat_map(|chunk| {
            chunk
                .exec_options
                .store_set
                .iter()
                .map(|key| (key.clone(), chunk.label.clone()))
        })
        .collect()
}

fn raw_languages_for_chunks(chunks: &[ChunkSpec]) -> Vec<String> {
    let kernels = chunks
        .iter()
        .filter_map(|chunk| match &chunk.engine {
            EngineName::Jupyter(kernel) => Some(kernel.as_str()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    raw_chunk_langs(&kernels.into_iter().collect::<Vec<_>>())
}

fn write_final_render_wrapper(plan: &mut PreprocessPlan, generation: &str) -> Result<()> {
    let context = notebook_template_context(
        &plan.layout,
        &plan.staged_input,
        plan.page_meta.clone(),
        plan.store.clone(),
    )?;
    let theme = crate::theme::notebook_source(&plan.theme, &context)?;
    let kernels = plan
        .chunks
        .iter()
        .filter_map(|chunk| match &chunk.engine {
            EngineName::Jupyter(kernel) => Some(kernel.as_str()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    plan.layout.render_input = write_render_wrapper(
        &plan.layout,
        &plan.runtime_import,
        if theme.is_some() {
            None
        } else {
            Some(&plan.staged_input)
        },
        &kernels.into_iter().collect::<Vec<_>>(),
        theme.as_ref(),
        Some(generation),
    )?;
    Ok(())
}

fn merge_chunk_parse_results(
    results: Vec<crate::typst::query::ChunkParseResult>,
) -> Result<crate::typst::query::ChunkParseResult> {
    let mut label_index = std::collections::HashMap::new();
    let mut chunks = Vec::new();
    let mut warnings = Vec::new();
    let mut target_orders = Vec::new();

    for result in results {
        warnings.extend(result.warnings);
        let mut order = Vec::new();
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
                order.push(existing_index);
                continue;
            }
            label_index.insert(chunk.label.clone(), chunks.len());
            order.push(chunks.len());
            chunks.push(chunk);
        }
        target_orders.push(order);
    }

    let mut outgoing = vec![std::collections::BTreeSet::new(); chunks.len()];
    let mut indegree = vec![0usize; chunks.len()];
    for order in target_orders {
        for pair in order.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            if from != to && outgoing[from].insert(to) {
                indegree[to] += 1;
            }
        }
    }
    let mut ready: std::collections::BTreeSet<usize> = indegree
        .iter()
        .enumerate()
        .filter_map(|(index, degree)| (*degree == 0).then_some(index))
        .collect();
    let mut merged = Vec::with_capacity(chunks.len());
    while let Some(index) = ready.pop_first() {
        merged.push(chunks[index].clone());
        for &next in &outgoing[index] {
            indegree[next] -= 1;
            if indegree[next] == 0 {
                ready.insert(next);
            }
        }
    }
    if merged.len() != chunks.len() {
        return Err(anyhow!(
            "paged and HTML target plans impose incompatible chunk ordering"
        ));
    }

    Ok(crate::typst::query::ChunkParseResult {
        chunks: merged,
        warnings,
    })
}

fn same_chunk_definition(left: &ChunkSpec, right: &ChunkSpec) -> bool {
    left.engine == right.engine
        && left.code == right.code
        && left.script == right.script
        && left.exec_options == right.exec_options
        && left.display_options == right.display_options
        && left.crossref_labels == right.crossref_labels
}

fn same_executed_chunk(left: &ChunkSpec, right: &ChunkSpec) -> bool {
    left.label == right.label
        && left.ordinal == right.ordinal
        && left.engine == right.engine
        && left.code == right.code
        && left.script == right.script
        && left.exec_options == right.exec_options
}

fn validate_store_plan(
    chunks: &[ChunkSpec],
    initialized: &serde_json::Map<String, serde_json::Value>,
) -> Result<()> {
    let mut writers = std::collections::HashMap::<&str, &str>::new();
    for chunk in chunks {
        if (!chunk.exec_options.store_get.is_empty() || !chunk.exec_options.store_set.is_empty())
            && !matches!(chunk.engine, EngineName::R | EngineName::Python)
        {
            return Err(anyhow!(
                "chunk `{}` declares a document store option, but engine `{}` does not support the Calepin document store",
                chunk.label,
                chunk.engine
            ));
        }
        for key in &chunk.exec_options.store_set {
            if initialized.contains_key(key) {
                return Err(anyhow!(
                    "store key `{key}` is initialized before execution and is also written by chunk `{}`",
                    chunk.label
                ));
            }
            if let Some(first) = writers.insert(key, &chunk.label) {
                return Err(anyhow!(
                    "store key `{key}` has more than one writer: chunk `{first}` and chunk `{}`",
                    chunk.label
                ));
            }
        }
    }
    if initialized.len() + writers.len() > crate::typst::store::MAX_KEYS {
        return Err(anyhow!(
            "document store declares more than {} initialized and written keys",
            crate::typst::store::MAX_KEYS
        ));
    }
    Ok(())
}

fn validate_executed_prefix(
    previous: &[ChunkSpec],
    next: &[ChunkSpec],
    executed: usize,
) -> Result<()> {
    if next.len() < executed {
        return Err(anyhow!(
            "dynamic store expansion removed an already-executed chunk"
        ));
    }
    for index in 0..executed {
        let old = &previous[index];
        let new = &next[index];
        if !same_executed_chunk(old, new) {
            return Err(anyhow!(
                "dynamic store expansion changed the already-executed chunk `{}`; stored values may generate chunks only after the execution frontier",
                old.label
            ));
        }
    }
    Ok(())
}

pub fn execute_preprocess_plan(plan: PreprocessPlan) -> Result<PreprocessOutput> {
    execute_preprocess_plan_with_chunk_progress(plan, None)
}

pub fn execute_preprocess_plan_with_chunk_progress(
    mut plan: PreprocessPlan,
    chunk_progress: Option<&Progress>,
) -> Result<PreprocessOutput> {
    let staged = tempfile::Builder::new()
        .prefix("calepin-figures-")
        .tempdir()
        .context("failed to create temporary figures directory")?;
    let staged_figures_dir = staged.path().join("figures");
    std::fs::create_dir_all(&staged_figures_dir)
        .with_context(|| format!("failed to create {}", staged_figures_dir.display()))?;

    let execution_config = ExecutionConfig {
        cwd: plan.cwd.clone(),
        executables: plan.executables.clone(),
        timeout: plan.timeout,
        store: plan.store.as_object().cloned().unwrap_or_default(),
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

    let mut chunk_index = 0;
    let mut expansion_stages = 0usize;
    while chunk_index < plan.chunks.len() {
        let chunk = plan.chunks[chunk_index].clone();
        if let Some(progress) = &progress {
            progress.set_message(format!(
                "[run] {input}: chunk {}/{} `{}`",
                chunk_index + 1,
                plan.chunks.len(),
                chunk.label
            ));
        }
        let result = execute_chunk_live(&mut pool, &chunk, &staged_figures_dir, &plan.layout)?;
        if let Some(progress) = &progress {
            progress.inc(1);
        }
        if let Some(progress) = chunk_progress {
            progress.inc(1);
        }
        chunk_results[chunk_index] = Some(result);
        chunk_index += 1;

        if !chunk.exec_options.store_set.is_empty() {
            write_if_changed(
                &plan.query_store_path,
                serde_json::to_vec(&Value::Object(pool.store().clone()))?,
            )?;
            plan.query_input = write_store_aware_query_wrapper(
                &plan.layout,
                &plan.runtime_import,
                &plan.staged_input,
                &plan.query_theme,
                &Value::Object(pool.store().clone()),
            )?;
            let metadata = preprocess_metadata(
                &plan.executables.typst,
                &plan.layout,
                &plan.query_input,
                &plan.results_input,
                &plan.store_input,
            )?;
            let repeated = parse_typst_initializers(&metadata.store_initializer_queries)?;
            if repeated != plan.initializers {
                return Err(anyhow!(
                    "calepin.store.set() declarations changed after execution began"
                ));
            }
            let next_setup = parse_setup_config(&metadata.setup_json)?.unwrap_or_default();
            let parsed = merge_chunk_parse_results(
                metadata
                    .chunk_queries
                    .iter()
                    .map(|json| parse_chunks_with_warnings(json, Some(next_setup.clone())))
                    .collect::<Result<Vec<_>>>()?,
            )?;
            let next_chunks = parsed.chunks;
            validate_store_plan(
                &next_chunks,
                plan.store.as_object().expect("resolved store is an object"),
            )?;
            validate_executed_prefix(&plan.chunks, &next_chunks, chunk_index)?;
            let old_writers: std::collections::HashSet<_> = plan
                .chunks
                .iter()
                .flat_map(|chunk| chunk.exec_options.store_set.iter().cloned())
                .collect();
            let new_writers: Vec<_> = next_chunks
                .iter()
                .flat_map(|chunk| chunk.exec_options.store_set.iter().cloned())
                .filter(|key| !old_writers.contains(key))
                .collect();
            if !new_writers.is_empty() {
                expansion_stages += 1;
                if expansion_stages > 32 {
                    return Err(anyhow!(
                        "dynamic store expansion did not stabilize after 32 new-writer stages"
                    ));
                }
            }
            plan.chunks = next_chunks;
            plan.setup_config = next_setup;
            plan.page_meta = metadata.page_meta;
            chunk_results.resize(plan.chunks.len(), None);
        }
    }

    let chunk_results = chunk_results
        .into_iter()
        .map(|result| {
            result.context("chunk execution produced no result; this indicates a planner bug")
        })
        .collect::<Result<Vec<_>>>()?;

    publish_staged_figures(&staged_figures_dir, &plan.layout.figures_dir)?;
    let completed_store = pool.store().clone();
    let generation = completed_generation(plan.fingerprint, &completed_store)?;
    let document = build_results_document_with_store(
        &plan.layout.input_rel,
        chunk_results,
        completed_store.clone(),
        generation.clone(),
    )?;
    // Publish results before the wrapper. The wrapper carries the same
    // generation and refuses to render against a mismatched results snapshot,
    // so an active Typst watcher cannot publish a mixed build.
    write_results(&plan.layout.results_path, &document)?;
    plan.store = Value::Object(completed_store.clone());
    plan.raw_languages = raw_languages_for_chunks(&plan.chunks);
    write_final_render_wrapper(&mut plan, &generation)?;
    let _ = fs::remove_file(&plan.query_store_path);
    write_notebook_binding(&plan.layout, &plan.raw_languages)?;
    write_expansion_manifest(&plan, &generation, &completed_store)?;
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
        store: serde_json::Value::Object(completed_store),
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
/// `input` under `artifact_dir`, or `None` when it is missing or stale for the
/// current content. Sites that configure `asset-dir` persist page metadata
/// there rather than in `.calepin`, so callers must pass the same directory
/// preprocessing wrote to.
pub fn read_page_meta_in_dir(
    input: &Path,
    root: Option<&Path>,
    artifact_dir: &Path,
) -> Option<serde_json::Value> {
    let layout = resolve_layout_in_dir(input, root, artifact_dir).ok()?;
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
        let source_is_dir = entry
            .file_type()
            .with_context(|| format!("failed to stat {}", path.display()))?
            .is_dir();
        remove_mismatched_figure_target(&target, source_is_dir)?;
        if source_is_dir {
            publish_staged_figures(&path, &target)?;
        } else {
            publish_staged_file(&path, &target)?;
        }
    }

    prune_stale_figures(staged, final_dir)?;
    Ok(())
}

fn remove_mismatched_figure_target(target: &Path, source_is_dir: bool) -> Result<()> {
    let metadata = match std::fs::symlink_metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to stat {}", target.display()))
        }
    };
    let target_is_dir = metadata.file_type().is_dir();
    if source_is_dir == target_is_dir {
        return Ok(());
    }
    remove_figure_artifact(target, target_is_dir)
}

fn prune_stale_figures(staged: &Path, final_dir: &Path) -> Result<()> {
    let entries = match std::fs::read_dir(final_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", final_dir.display()))
        }
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to read {}", final_dir.display()))?;
        if staged.join(entry.file_name()).exists() {
            continue;
        }
        let path = entry.path();
        let is_dir = entry
            .file_type()
            .with_context(|| format!("failed to stat {}", path.display()))?
            .is_dir();
        remove_figure_artifact(&path, is_dir)?;
    }
    Ok(())
}

fn remove_figure_artifact(path: &Path, is_dir: bool) -> Result<()> {
    if is_dir {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
    .with_context(|| format!("failed to remove stale figure {}", path.display()))
}

fn publish_staged_file(source: &Path, target: &Path) -> Result<()> {
    let bytes = std::fs::read(source)
        .with_context(|| format!("failed to read staged figure {}", source.display()))?;
    write_if_changed(target, bytes)
}

/// Resolve externally initialized store values.
fn resolve_store(
    config_store: &std::collections::BTreeMap<String, toml::Value>,
    overrides: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value> {
    let mut map = serde_json::Map::new();
    if let Ok(serde_json::Value::Object(config_map)) = serde_json::to_value(config_store) {
        map.extend(config_map);
    }
    for (key, value) in overrides {
        map.insert(key.clone(), value.clone());
    }
    crate::typst::store::validate_store(&map)?;
    Ok(serde_json::Value::Object(map))
}

fn parse_typst_initializers(
    target_queries: &[String],
) -> Result<serde_json::Map<String, serde_json::Value>> {
    let mut target_maps = Vec::new();
    for query in target_queries {
        let entries: Vec<serde_json::Value> =
            serde_json::from_str(query).context("failed to parse calepin.store.set() metadata")?;
        let mut map = serde_json::Map::new();
        for entry in entries {
            let declaration = entry
                .get("value")
                .and_then(serde_json::Value::as_object)
                .ok_or_else(|| anyhow!("malformed calepin.store.set() metadata"))?;
            let key = declaration
                .get("key")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| anyhow!("calepin.store.set() key must be a string"))?;
            crate::typst::store::validate_key(key)?;
            let value = declaration
                .get("value")
                .cloned()
                .ok_or_else(|| anyhow!("calepin.store.set() is missing its value"))?;
            crate::typst::store::validate_value(&value)?;
            if map.insert(key.to_string(), value).is_some() {
                return Err(anyhow!(
                    "store key `{key}` is initialized more than once with calepin.store.set()"
                ));
            }
        }
        target_maps.push(map);
    }
    let first = target_maps.first().cloned().unwrap_or_default();
    if target_maps.iter().any(|map| map != &first) {
        return Err(anyhow!(
            "calepin.store.set() declarations differ between paged and HTML targets"
        ));
    }
    Ok(first)
}

fn merge_initial_store(
    external: serde_json::Value,
    document: &serde_json::Map<String, serde_json::Value>,
    cli: &[String],
) -> Result<serde_json::Value> {
    let mut store = external.as_object().cloned().unwrap_or_default();
    store.extend(document.clone());
    crate::config::apply_store_overrides(&mut store, cli)?;
    crate::typst::store::validate_store(&store)?;
    Ok(serde_json::Value::Object(store))
}

#[cfg(test)]
fn resolve_vars(
    config: &std::collections::BTreeMap<String, toml::Value>,
    document: &serde_json::Value,
    cli: &[String],
) -> Result<serde_json::Value> {
    let external = resolve_store(config, &serde_json::Map::new())?;
    let empty = serde_json::Map::new();
    merge_initial_store(external, document.as_object().unwrap_or(&empty), cli)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::model::ResultsMode;
    use crate::typst::paths::slash_path;
    use crate::typst::testfixtures;
    use crate::utils::testutil::command_available;

    fn fingerprint_execution<'a>(
        cwd: &'a Path,
        vars: &'a serde_json::Value,
    ) -> ExecutionFingerprintInputs<'a> {
        ExecutionFingerprintInputs {
            cwd,
            timeout: Some(Duration::from_secs(5)),
            store: vars,
        }
    }

    #[test]
    fn page_meta_roundtrips_and_detects_stale_source() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("page.typ");
        fs::write(&input, "= Home\n").unwrap();
        let layout = resolve_layout(&input, None).unwrap();
        let value = serde_json::json!({"title": "Home", "pdf": false});

        write_page_meta(&layout, Some(&value)).unwrap();
        assert_eq!(read_page_meta_in_dir(&input, None, Path::new(CALEPIN_DIR)), Some(value));

        fs::write(&input, "= Changed\n").unwrap();
        assert_eq!(read_page_meta_in_dir(&input, None, Path::new(CALEPIN_DIR)), None);
    }

    #[test]
    fn page_meta_reads_from_a_configured_artifact_dir() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("page.typ");
        fs::write(&input, "= Home\n").unwrap();
        let artifact_dir = Path::new("_calepin");
        let layout = resolve_layout_in_dir(&input, None, artifact_dir).unwrap();
        let value = serde_json::json!({"title": "Home"});

        write_page_meta(&layout, Some(&value)).unwrap();

        assert_eq!(read_page_meta_in_dir(&input, None, Path::new(CALEPIN_DIR)), None);
        assert_eq!(
            read_page_meta_in_dir(&input, None, artifact_dir),
            Some(value)
        );
    }

    #[test]
    fn page_meta_absent_when_document_exposes_none() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("page.typ");
        fs::write(&input, "= Home\n").unwrap();
        let layout = resolve_layout(&input, None).unwrap();

        write_page_meta(&layout, None).unwrap();

        assert_eq!(read_page_meta_in_dir(&input, None, Path::new(CALEPIN_DIR)), None);
    }

    #[test]
    fn cli_store_overrides_document_initializers() {
        let setup = serde_json::json!({"region": "NY", "min_count": 10});
        let resolved = resolve_vars(
            &std::collections::BTreeMap::new(),
            &setup,
            &[
                "store.region=CA".to_string(),
                "store.alpha=0.5".to_string(),
                "store.active=true".to_string(),
            ],
        )
        .unwrap();
        assert_eq!(
            resolved,
            serde_json::json!({"region":"CA","min_count":10,"alpha":0.5,"active":true})
        );
    }

    #[test]
    fn store_merges_config_then_document_then_cli() {
        let config = std::collections::BTreeMap::from([
            (
                "region".to_string(),
                toml::Value::String("config".to_string()),
            ),
            (
                "source".to_string(),
                toml::Value::String("config".to_string()),
            ),
        ]);
        let setup = serde_json::json!({"region": "setup", "doc": "setup"});
        let resolved = resolve_vars(&config, &setup, &["store.region=cli".to_string()]).unwrap();
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
            config_overrides: Vec::new(),
            force: false,
        })
        .unwrap();

        assert_eq!(
            plan.theme,
            crate::theme::ThemeSelection::Builtin("academic")
        );
    }

    #[test]
    fn preprocess_theme_can_come_from_set_override() {
        if !command_available("typst") {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "#set document(title: [Paper])\nHello").unwrap();

        let plan = prepare_preprocess_plan(PreprocessOptions {
            input,
            root: Some(dir.path().to_path_buf()),
            config: None,
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
            config_overrides: vec!["theme=academic".to_string()],
            force: false,
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
            config_overrides: Vec::new(),
            force: false,
        })
        .unwrap();

        let staged_source = std::fs::read_to_string(plan.layout.entry_path("source.typ")).unwrap();
        assert!(staged_source.contains("#import \"/_runtime/calepin.typ\" as calepin"));
        assert_eq!(
            plan.layout.artifact_dir,
            plan.layout.root.join("_runtime/paper")
        );
        let wrapper = std::fs::read_to_string(plan.layout.entry_path("query-wrapper.typ")).unwrap();
        assert!(wrapper.contains("#import \"/_runtime/calepin.typ\""));
        assert!(!wrapper.contains("#import \"/.calepin/calepin.typ\""));
        assert!(!plan.layout.entry_path("wrapper.typ").exists());
        assert!(plan.layout.root.join("_runtime/calepin.typ").is_file());
    }

    #[test]
    fn cli_store_must_be_a_table() {
        let mut store = serde_json::Map::new();
        let err = crate::config::apply_store_overrides(&mut store, &["store=false".to_string()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("store"), "{err}");
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
            fingerprint_execution(dir.path(), &serde_json::json!({})),
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
            fingerprint_execution(dir.path(), &serde_json::json!({})),
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
            fingerprint_execution(dir.path(), &serde_json::json!({"region": "NY"})),
            &crate::theme::ThemeSelection::Default,
            Path::new(CALEPIN_DIR),
            0,
        )
        .unwrap();
        let changed = preprocess_fingerprint(
            &layout,
            &executables,
            &[chunk],
            fingerprint_execution(dir.path(), &serde_json::json!({"region": "CA"})),
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
            fingerprint_execution(dir.path(), &serde_json::json!({})),
            &crate::theme::ThemeSelection::Default,
            Path::new(CALEPIN_DIR),
            0,
        )
        .unwrap();

        let code_changed = preprocess_fingerprint(
            &layout,
            &executables,
            &[test_chunk("print(2)")],
            fingerprint_execution(dir.path(), &serde_json::json!({})),
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
            fingerprint_execution(dir.path(), &serde_json::json!({})),
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
            fingerprint_execution(dir.path(), &serde_json::json!({})),
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
            fingerprint_execution(dir.path(), &serde_json::json!({})),
            &crate::theme::ThemeSelection::Default,
            Path::new(CALEPIN_DIR),
            0,
        )
        .unwrap();
        let changed = preprocess_fingerprint(
            &layout,
            &executables,
            &[chunk],
            fingerprint_execution(dir.path(), &serde_json::json!({})),
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
            fingerprint_execution(dir.path(), &serde_json::json!({})),
            &crate::theme::ThemeSelection::Default,
            Path::new(CALEPIN_DIR),
            0,
        )
        .unwrap();
        let custom_asset_dir = preprocess_fingerprint(
            &layout,
            &executables,
            &[chunk],
            fingerprint_execution(dir.path(), &serde_json::json!({})),
            &crate::theme::ThemeSelection::Default,
            Path::new("_runtime"),
            0,
        )
        .unwrap();
        assert_ne!(default_asset_dir, custom_asset_dir);
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
    fn publish_staged_figures_prunes_stale_files_and_directories() {
        let staged = tempfile::tempdir().unwrap();
        let final_dir = tempfile::tempdir().unwrap();
        let staged_figures = staged.path().join("figures");
        std::fs::create_dir_all(&staged_figures).unwrap();
        std::fs::write(staged_figures.join("current.svg"), "current").unwrap();
        std::fs::write(final_dir.path().join("stale.svg"), "stale").unwrap();
        std::fs::create_dir_all(final_dir.path().join("stale-nested")).unwrap();
        std::fs::write(final_dir.path().join("stale-nested/old.svg"), "old").unwrap();

        publish_staged_figures(&staged_figures, final_dir.path()).unwrap();

        assert!(final_dir.path().join("current.svg").is_file());
        assert!(!final_dir.path().join("stale.svg").exists());
        assert!(!final_dir.path().join("stale-nested").exists());
    }

    #[test]
    fn publish_staged_figures_replaces_mismatched_artifact_types() {
        let staged = tempfile::tempdir().unwrap();
        let final_dir = tempfile::tempdir().unwrap();
        let staged_figures = staged.path().join("figures");
        std::fs::create_dir_all(staged_figures.join("now-dir")).unwrap();
        std::fs::write(staged_figures.join("now-dir/current.svg"), "current").unwrap();
        std::fs::write(staged_figures.join("now-file.svg"), "current").unwrap();
        std::fs::write(final_dir.path().join("now-dir"), "old file").unwrap();
        std::fs::create_dir_all(final_dir.path().join("now-file.svg")).unwrap();

        publish_staged_figures(&staged_figures, final_dir.path()).unwrap();

        assert!(final_dir.path().join("now-dir/current.svg").is_file());
        assert!(final_dir.path().join("now-file.svg").is_file());
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
    fn preprocess_cache_rejects_missing_expansion_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let layout = test_layout(dir.path());
        let fingerprint = 0x2a;
        let chunk = test_chunk("print(1)");
        let results = crate::typst::results::build_results_document(
            &layout.input_rel,
            vec![crate::typst::model::ChunkResultDocument {
                label: chunk.label.clone(),
                engine: chunk.engine.clone(),
                source: chunk.code.clone(),
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

        let mut plan = PreprocessPlan {
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
            force: false,
            display_root: None,
            store: serde_json::json!({}),
            theme: crate::theme::ThemeSelection::Default,
            raw_languages: vec!["python".to_string(), "r".to_string()],
            query_input: PathBuf::new(),
            query_theme: crate::theme::ThemeSelection::Default,
            query_store_path: PathBuf::new(),
            results_input: String::new(),
            store_input: String::new(),
            setup_config: crate::typst::query::SetupConfig::default(),
            initializers: serde_json::Map::new(),
            staged_input: PathBuf::new(),
            runtime_import: String::new(),
            page_meta: None,
        };

        assert!(!preprocess_plan_cache_hit(&mut plan).unwrap());
    }

    #[test]
    fn render_wrapper_rewrites_notebook_theme_runtime_import() {
        let dir = tempfile::tempdir().unwrap();
        let layout = test_layout(dir.path());
        let notebook_theme = crate::theme::NotebookSource {
            source: "#import \"/.calepin/calepin.typ\": _html-themed-raw-block\n".to_string(),
        };

        let wrapper = write_render_wrapper(
            &layout,
            "/_runtime/calepin.typ",
            None,
            &[],
            Some(&notebook_theme),
            None,
        )
        .unwrap();
        let contents = std::fs::read_to_string(dir.path().join(wrapper)).unwrap();

        assert!(contents.contains("#import \"/_runtime/calepin.typ\": _html-themed-raw-block"));
        assert!(!contents.contains("#import \"/.calepin/calepin.typ\": _html-themed-raw-block"));
    }

    #[test]
    fn render_wrapper_keeps_notebook_theme_source_intact() {
        let dir = tempfile::tempdir().unwrap();
        let layout = test_layout(dir.path());
        let notebook_theme = crate::theme::NotebookSource {
            source: "#let notebook-theme-marker = true\n".to_string(),
        };

        let wrapper = write_render_wrapper(
            &layout,
            "/.calepin/calepin.typ",
            None,
            &[],
            Some(&notebook_theme),
            None,
        )
        .unwrap();
        let contents = std::fs::read_to_string(dir.path().join(wrapper)).unwrap();

        assert!(contents.contains("#let notebook-theme-marker = true"));
        assert!(!contents.contains("#include \"/.calepin/paper/source.typ\""));
    }

    #[test]
    fn render_wrapper_does_not_duplicate_template_owned_body() {
        let dir = tempfile::tempdir().unwrap();
        let layout = test_layout(dir.path());
        let notebook_theme = crate::theme::NotebookSource {
            source: "#include \"/.calepin/paper/source.typ\"\n[#emph[Appendix]]\n".to_string(),
        };

        let wrapper = write_render_wrapper(
            &layout,
            "/.calepin/calepin.typ",
            None,
            &[],
            Some(&notebook_theme),
            None,
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
            Some(&staged_input),
            &["bash"],
            None,
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
            // Chunk styling is installed by the generated wrapper, not by the
            // bundles, so a bundle carries only its own layout concerns.
            assert!(!source.source.contains("_default-chunk-chrome"));
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
