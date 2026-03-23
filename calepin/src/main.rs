#[macro_use]
mod cli;
mod engines;
mod filters;
mod formats;
mod parse;
mod plugin_manifest;
mod preview;
mod registry;
mod render;
mod site;
mod structures;
mod jinja_engine;
mod paths;
mod project;
#[allow(dead_code)]
mod tools;
mod types;
mod util;
mod value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;

use cli::{Cli, Command, RenderArgs, PreviewArgs, InfoAction};
use render::elements::ElementRenderer;
use engines::r::RSession;
use engines::python::PythonSession;
use engines::EngineContext;
use engines::cache::CacheState;

/// Resolved project context: project config + target, shared by render and preview.
struct ProjectContext {
    project_root: Option<PathBuf>,
    project_config: Option<project::ProjectConfig>,
    target_name: String,
    target: project::Target,
    /// True when the target was explicitly set (CLI flag or front matter),
    /// false when it fell back to the default "html".
    explicit_target: bool,
}

impl ProjectContext {
    /// Get the project-level `[var]` table, if any.
    fn project_var(&self) -> Option<&toml::Value> {
        self.project_config.as_ref().and_then(|c| c.var.as_ref())
    }

    /// Get the configured output directory, if any.
    fn output_dir(&self) -> Option<&str> {
        self.project_config.as_ref().and_then(|c| c.output.as_deref())
    }
}

/// Resolve project config and target from an input file and optional CLI target flag.
/// Falls back to front matter `target:`, then "html".
fn resolve_context(input: &Path, cli_target: Option<&str>) -> Result<ProjectContext> {
    let input_dir = input.parent().unwrap_or(Path::new("."));
    let abs_input_dir = if input_dir.is_relative() {
        std::env::current_dir().unwrap_or_default().join(input_dir)
    } else {
        input_dir.to_path_buf()
    };

    let project_root = project::find_project_root(&abs_input_dir);
    let project_config = project_root.as_ref().and_then(|root| {
        let cfg_path = project::config_path(root)?;
        match project::load_project_config(&cfg_path) {
            Ok(config) => Some(config),
            Err(e) => {
                eprintln!("Warning: failed to load {}: {}", cfg_path.display(), e);
                None
            }
        }
    });

    // Target name: CLI flag -> front matter -> "html"
    let (target_name, explicit_target) = if let Some(name) = cli_target {
        (name.to_string(), true)
    } else {
        // Read front matter to check for target:
        if let Ok(text) = fs::read_to_string(input) {
            if let Ok((meta, _)) = parse::yaml::split_yaml(&text) {
                match meta.target {
                    Some(t) => (t, true),
                    None => ("html".to_string(), false),
                }
            } else {
                ("html".to_string(), false)
            }
        } else {
            ("html".to_string(), false)
        }
    };

    let target = project::resolve_target(&target_name, project_config.as_ref())?;

    Ok(ProjectContext {
        project_root,
        project_config,
        target_name,
        target,
        explicit_target,
    })
}

/// Parse CLI args, injecting "render" as default subcommand when the first
/// positional argument looks like a file path rather than a known subcommand.
fn parse_cli() -> Cli {
    let args: Vec<String> = std::env::args().collect();

    let known = ["render", "preview", "flush", "init", "new", "info"];

    let needs_inject = args.get(1).map_or(false, |arg| {
        // Don't inject for flags (--help, -v, etc.)
        if arg.starts_with('-') {
            return false;
        }
        // If it's not a known subcommand, assume it's a file path → inject "render"
        !known.contains(&arg.as_str())
    });

    if needs_inject {
        let mut patched = vec![args[0].clone(), "render".to_string()];
        patched.extend_from_slice(&args[1..]);
        Cli::parse_from(patched)
    } else {
        Cli::parse()
    }
}

fn main() -> Result<()> {
    let cli = parse_cli();

    match cli.command {
        Command::Render(args) => handle_render(args),
        Command::Preview(args) => handle_preview(args),
        Command::Flush { path, yes } => handle_flush(&path, yes),
        Command::Init { template } => {
            eprintln!("Project init (template: {}) is not yet implemented.", template);
            Ok(())
        }
        Command::New { action } => handle_new(action),
        Command::Info { action } => handle_info(action),
    }
}

