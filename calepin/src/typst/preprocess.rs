use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use xxhash_rust::xxh3::xxh3_64;

use crate::config::{CalepinConfig, ExecutablePaths};
use crate::typst::execute::{EnginePool, ExecutionConfig};
use crate::typst::model::{ChunkResultDocument, ChunkSpec, EngineName, ExecOptions, LayoutPaths};
use crate::typst::paths::{artifact_reference, project_relative_path, resolve_layout, slash_path};
use crate::typst::query::{parse_chunks, parse_setup_config};
use crate::typst::results::{build_results_document, write_results};
use crate::typst::runtime::write_runtime;
use crate::typst::sync::write_page_sync;

#[derive(Debug, Clone)]
pub struct PreprocessOptions {
    pub input: PathBuf,
    pub results: Option<PathBuf>,
    pub clean: bool,
    pub quiet: bool,
    pub timeout: Option<u64>,
    pub sync_pages: bool,
}

#[derive(Debug)]
pub struct PreprocessOutput {
    pub layout: LayoutPaths,
    pub executables: ExecutablePaths,
    pub themes_dir: PathBuf,
    pub fingerprint: u64,
}

#[derive(Debug)]
pub struct PreprocessPlan {
    pub layout: LayoutPaths,
    pub executables: ExecutablePaths,
    pub themes_dir: PathBuf,
    pub fingerprint: u64,
    chunks: Vec<ChunkSpec>,
    cwd: PathBuf,
    timeout: Option<Duration>,
    clean: bool,
    quiet: bool,
    sync_pages: bool,
}

pub fn preprocess(options: PreprocessOptions) -> Result<PreprocessOutput> {
    let plan = prepare_preprocess_plan(options)?;
    execute_preprocess_plan(plan)
}

pub fn prepare_preprocess_plan(options: PreprocessOptions) -> Result<PreprocessPlan> {
    let mut layout = resolve_layout(
        &options.input,
        None,
        options.results.as_deref(),
    )?;
    let config = CalepinConfig::load(&layout.root)?;

    write_runtime(&layout.root)?;
    layout.render_input = write_render_wrapper(&layout)?;
    let results_input = artifact_reference(&layout.root, &layout.results_path);
    let setup_json = typst_query(
        &config.executables.typst,
        &layout,
        "<calepin-config>",
        &results_input,
    )?;
    let setup_config = parse_setup_config(&setup_json)?;
    let setup_config = setup_config.unwrap_or_default();
    let chunks_json = typst_query(
        &config.executables.typst,
        &layout,
        "raw.where(block: true).or(<calepin-chunk>)",
        &results_input,
    )?;
    let chunks = parse_chunks(&chunks_json, Some(setup_config.clone()))?;
    let cwd = layout.work_dir.clone();
    let timeout = options.timeout.map(Duration::from_secs);
    let fingerprint = preprocess_fingerprint(&layout, &config.executables, &chunks, &cwd, timeout)?;

    Ok(PreprocessPlan {
        layout,
        executables: config.executables,
        themes_dir: config.themes_dir,
        fingerprint,
        chunks,
        cwd,
        timeout,
        clean: options.clean,
        quiet: options.quiet,
        sync_pages: options.sync_pages,
    })
}

