use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use xxhash_rust::xxh3::xxh3_64;

use crate::config::{CalepinConfig, ExecutablePaths, PdfTheme};
use crate::typst::execute::{EnginePool, ExecutionConfig};
use crate::typst::model::{ChunkResultDocument, ChunkSpec, EngineName, ExecOptions, LayoutPaths};
use crate::typst::paths::{artifact_reference, project_relative_path, resolve_layout, slash_path};
use crate::typst::query::{parse_chunks_with_warnings, parse_setup_config};
use crate::typst::results::{build_results_document, write_results};
use crate::typst::runtime::write_runtime;
use crate::typst::source_rewrite::write_staged_source;
use crate::typst::sync::write_page_sync;
use crate::typst::version::assert_supported_typst;
use crate::utils::{process, tools};

const PREPROCESS_FINGERPRINT_FILE: &str = "fingerprint.xxh3";
const DEFAULT_PDF_THEME_SOURCE: &str = include_str!("../assets/themes/calepin-pdf.typ");
const TYPST_SNIPPETS: &[(&str, &str)] = &[(
    "code-block.typ",
    include_str!("../assets/snippets/typst/code-block.typ"),
)];

#[derive(Debug, Clone)]
pub struct PreprocessOptions {
    pub input: PathBuf,
    pub config: Option<PathBuf>,
    pub quiet: bool,
    pub timeout: Option<u64>,
    pub sync_pages: bool,
    /// `key=value` document-parameter overrides from the CLI (`-P`).
    pub param_overrides: Vec<String>,
}

#[derive(Debug)]
pub struct PreprocessOutput {
    pub layout: LayoutPaths,
    pub executables: ExecutablePaths,
    pub fingerprint: u64,
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
    params: serde_json::Value,
}

pub fn preprocess(options: PreprocessOptions) -> Result<PreprocessOutput> {
    let plan = prepare_preprocess_plan(options)?;
    execute_preprocess_plan(plan)
}

pub fn preprocess_cached(options: PreprocessOptions) -> Result<PreprocessOutput> {
    let plan = prepare_preprocess_plan(options)?;
    if preprocess_cache_hit(&plan.layout, plan.fingerprint)? {
        if !plan.quiet {
            eprintln!("calepin [cache] {}", display_input(&plan.layout));
        }
        return Ok(PreprocessOutput {
            layout: plan.layout,
            executables: plan.executables,
            fingerprint: plan.fingerprint,
        });
    }
    execute_preprocess_plan(plan)
}

pub fn prepare_preprocess_plan(options: PreprocessOptions) -> Result<PreprocessPlan> {
    let mut layout = resolve_layout(&options.input, None)?;
    let config = CalepinConfig::load(&layout.root, options.config.as_deref())?;
    assert_supported_typst(&config.executables.typst)?;

    write_runtime(&layout.root)?;
    write_typst_snippets(&layout.root)?;
    let staged_input = write_staged_source(&layout)?;
    let query_input = write_render_wrapper(&layout, &staged_input, &[], None)?;
    let results_input = artifact_reference(&layout.root, &layout.results_path);
    let setup_json = typst_query(
        &config.executables.typst,
        &layout,
        &query_input,
        "<calepin-config>",
        &results_input,
    )?;
    let setup_config = parse_setup_config(&setup_json)?;
    let setup_config = setup_config.unwrap_or_default();
    let chunks_json = typst_query(
        &config.executables.typst,
        &layout,
        &query_input,
        "raw.where(block: true).or(<calepin-fence-label>).or(<calepin-chunk>)",
        &results_input,
    )?;
    let parsed_chunks = parse_chunks_with_warnings(&chunks_json, Some(setup_config.clone()))?;
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
    let pdf_theme = load_pdf_theme_source(&config.pdf_theme)?;
    if !jupyter_kernels.is_empty() {
        let kernels: Vec<&str> = jupyter_kernels.into_iter().collect();
        layout.render_input =
            write_render_wrapper(&layout, &staged_input, &kernels, pdf_theme.as_deref())?;
    } else {
        layout.render_input =
            write_render_wrapper(&layout, &staged_input, &[], pdf_theme.as_deref())?;
    }

    let params = resolve_params(&setup_config.defaults.params, &options.param_overrides)?;

    let cwd = layout.work_dir.clone();
    let timeout = options.timeout.map(Duration::from_secs);
    let fingerprint = preprocess_fingerprint(
        &layout,
        &config.executables,
        &chunks,
        &cwd,
        timeout,
        &params,
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
        params,
    })
}