fn handle_flush(path: &Path, skip_confirm: bool) -> Result<()> {
    use std::io::Write;

    let root = if path.is_relative() {
        std::env::current_dir()?.join(path)
    } else {
        path.to_path_buf()
    };

    // Collect directories and files to delete
    let mut targets: Vec<PathBuf> = Vec::new();
    let latex_exts = ["aux", "log", "out", "toc", "fls", "fdb_latexmk", "synctex.gz", "xdv"];

    // Walk recursively to find _calepin_cache/_calepin_files dirs and LaTeX artefacts
    fn find_targets(dir: &Path, targets: &mut Vec<PathBuf>, latex_exts: &[&str]) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let name = entry.file_name();
                if name == "_calepin_cache" || name == "_calepin_files" {
                    targets.push(p);
                } else if name != "." && name != ".." && name != ".git" && name != "node_modules" {
                    find_targets(&p, targets, latex_exts);
                }
            } else if p.is_file() {
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if latex_exts.contains(&ext) {
                        targets.push(p);
                    }
                }
            }
        }
    }
    find_targets(&root, &mut targets, &latex_exts);

    if targets.is_empty() {
        eprintln!("Nothing to clean.");
        return Ok(());
    }

    // Show what will be deleted
    for t in &targets {
        let display = t.strip_prefix(&root).unwrap_or(t);
        if t.is_dir() {
            eprintln!("  rm -rf {}/", display.display());
        } else {
            eprintln!("  rm {}", display.display());
        }
    }

    // Confirm
    if !skip_confirm {
        eprint!("\nDelete these? [y/N] ");
        std::io::stderr().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            eprintln!("Cancelled.");
            return Ok(());
        }
    }

    // Delete
    for t in &targets {
        if t.is_dir() {
            std::fs::remove_dir_all(t)?;
        } else {
            std::fs::remove_file(t)?;
        }
    }

    eprintln!("Done.");
    Ok(())
}

fn handle_render(args: RenderArgs) -> Result<()> {
    cli::set_quiet(args.quiet);
    let mut overrides = args.overrides;
    if args.no_highlight {
        overrides.push("highlight-style=none".to_string());
    }

    // Site mode: single .toml config with [site] section, or legacy .yaml manifest
    if args.input.len() == 1 && cli::is_site_config(&args.input[0]) {
        let output = args.output.unwrap_or_else(|| PathBuf::from("output"));
        return site::build_site(Some(args.input[0].as_path()), &output, args.clean, args.quiet, args.target.as_deref());
    }

    // Single file: may use -o as output file path
    if args.input.len() == 1 {
        // Multi-target: split comma-separated targets and render each
        if let Some(ref target_str) = args.target {
            if target_str.contains(',') {
                let targets: Vec<&str> = target_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
                for t in &targets {
                    render_one(&args.input[0], None, Some(t), &overrides, args.quiet)?;
                }
                return Ok(());
            }
        }
        return render_one(&args.input[0], args.output.as_deref(), args.target.as_deref(), &overrides, args.quiet);
    }

    // Multiple files: render in parallel.
    // -o is an output directory (if given); target/overrides/quiet apply to all.
    if let Some(ref out_dir) = args.output {
        std::fs::create_dir_all(out_dir)
            .with_context(|| format!("Failed to create output directory: {}", out_dir.display()))?;
    }

    // Resolve project context once and share it across all files.
    // Previously each file called resolve_context() independently, which
    // re-parsed calepin.toml on every thread (~27% of batch time in profiles).
    let ctx = resolve_context(&args.input[0], args.target.as_deref())?;

    let output_ext = args.output.as_ref().map(|dir| {
        (dir.clone(), ctx.target.output_extension().to_string())
    });

    use rayon::prelude::*;

    let errors: Vec<String> = args.input
        .par_iter()
        .filter_map(|input| {
            let file_output = output_ext.as_ref().map(|(dir, ext)| {
                dir.join(input.file_name().unwrap()).with_extension(ext)
            });
            match render_one_with_context(input, file_output.as_deref(), &ctx, &overrides, args.quiet) {
                Ok(()) => None,
                Err(e) => Some(format!("{:#}", e)),
            }
        })
        .collect();

    if !errors.is_empty() {
        for e in &errors {
            eprintln!("\x1b[31mError:\x1b[0m {}", e);
        }
        anyhow::bail!("{} of {} files failed to render", errors.len(), args.input.len());
    }

    Ok(())
}

/// Render a single .qmd file.
fn render_one(
    input: &Path,
    output: Option<&Path>,
    target: Option<&str>,
    overrides: &[String],
    quiet: bool,
) -> Result<()> {
    let ctx = resolve_context(input, target)?;
    render_one_with_context(input, output, &ctx, overrides, quiet)
}