fn write_render_wrapper(layout: &LayoutPaths) -> Result<PathBuf> {
    let mut wrapper_relative = PathBuf::from(".calepin");
    let mut stem = layout.input_rel.clone();
    stem.set_extension("");
    wrapper_relative.push(stem);
    wrapper_relative.push("calepin-wrapper.typ");

    let wrapper = layout.root.join(&wrapper_relative);

    let mut lines = String::from("#import \"/.calepin/calepin.typ\": *\n\n");

    lines.push('\n');
    lines.push('\n');

    let engines: [(&str, &str); 9] = [
        ("python", "python"),
        ("r", "r"),
        ("julia", "julia"),
        ("sh", "sh"),
        ("bash", "sh"),
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

    lines.push_str(&format!(
        "\n#include \"/{}\"\n",
        slash_path(&layout.input_rel)
    ));

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

pub fn execute_preprocess_plan(plan: PreprocessPlan) -> Result<PreprocessOutput> {
    if plan.clean {
        clean_outputs(&plan.layout)?;
    }

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
    };
    let mut pool = EnginePool::new(execution_config);
    let mut chunk_results = Vec::with_capacity(plan.chunks.len());

    if !plan.quiet {
        eprintln!(
            "calepin executing {} chunk{}",
            plan.chunks.len(),
            if plan.chunks.len() == 1 { "" } else { "s" },
        );
    }

    for chunk in &plan.chunks {
        let result = execute_chunk_live(&mut pool, chunk, &staged_figures_dir, &plan.layout)?;
        chunk_results.push(result);
    }

    publish_staged_figures(&staged_figures_dir, &plan.layout.figures_dir)?;
    let document = build_results_document(&plan.layout.input_rel, chunk_results);
    write_results(&plan.layout.results_path, &document)?;
    if plan.sync_pages {
        if let Err(error) = write_page_sync(&plan.executables.typst, &plan.layout, &plan.chunks) {
            if !plan.quiet {
                cwarn!("page sync failed: {}", error);
            }
        }
    }

    if !plan.quiet {
        eprintln!(
            "output saved to {}",
            project_relative_path(&plan.layout.root, &plan.layout.results_path)
        );
    }

    Ok(PreprocessOutput {
        layout: plan.layout,
        executables: plan.executables,
        themes_dir: plan.themes_dir,
        fingerprint: plan.fingerprint,
    })
}

pub fn typst_query(
    typst: &Path,
    layout: &LayoutPaths,
    selector: &str,
    results_input: &str,
) -> Result<String> {
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
        .arg("--input")
        .arg("calepin-target=paged")
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
) -> String {
    let final_path = path
        .strip_prefix(execution_figures_dir)
        .map(|relative| final_figures_dir.join(relative))
        .unwrap_or_else(|_| path.to_path_buf());
    artifact_reference(root, &final_path)
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

fn preprocess_fingerprint(
    layout: &LayoutPaths,
    executables: &ExecutablePaths,
    chunks: &[ChunkSpec],
    cwd: &Path,
    timeout: Option<Duration>,
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
    };
    let bytes = serde_json::to_vec(&payload)?;
    Ok(xxh3_64(&bytes))
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
            engine: chunk.engine,
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
    julia: String,
    shell: String,
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
            julia: path_fingerprint(&paths.julia),
            shell: path_fingerprint(&paths.shell),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::model::{DisplayOptions, ItemSelector, ResultsMode};
    use crate::typst::paths::slash_path;

    #[test]
    fn query_command_uses_root_relative_input() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("paper.typ");
        std::fs::write(&input, "").unwrap();
        let layout = resolve_layout(&input, Some(dir.path()), None).unwrap();
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
        )
        .unwrap();

        assert_eq!(first, second);
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
        )
        .unwrap();

        let code_changed = preprocess_fingerprint(
            &layout,
            &executables,
            &[test_chunk("print(2)")],
            dir.path(),
            Some(Duration::from_secs(5)),
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
        )
        .unwrap();
        assert_ne!(baseline, executables_changed);
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
                format: crate::typst::model::default_format_order(),
                item: ItemSelector::ALL,
                placeholder: true,
                fig_display_width: None,
                fig_display_height: None,
                fig_display_align: None,
                fig_display_responsive: None,
                fig_display_link: None,
                fig_caption: None,
                fig_caption_position: None,
                fig_alt_text: None,
                fig_subcaptions: None,
                fig_layout_columns: None,
                fig_layout_rows: None,
                fig_layout_design: None,
                kind: None,
            },
            ordinal: 0,
        }
    }
}