fn write_render_wrapper(
    layout: &LayoutPaths,
    include_input: &Path,
    jupyter_kernels: &[&str],
    pdf_theme: Option<&str>,
) -> Result<PathBuf> {
    let mut wrapper_relative = PathBuf::from(".calepin");
    let mut stem = layout.input_rel.clone();
    stem.set_extension("");
    wrapper_relative.push(stem);
    wrapper_relative.push("calepin-wrapper.typ");

    let wrapper = layout.root.join(&wrapper_relative);

    let mut lines = String::from("#import \"/.calepin/calepin.typ\": *\n\n");

    lines.push('\n');
    lines.push('\n');

    for lang in ["typ", "typst"] {
        lines.push_str(&format!(
            "#show raw.where(block: true, lang: \"{lang}\", theme: auto): it => _without-raw-chunk-transforms(() => it)\n"
        ));
    }

    let engines: [(&str, &str); 6] = [
        ("python", "python"),
        ("r", "r"),
        ("mermaid", "mermaid"),
        ("dot", "dot"),
        ("tikz", "tikz"),
        ("d2", "d2"),
    ];

    for (lang, engine) in &engines {
        lines.push_str(&format!(
            "#show raw.where(block: true, lang: \"{}\", theme: auto): it => if _disable-raw-chunk-transforms.get() {{ it }} else {{ chunk-from-raw-plain(\"{}\", it) }}\n",
            lang, engine
        ));
    }

    for kernel in jupyter_kernels {
        lines.push_str(&format!(
            "#show raw.where(block: true, lang: \"{kernel}\", theme: auto): it => if _disable-raw-chunk-transforms.get() {{ it }} else {{ chunk-from-raw-plain(\"{kernel}\", it) }}\n"
        ));
    }

    if let Some(pdf_theme) = pdf_theme {
        lines.push_str("\n// PDF theme\n");
        lines.push_str(pdf_theme);
        if !pdf_theme.ends_with('\n') {
            lines.push('\n');
        }
    }

    lines.push_str(&format!("\n#include \"/{}\"\n", slash_path(include_input)));

    if let Some(parent) = wrapper.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if fs::read_to_string(&wrapper).is_ok_and(|existing| existing == lines) {
        return Ok(wrapper_relative);
    }

    fs::write(&wrapper, lines).with_context(|| format!("failed to write {}", wrapper.display()))?;
    Ok(wrapper_relative)
}

fn load_pdf_theme_source(pdf_theme: &PdfTheme) -> Result<Option<String>> {
    match pdf_theme {
        PdfTheme::Disabled => Ok(None),
        PdfTheme::Default => Ok(Some(DEFAULT_PDF_THEME_SOURCE.to_string())),
        PdfTheme::Path(path) => {
            let source = fs::read_to_string(path)
                .with_context(|| format!("failed to read pdf_theme {}", path.display()))?;
            Ok(Some(source))
        }
    }
}

fn write_typst_snippets(root: &Path) -> Result<()> {
    let dir = root.join(".calepin/snippets/typst");
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    for (name, source) in TYPST_SNIPPETS {
        let path = dir.join(name);
        if fs::read_to_string(&path).is_ok_and(|existing| existing == *source) {
            continue;
        }
        fs::write(&path, source).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
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
            display_input(&plan.layout),
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
    })
}