/// Render a single .qmd file with a pre-resolved project context.
fn render_one_with_context(
    input: &Path,
    output: Option<&Path>,
    ctx: &ProjectContext,
    overrides: &[String],
    quiet: bool,
) -> Result<()> {
    let (output_path, final_output, renderer) = render_file(
        input,
        output,
        Some(&ctx.target_name),
        overrides,
        Some(&ctx.target),
        ctx.project_root.as_deref(),
        ctx.project_var(),
        if ctx.explicit_target { ctx.output_dir() } else { None },
    )?;

    renderer.write_output(&final_output, &output_path)?;

    if !quiet {
        eprintln!("-> {}", output_path.display());
    }

    if let Some(ref compile_cfg) = ctx.target.compile {
        run_compile_step(&output_path, compile_cfg, quiet)?;
    }

    Ok(())
}

/// Run a target's compile step.
pub fn run_compile_step(
    rendered_path: &Path,
    compile_cfg: &project::CompileConfig,
    quiet: bool,
) -> Result<()> {
    let command = compile_cfg.command.as_deref()
        .ok_or_else(|| anyhow::anyhow!("Target compile section has no command"))?;
    let compile_ext = compile_cfg.extension.as_deref()
        .ok_or_else(|| anyhow::anyhow!("Target compile section has no extension"))?;

    let output_path = rendered_path.with_extension(compile_ext);
    let cmd = command
        .replace("{input}", &rendered_path.to_string_lossy())
        .replace("{output}", &output_path.to_string_lossy());

    if !quiet {
        eprintln!("  compiling: {}", cmd);
    }

    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .status()
        .with_context(|| format!("Failed to run compile command: {}", cmd))?;

    if !status.success() {
        anyhow::bail!("Compile command failed: {}", cmd);
    }

    if !quiet {
        eprintln!("→ {}", output_path.display());
    }

    Ok(())
}

fn handle_preview(args: PreviewArgs) -> Result<()> {
    // Directory: serve it over HTTP
    if args.input.is_dir() {
        return site::serve(&args.input, args.port);
    }
    // Project manifest: build, serve with live-reload, and watch for changes
    if cli::is_site_config(&args.input) {
        // For non-HTML targets, do a one-shot build and open the output.
        let is_html = {
            let target_name = args.target.as_deref().unwrap_or("html");
            let config = project::load_project_config(&args.input)?;
            let target = project::resolve_target(target_name, Some(&config))?;
            target.base == "html"
        };
        if !is_html {
            let output = PathBuf::from("output");
            site::build_site(Some(args.input.as_path()), &output, true, false, args.target.as_deref())?;
            let pdf = output.join("book.pdf");
            if pdf.exists() {
                eprintln!("Opening {}", pdf.display());
                let _ = open::that(&pdf);
            }
            return Ok(());
        }

        return preview::run_site(&args.input, &args);
    }
    // Resolve target using the same path as render
    let ctx = resolve_context(&args.input, args.target.as_deref())?;
    preview::run(&args.input, &args, &ctx.target_name, &ctx.target)
}


fn handle_new(action: cli::NewAction) -> Result<()> {
    match action {
        cli::NewAction::Gibberish { files, paragraphs, dir, complexity } => {
            generate_gibberish(&dir, files, paragraphs, complexity)
        }
    }
}

fn generate_gibberish(
    dir: &std::path::Path,
    num_files: usize,
    num_paragraphs: usize,
    complexity: u8,
) -> Result<()> {
    std::fs::create_dir_all(dir)
        .with_context(|| format!("Failed to create directory: {}", dir.display()))?;

    let code_chunks = complexity >= 1;
    let extras = complexity >= 2;

    if extras {
        let bib = generate_library_bib();
        std::fs::write(dir.join("library.bib"), bib)
            .context("Failed to write library.bib")?;
    }

    for i in 0..num_files {
        let filename = format!("page-{:03}.qmd", i + 1);
        let path = dir.join(&filename);

        let title = jinja_engine::lipsum_words(3 + (i * 7) % 4);
        let mut body = String::new();

        // Distribute paragraphs across 5 headings with 3 subheadings each
        let section_ids: Vec<String> = (0..5)
            .map(|h| format!("sec-{}-{}", i + 1, h + 1))
            .collect();
        let heading_titles: Vec<String> = (0..5)
            .map(|h| jinja_engine::lipsum_words(2 + ((i + h) * 5) % 4))
            .collect();
        let sub_titles: Vec<String> = (0..15)
            .map(|s| jinja_engine::lipsum_words(2 + ((i + s) * 3) % 4))
            .collect();

        let mut para_idx = 0;
        let mut table_idx = 0usize;
        let paras_per_section = num_paragraphs / 5;

        for h in 0..5 {
            if extras {
                body.push_str(&format!(
                    "## {} {{#{}}} \n\n",
                    heading_titles[h], section_ids[h]
                ));
            } else {
                body.push_str(&format!("## {}\n\n", heading_titles[h]));
            }

            let paras_before_subs = paras_per_section / 4;
            for _ in 0..paras_before_subs {
                body.push_str(&jinja_engine::lipsum_paragraphs(1));
                body.push_str("\n\n");
                para_idx += 1;
            }

            let paras_per_sub = (paras_per_section - paras_before_subs) / 3;
            for s in 0..3 {
                let sub_idx = h * 3 + s;
                body.push_str(&format!("### {}\n\n", sub_titles[sub_idx]));
                let count = if s == 2 {
                    paras_per_section - paras_before_subs - paras_per_sub * 2
                } else {
                    paras_per_sub
                };
                for _ in 0..count {
                    // Vary the lipsum by using different offsets
                    let offset = para_idx * 17 + i * 31;
                    let sentence_count = 3 + (offset % 4);
                    let mut sentences = Vec::new();
                    for j in 0..sentence_count {
                        let len = 8 + ((offset + j * 5) % 10);
                        sentences.push(jinja_engine::lipsum_sentence(len, offset + j * 11));
                    }

                    if extras {
                        // Inject footnotes, cross-refs, and citations into the paragraph
                        let mut para_text = sentences.join(" ");
                        let seed = para_idx * 7 + i * 13;

                        // Footnote every ~5th paragraph
                        if para_idx % 5 == 1 {
                            let fn_text = jinja_engine::lipsum_sentence(
                                6 + (seed % 5),
                                seed + 3,
                            );
                            para_text.push_str(&format!("^[{}]", fn_text));
                        }

                        // Citation every ~3rd paragraph
                        if para_idx % 3 == 0 {
                            let cite_key = BIB_KEYS[seed % BIB_KEYS.len()];
                            // Vary between inline @key and parenthetical [@key]
                            if seed % 2 == 0 {
                                para_text = format!(
                                    "As shown by @{}, {}",
                                    cite_key, para_text
                                );
                            } else {
                                para_text.push_str(&format!(" [@{}]", cite_key));
                            }
                        }

                        // Cross-ref to another section every ~7th paragraph
                        if para_idx % 7 == 3 {
                            let ref_sec = &section_ids[(h + 1) % 5];
                            para_text.push_str(&format!(
                                " See also @{}.",
                                ref_sec
                            ));
                        }

                        body.push_str(&para_text);
                    } else {
                        body.push_str(&sentences.join(" "));
                    }
                    body.push_str("\n\n");

                    // Insert a code chunk every 4th paragraph, alternating R and Python
                    if code_chunks && para_idx % 4 == 2 {
                        if para_idx % 8 < 4 {
                            body.push_str(&build_r_chunk(offset));
                        } else {
                            body.push_str(&build_python_chunk(offset));
                        }
                        body.push('\n');
                    }

                    // Insert a table every ~10th paragraph (complexity 2 only)
                    if extras && para_idx % 10 == 6 {
                        table_idx += 1;
                        body.push_str(&build_gibberish_table(i, table_idx));
                        body.push('\n');
                    }

                    para_idx += 1;
                }
            }
        }

        // Any remaining paragraphs
        while para_idx < num_paragraphs {
            let offset = para_idx * 17 + i * 31;
            let sentence_count = 3 + (offset % 4);
            let mut sentences = Vec::new();
            for j in 0..sentence_count {
                let len = 8 + ((offset + j * 5) % 10);
                sentences.push(jinja_engine::lipsum_sentence(len, offset + j * 11));
            }
            body.push_str(&sentences.join(" "));
            body.push_str("\n\n");
            para_idx += 1;
        }

        let front_matter = if extras {
            format!(
                "---\ntitle: \"{}\"\nbibliography: library.bib\n---",
                title
            )
        } else {
            format!("---\ntitle: \"{}\"\n---", title)
        };

        let content = format!("{}\n\n{}", front_matter, body.trim_end());
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write {}", path.display()))?;
    }

    let desc = match complexity {
        0 => "prose only",
        1 => "prose + code chunks",
        _ => "prose + code chunks + cross-refs/footnotes/citations/tables",
    };
    eprintln!(
        "Generated {} files in {}/ ({} paragraphs, 5 sections x 3 subsections, {})",
        num_files, dir.display(), num_paragraphs, desc
    );
    Ok(())
}