pub fn typst_query(
    typst: &Path,
    layout: &LayoutPaths,
    input: &Path,
    selector: &str,
    results_input: &str,
) -> Result<String> {
    process::validate_executable(typst, "run typst query", Some(&tools::TYPST))?;
    let output = Command::new(typst)
        .arg("query")
        .arg(input)
        .arg(selector)
        .arg("--root")
        .arg(&layout.root)
        .arg("--input")
        .arg("calepin-mode=query")
        .arg("--input")
        .arg(format!("calepin-results={results_input}"))
        .arg("--input")
        .arg("calepin-target=paged")
        .current_dir(&layout.root)
        .output()
        .map_err(|error| {
            process::spawn_error(typst, "run typst query", error, Some(&tools::TYPST))
        })?;

    if !output.status.success() {
        return Err(anyhow!(
            "typst query {} failed:\n{}",
            selector,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    String::from_utf8(output.stdout).context("typst query output was not UTF-8")
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

fn display_input(layout: &LayoutPaths) -> String {
    project_relative_path(&layout.root, &layout.input)
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
    if std::fs::read(target).is_ok_and(|existing| existing == bytes) {
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(target, bytes).with_context(|| format!("failed to write {}", target.display()))
}

/// Write `params.json` next to `results.json` when there are parameters, and
/// return its path. Returns `None` (and removes any stale file) when empty.
fn write_params_file(layout: &LayoutPaths, params: &serde_json::Value) -> Result<Option<PathBuf>> {
    let path = layout.results_path.with_file_name("params.json");
    let is_empty = params.as_object().is_none_or(|map| map.is_empty());
    if is_empty {
        let _ = fs::remove_file(&path);
        return Ok(None);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(params)?;
    fs::write(&path, json).with_context(|| format!("failed to write {}", path.display()))?;
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

fn preprocess_fingerprint(
    layout: &LayoutPaths,
    executables: &ExecutablePaths,
    chunks: &[ChunkSpec],
    cwd: &Path,
    timeout: Option<Duration>,
    params: &serde_json::Value,
) -> Result<u64> {
    let payload = PreprocessFingerprint {
        schema: crate::typst::model::RESULT_SCHEMA_VERSION,
        calepin_version: env!("CARGO_PKG_VERSION"),
        input_rel: path_fingerprint(&layout.input_rel),
        figures_dir: path_fingerprint(&layout.figures_dir),
        cwd: path_fingerprint(cwd),
        timeout_secs: timeout.map(|duration| duration.as_secs()),
        executables: ExecutableFingerprint::from(executables),
        chunks: chunks
            .iter()
            .map(ChunkFingerprint::from)
            .collect::<Vec<_>>(),
        params: params.clone(),
    };
    let bytes = serde_json::to_vec(&payload)?;
    Ok(xxh3_64(&bytes))
}

fn preprocess_cache_hit(layout: &LayoutPaths, fingerprint: u64) -> Result<bool> {
    if !layout.results_path.is_file() {
        return Ok(false);
    }
    Ok(read_preprocess_fingerprint(layout)? == Some(fingerprint))
}

fn preprocess_fingerprint_path(layout: &LayoutPaths) -> PathBuf {
    layout
        .results_path
        .with_file_name(PREPROCESS_FINGERPRINT_FILE)
}

fn read_preprocess_fingerprint(layout: &LayoutPaths) -> Result<Option<u64>> {
    let path = preprocess_fingerprint_path(layout);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()))
        }
    };
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    Ok(u64::from_str_radix(text, 16).ok())
}

fn write_preprocess_fingerprint(layout: &LayoutPaths, fingerprint: u64) -> Result<()> {
    let path = preprocess_fingerprint_path(layout);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = format!("{fingerprint:016x}\n");
    if fs::read_to_string(&path).is_ok_and(|existing| existing == text) {
        return Ok(());
    }
    fs::write(&path, text).with_context(|| format!("failed to write {}", path.display()))
}

#[derive(Serialize)]
struct PreprocessFingerprint {
    schema: u8,
    calepin_version: &'static str,
    input_rel: String,
    figures_dir: String,
    cwd: String,
    timeout_secs: Option<u64>,
    executables: ExecutableFingerprint,
    chunks: Vec<ChunkFingerprint>,
    params: serde_json::Value,
}

#[derive(Serialize)]
struct ChunkFingerprint {
    label: String,
    ordinal: usize,
    engine: EngineName,
    code: String,
    exec_options: ExecOptions,
}

impl From<&ChunkSpec> for ChunkFingerprint {
    fn from(chunk: &ChunkSpec) -> Self {
        Self {
            label: chunk.label.clone(),
            ordinal: chunk.ordinal,
            engine: chunk.engine.clone(),
            code: chunk.code.clone(),
            exec_options: chunk.exec_options.clone(),
        }
    }
}

#[derive(Serialize)]
struct ExecutableFingerprint {
    typst: String,
    rscript: String,
    python: String,
    mmdc: String,
    dot: String,
    tectonic: String,
    dvisvgm: String,
    pdf2svg: String,
    d2: String,
    chrome: Option<String>,
}

impl From<&ExecutablePaths> for ExecutableFingerprint {
    fn from(paths: &ExecutablePaths) -> Self {
        Self {
            typst: path_fingerprint(&paths.typst),
            rscript: path_fingerprint(&paths.rscript),
            python: path_fingerprint(&paths.python),
            mmdc: path_fingerprint(&paths.mmdc),
            dot: path_fingerprint(&paths.dot),
            tectonic: path_fingerprint(&paths.tectonic),
            dvisvgm: path_fingerprint(&paths.dvisvgm),
            pdf2svg: path_fingerprint(&paths.pdf2svg),
            d2: path_fingerprint(&paths.d2),
            chrome: paths.chrome.as_deref().map(path_fingerprint),
        }
    }
}

fn path_fingerprint(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::model::{DisplayOptions, ResultsMode};
    use crate::typst::paths::slash_path;

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
        )
        .unwrap();
        let changed = preprocess_fingerprint(
            &layout,
            &executables,
            &[chunk],
            dir.path(),
            Some(Duration::from_secs(5)),
            &serde_json::json!({"region": "CA"}),
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
        )
        .unwrap();

        let code_changed = preprocess_fingerprint(
            &layout,
            &executables,
            &[test_chunk("print(2)")],
            dir.path(),
            Some(Duration::from_secs(5)),
            &serde_json::json!({}),
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
        )
        .unwrap();
        assert_ne!(baseline, executables_changed);
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
    fn render_wrapper_includes_pdf_theme_before_source() {
        let dir = tempfile::tempdir().unwrap();
        let layout = test_layout(dir.path());
        let staged_input = PathBuf::from(".calepin/paper/source.typ");
        let pdf_theme = "#let pdf-theme-marker = true\n";

        let wrapper = write_render_wrapper(&layout, &staged_input, &[], Some(pdf_theme)).unwrap();
        let contents = std::fs::read_to_string(dir.path().join(wrapper)).unwrap();

        let theme_marker = contents.find("#let pdf-theme-marker = true").unwrap();
        let source_include = contents
            .find("#include \"/.calepin/paper/source.typ\"")
            .unwrap();
        assert!(theme_marker < source_include);
    }

    #[test]
    fn default_pdf_theme_source_is_loaded() {
        let source = load_pdf_theme_source(&PdfTheme::Default).unwrap().unwrap();

        assert_eq!(source, DEFAULT_PDF_THEME_SOURCE);
    }

    #[test]
    fn typst_snippets_are_staged_under_calepin_dir() {
        let dir = tempfile::tempdir().unwrap();

        write_typst_snippets(dir.path()).unwrap();

        assert_eq!(
            std::fs::read_to_string(dir.path().join(".calepin/snippets/typst/code-block.typ"))
                .unwrap(),
            TYPST_SNIPPETS[0].1
        );
    }

    #[test]
    fn custom_pdf_theme_source_is_loaded() {
        let theme_dir = tempfile::tempdir().unwrap();
        let theme = theme_dir.path().join("pdf.typ");
        std::fs::write(&theme, "#let custom = true\n").unwrap();

        let source = load_pdf_theme_source(&PdfTheme::Path(theme))
            .unwrap()
            .unwrap();

        assert_eq!(source, "#let custom = true\n");
    }

    fn test_layout(root: &Path) -> LayoutPaths {
        LayoutPaths {
            root: root.to_path_buf(),
            input: root.join("paper.typ"),
            input_rel: PathBuf::from("paper.typ"),
            render_input: PathBuf::from("paper.typ"),
            work_dir: root.to_path_buf(),
            results_path: root.join(".calepin/paper/results.json"),
            figures_dir: root.join(".calepin/paper/figures"),
        }
    }

    fn test_chunk(code: &str) -> ChunkSpec {
        ChunkSpec {
            label: "answer".to_string(),
            engine: EngineName::Python,
            code: code.to_string(),
            exec_options: ExecOptions {
                eval: true,
                error: false,
                fig_device_format: "svg".to_string(),
                fig_device_dpi: 150,
                fig_device_width: 6.0,
                fig_device_height: None,
                fig_device_aspect: 0.618,
            },
            display_options: DisplayOptions {
                echo: true,
                output: true,
                results: ResultsMode::Verbatim,
                warning: true,
                message: true,
                placeholder: true,
                fig_width: None,
                fig_height: None,
                fig_align: None,
                fig_responsive: None,
                fig_link: None,
                fig_caption: None,
                fig_cap_location: None,
                fig_alt_text: None,
                fig_subcaptions: None,
                fig_layout_columns: None,
                fig_layout_rows: None,
                kind: None,
            },
            ordinal: 0,
            crossref_labels: vec![],
        }
    }
}