const R_SNIPPETS: &[&str] = &[
    "library(ggplot2)\n\ndat <- data.frame(\n  x = rnorm(100),\n  y = rnorm(100),\n  group = sample(letters[1:3], 100, replace = TRUE)\n)\n\nggplot(dat, aes(x, y, color = group)) +\n  geom_point(size = 2) +\n  theme_minimal()",
    "fit <- lm(mpg ~ wt + hp + factor(cyl), data = mtcars)\nsummary(fit)\nconfint(fit)",
    "library(dplyr)\n\nmtcars |>\n  group_by(cyl) |>\n  summarise(\n    mean_mpg = mean(mpg),\n    sd_mpg = sd(mpg),\n    n = n()\n  ) |>\n  arrange(desc(mean_mpg))",
    "x <- seq(-2 * pi, 2 * pi, length.out = 200)\nplot(x, sin(x), type = \"l\", col = \"steelblue\", lwd = 2)\nlines(x, cos(x), col = \"coral\", lwd = 2)\nlegend(\"topright\", c(\"sin\", \"cos\"), col = c(\"steelblue\", \"coral\"), lwd = 2)",
    "mat <- matrix(rnorm(20), nrow = 4)\ncolnames(mat) <- paste0(\"V\", 1:5)\nrownames(mat) <- paste0(\"obs\", 1:4)\nknitr::kable(mat, digits = 2)",
];

const PYTHON_SNIPPETS: &[&str] = &[
    "import numpy as np\nimport matplotlib.pyplot as plt\n\nx = np.linspace(0, 10, 100)\ny = np.sin(x) * np.exp(-x / 5)\n\nplt.figure(figsize=(8, 4))\nplt.plot(x, y, linewidth=2)\nplt.xlabel(\"x\")\nplt.ylabel(\"f(x)\")\nplt.title(\"Damped oscillation\")\nplt.show()",
    "import pandas as pd\n\ndf = pd.DataFrame({\n    \"name\": [\"Alice\", \"Bob\", \"Charlie\", \"Diana\"],\n    \"score\": [92, 87, 78, 95],\n    \"grade\": [\"A\", \"B\", \"C\", \"A\"],\n})\ndf.describe()",
    "from collections import Counter\n\nwords = \"the quick brown fox jumps over the lazy dog\".split()\ncounts = Counter(words)\nfor word, count in counts.most_common(5):\n    print(f\"{word}: {count}\")",
    "import numpy as np\n\nA = np.array([[1, 2], [3, 4]])\nb = np.array([5, 6])\nx = np.linalg.solve(A, b)\nprint(f\"Solution: {x}\")\nprint(f\"Verify: {A @ x}\")",
    "def fibonacci(n):\n    a, b = 0, 1\n    for _ in range(n):\n        yield a\n        a, b = b, a + b\n\nlist(fibonacci(10))",
];

fn build_r_chunk(seed: usize) -> String {
    let snippet = R_SNIPPETS[seed % R_SNIPPETS.len()];
    format!("```r\n{}\n```\n", snippet)
}

fn build_python_chunk(seed: usize) -> String {
    let snippet = PYTHON_SNIPPETS[seed % PYTHON_SNIPPETS.len()];
    format!("```python\n{}\n```\n", snippet)
}

const BIB_KEYS: &[&str] = &[
    "lorem2019", "consectetur2021", "veniam2020", "fugiat2022",
    "blanditiis2023", "repellendus2018", "sapiente2020", "asperiores2024",
];

fn generate_library_bib() -> String {
    r#"@article{lorem2019,
  author  = {Lorem, Ipsum and Dolor, Sit Amet},
  title   = {On the Convergence of Adipiscing Processes in Elit Manifolds},
  journal = {Journal of Gibberish Studies},
  year    = {2019},
  volume  = {42},
  number  = {3},
  pages   = {217--234},
}

@book{consectetur2021,
  author    = {Consectetur, Adipiscing},
  title     = {Foundations of Eiusmod Tempor Theory},
  publisher = {Incididunt University Press},
  year      = {2021},
  edition   = {2nd},
}

@inproceedings{veniam2020,
  author    = {Veniam, Quis and Nostrud, Exercitation and Ullamco, Laboris},
  title     = {Aliquip Methods for Commodo Consequat Estimation},
  booktitle = {Proceedings of the International Conference on Duis Aute},
  year      = {2020},
  pages     = {112--119},
}

@article{fugiat2022,
  author  = {Fugiat, Nulla and Pariatur, Excepteur},
  title   = {Occaecat Cupidatat and Non-Proident Structures: A Review},
  journal = {Annals of Culpa Officia},
  year    = {2022},
  volume  = {15},
  pages   = {88--107},
}

@techreport{blanditiis2023,
  author      = {Blanditiis, Praesentium and Voluptatum, Deleniti},
  title       = {Corrupti Quos Dolores: Benchmarking Molestias Frameworks},
  institution = {Obcaecati Research Lab},
  year        = {2023},
  number      = {TR-2023-07},
}

@article{repellendus2018,
  author  = {Repellendus, Temporibus and Quibusdam, Officiis and Debitis, Aut},
  title   = {Rerum Necessitatibus in Saepe Eveniet Networks},
  journal = {Computational Voluptates Research},
  year    = {2018},
  volume  = {9},
  number  = {1},
  pages   = {33--51},
}

@phdthesis{sapiente2020,
  author = {Sapiente, Delectus},
  title  = {Reiciendis Voluptatibus and Their Applications to Maiores Alias Systems},
  school = {University of Perferendis},
  year   = {2020},
}

@article{asperiores2024,
  author  = {Asperiores, Repellat and Ipsum, Dolor},
  title   = {Stochastic Adipiscing with Eiusmod Constraints Under Tempor Uncertainty},
  journal = {Journal of Gibberish Studies},
  year    = {2024},
  volume  = {47},
  number  = {1},
  pages   = {1--29},
}
"#.to_string()
}

const TABLE_HEADERS: &[&[&str]] = &[
    &["Method", "Accuracy", "Precision", "Recall", "F1"],
    &["Parameter", "Value", "Std. Error", "t-stat", "p-value"],
    &["Model", "AIC", "BIC", "RMSE", "R\u{00B2}"],
    &["Dataset", "n", "Mean", "Median", "SD"],
    &["Configuration", "Time (s)", "Memory (MB)", "Iterations", "Status"],
];

fn build_gibberish_table(file_idx: usize, table_idx: usize) -> String {
    let seed = file_idx * 13 + table_idx * 7;
    let headers = TABLE_HEADERS[seed % TABLE_HEADERS.len()];
    let label = format!("tbl-{}-{}", file_idx + 1, table_idx);

    let mut out = String::new();

    // Header row
    out.push_str("| ");
    out.push_str(&headers.join(" | "));
    out.push_str(" |\n");

    // Separator
    out.push('|');
    for _ in headers {
        out.push_str("--------|");
    }
    out.push('\n');

    // 3-5 data rows
    let num_rows = 3 + (seed % 3);
    let row_labels = ["Baseline", "Eiusmod-A", "Tempor-B", "Hybrid", "Veniam-C"];
    for r in 0..num_rows {
        out.push_str("| ");
        out.push_str(row_labels[r % row_labels.len()]);
        for c in 1..headers.len() {
            let val = ((seed + r * 17 + c * 11) % 900) as f64 / 10.0 + 1.0;
            out.push_str(&format!(" | {:.1}", val));
        }
        out.push_str(" |\n");
    }

    // Caption with cross-ref label
    let caption = jinja_engine::lipsum_sentence(5 + (seed % 4), seed + 2);
    out.push_str(&format!(
        "\n: {} {{#{}}} \n",
        caption, label
    ));

    out
}

fn handle_info(action: InfoAction) -> Result<()> {
    match action {
        InfoAction::Csl => {
            use hayagriva::archive::ArchivedStyle;

            println!("Calepin uses CSL (Citation Style Language) for bibliography");
            println!("formatting. Over 2,600 styles are available from the Zotero");
            println!("style repository:");
            println!();
            println!("  https://www.zotero.org/styles");
            println!();
            println!("Download a .csl file and place it in assets/csl/, then set");
            println!("csl: in calepin.toml or in document front matter.");
            println!();
            println!("The following shortcuts are also available as built-in names");
            println!("(no download required):");
            println!();

            let mut names: Vec<&str> = ArchivedStyle::all().iter()
                .map(|s| s.names()[0])
                .collect();
            names.sort();

            // Print comma-separated, wrapped at 79 characters
            let joined = names.join(", ");
            let mut line = String::from("  ");
            for word in joined.split(' ') {
                if line.len() + 1 + word.len() > 79 && line.len() > 2 {
                    println!("{}", line);
                    line = format!("  {}", word);
                } else {
                    if line.len() > 2 { line.push(' '); }
                    line.push_str(word);
                }
            }
            if !line.trim().is_empty() {
                println!("{}", line);
            }
            Ok(())
        }
        InfoAction::Themes => {
            println!("Built-in syntax highlighting themes:\n");
            if let Some(dir) = render::elements::BUILTIN_PROJECT.get_dir("assets/highlighting") {
                let mut names: Vec<&str> = dir.files()
                    .filter_map(|f| {
                        if f.path().extension()?.to_str()? == "tmTheme" {
                            f.path().file_stem()?.to_str()
                        } else {
                            None
                        }
                    })
                    .collect();
                names.sort();
                for name in &names {
                    println!("  {}", name);
                }
                println!("\n{} themes available.", names.len());
            }
            println!("Custom themes: place a .tmTheme file in assets/highlighting/");
            Ok(())
        }
        InfoAction::Completions { shell } => {
            let mut cmd = <Cli as clap::CommandFactory>::command();
            clap_complete::generate(shell, &mut cmd, "calepin", &mut std::io::stdout());
            Ok(())
        }
    }
}

/// Result of the core render pipeline (before page template wrapping).
pub struct RenderResult {
    pub rendered: String,
    pub metadata: types::Metadata,
    pub element_renderer: ElementRenderer,
}

/// Core render pipeline: parse, evaluate, render. Does NOT apply the page template.
/// If `format` is None, falls back to the format declared in YAML front matter, then "html".
pub fn render_core(
    input: &Path,
    output_path: &Path,
    format: Option<&str>,
    overrides: &[String],
    project_var: Option<&toml::Value>,
) -> Result<RenderResult> {

/// Whether `CALEPIN_TIMING=1` is set (checked once at startup).
static TIMING: LazyLock<bool> = LazyLock::new(|| std::env::var("CALEPIN_TIMING").is_ok());

/// Print a timing line to stderr if `CALEPIN_TIMING` is set.
macro_rules! timed {
    ($label:expr, $block:expr) => {{
        if *TIMING {
            let _t = Instant::now();
            let _r = $block;
            eprintln!("[timing] {:.<30} {:>8.3}ms", $label, _t.elapsed().as_secs_f64() * 1000.0);
            _r
        } else {
            $block
        }
    }};
}

    let t_total = if *TIMING { Some(Instant::now()) } else { None };

    // 1. Read input file
    let input_text = fs::read_to_string(input)
        .with_context(|| format!("Failed to read input file: {}", input.display()))?;

    // 2. Parse YAML front matter, then apply CLI overrides
    let (mut metadata, body) = timed!("parse_yaml", parse::yaml::split_yaml(&input_text)?);
    let body = render::markers::sanitize(&body);
    metadata.apply_overrides(overrides);
    metadata.resolve_date(Some(input));

    // Merge project-level var as defaults (front matter wins)
    if let Some(pv) = project_var {
        if let Some(table) = pv.as_table() {
            for (key, val) in table {
                if !metadata.var.contains_key(key) {
                    metadata.var.insert(key.clone(), crate::value::from_toml(val.clone()));
                }
            }
        }
    }

    // 2b. Construct path context and validate paths
    let mut path_ctx = paths::PathContext::for_single_file(input, output_path);
    path_ctx.apply_metadata(&metadata);
    let input_name = input.file_name()
        .unwrap_or_default()
        .to_string_lossy();
    paths::validate_paths(&metadata, &path_ctx, &input_name)?;

    // 3. Create renderer for this format
    let format_str = format
        .map(|s| s.to_string())
        .or_else(|| metadata.target.clone())
        .unwrap_or_else(|| "html".to_string());
    let renderer = formats::create_renderer(&format_str)?;

    // 4. Expand includes before block parsing (so included code chunks are parsed)
    let body = timed!("expand_includes", jinja_engine::expand_includes(&body, &path_ctx.document_dir));

    // 4a. Preprocess hook: pipe body through script if custom format defines one
    let body = if let Some(script) = renderer.preprocess() {
        let input = serde_json::json!({
            "body": body,
            "format": format_str,
        });
        formats::run_script(script, &input.to_string(), &[])?
    } else {
        body
    };

    // 4b. Parse body into blocks
    let blocks = timed!("parse_blocks", parse::blocks::parse_body(&body)?);

    // 5. Initialize engine subprocesses only if needed
    let mut r_session = if engines::util::needs_engine(&blocks, &body, &metadata, "r") {
        Some(timed!("init_r", RSession::init(renderer.base_format())?))
    } else {
        None
    };
    let mut py_session = if engines::util::needs_engine(&blocks, &body, &metadata, "python") {
        Some(timed!("init_python", PythonSession::init()?))
    } else {
        None
    };
    let mut sh_session = if engines::util::needs_engine(&blocks, &body, &metadata, "sh") {
        Some(engines::sh::ShSession::init()?)
    } else {
        None
    };
    let mut ctx = EngineContext {
        r: r_session.as_mut(),
        python: py_session.as_mut(),
        sh: sh_session.as_mut(),
    };

    // 5b. Evaluate inline code in metadata fields (title, date, etc.)
    metadata.evaluate_inline(&mut ctx);

    // 6. Load plugin registry
    let registry = timed!("load_plugins", std::rc::Rc::new(
        registry::PluginRegistry::load(&metadata.plugins, &path_ctx.document_dir)
    ));

    // 7. Create element renderer
    let highlight_config = metadata.var.get("highlight-style")
        .map(|v| filters::highlighting::parse_highlight_config(v))
        .unwrap_or_else(|| {
            // Defaults from built-in calepin.toml [meta].highlight
            let cfg = project::builtin_config();
            let defaults = cfg.meta.as_ref().and_then(|m| m.highlight.as_ref());
            filters::highlighting::HighlightConfig::LightDark {
                light: defaults.and_then(|h| h.light.clone()).unwrap_or_else(|| "github".to_string()),
                dark: defaults.and_then(|h| h.dark.clone()).unwrap_or_else(|| "nord".to_string()),
            }
        });
    let mut element_renderer = ElementRenderer::new(renderer.base_format(), highlight_config);
    element_renderer.number_sections = metadata.number_sections;
    element_renderer.shift_headings = metadata.title.is_some();
    element_renderer.default_fig_cap_location = metadata.var.get("fig-cap-location")
        .and_then(|v| v.as_str()).map(|s| s.to_string());

    // 8. Evaluate: execute code chunks and produce elements
    let stem = output_path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let fig_dir = path_ctx.figures_dir(&stem);
    let fig_ext = renderer.default_fig_ext();
    let cache_enabled = metadata.var.get("execute")
        .and_then(|v| v.get("cache"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let cache_dir = path_ctx.cache_root(&stem);
    let mut cache = CacheState::new(input, &cache_dir, cache_enabled);
    let eval_result = timed!("evaluate", engines::evaluate(&blocks, &fig_dir, fig_ext, renderer.base_format(), &metadata, &registry, &mut ctx, &mut cache)?);
    let mut elements = eval_result.elements;

    // 9. Bibliography
    timed!("bibliography", filters::bibliography::process_citations(&mut elements, &metadata, &path_ctx.document_dir)?);

    // 10. Set registry on element renderer
    element_renderer.set_registry(registry);
    element_renderer.set_sc_fragments(eval_result.sc_fragments);

    // 12. Render elements to final format
    let rendered = timed!("render", renderer.render(&elements, &element_renderer)?);

    // 13. Cross-ref resolution (section IDs pre-collected from AST walk)
    let thm_nums = element_renderer.theorem_numbers();
    let walk_meta = element_renderer.walk_metadata();
    let rendered = timed!("crossref", match renderer.base_format() {
        "html" => filters::crossref::resolve_html_with_ids(&rendered, &thm_nums, &walk_meta.ids),
        "latex" => filters::crossref::resolve_latex(&rendered, &thm_nums),
        _ => filters::crossref::resolve_plain(&rendered, &thm_nums),
    });

    // 14. Number sections (HTML only) — now handled in the AST walker
    //     (render/html_ast.rs) via ElementRenderer.number_sections

    // Clean up empty fig_dir
    if fig_dir.is_dir() && std::fs::read_dir(&fig_dir).map_or(false, |mut d| d.next().is_none()) {
        std::fs::remove_dir(&fig_dir).ok();
    }

    if let Some(t) = t_total {
        eprintln!("[timing] {:=<30} {:>8.3}ms", "TOTAL ", t.elapsed().as_secs_f64() * 1000.0);
    }

    Ok(RenderResult { rendered, metadata, element_renderer })
}

/// Full render pipeline. Returns (output_path, rendered_content, renderer).
pub fn render_file(
    input: &Path,
    output: Option<&Path>,
    format: Option<&str>,
    overrides: &[String],
    target: Option<&project::Target>,
    project_root: Option<&Path>,
    project_var: Option<&toml::Value>,
    output_dir: Option<&str>,
) -> Result<(PathBuf, String, Box<dyn formats::OutputRenderer>)> {
    // If we have a target, use its base as the format
    let resolved_format = if let Some(t) = target {
        Some(t.base.clone())
    } else {
        format
            .map(|s| s.to_string())
            .or_else(|| {
                output
                    .and_then(|p| p.extension())
                    .and_then(|e| e.to_str())
                    .map(|ext| formats::resolve_format_from_extension(ext).to_string())
            })
    };

    // Determine output extension (target override or renderer default)
    let preliminary_format = resolved_format.as_deref().unwrap_or("html");
    let renderer = formats::create_renderer(preliminary_format)?;
    let ext = target.map(|t| t.output_extension()).unwrap_or(renderer.extension());

    // Resolve output path
    let output_path = if let Some(o) = output {
        o.to_path_buf()
    } else if let (Some(_), Some(fmt)) = (target, format) {
        // Use target-aware output path when a target is specified
        project::resolve_target_output_path(input, fmt, ext, project_root, output_dir)
    } else {
        input.with_extension(ext)
    };

    let result = render_core(input, &output_path, resolved_format.as_deref(), overrides, project_var)?;

    let final_output = renderer
        .apply_template(&result.rendered, &result.metadata, &result.element_renderer)
        .unwrap_or(result.rendered);

    Ok((output_path, final_output, renderer))
}
