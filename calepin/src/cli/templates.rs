//! `calepin templates` subcommand: list, show, diff, reset, eject, outdated, vars, preview, lint.
//!
//! All subcommands operate on a specific sidecar identified by the input .qmd file.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::cli::TemplatesAction;
use crate::render::elements::{BUILTIN_EXTENSIONS, resolve_builtin_template};

// ---------------------------------------------------------------------------
// Context: resolve sidecar + target from input .qmd
// ---------------------------------------------------------------------------

struct TemplateContext {
    /// The flat templates directory: {stem}_calepin/templates/{target}/
    tpl_dir: PathBuf,
    target_name: String,
    writer: String,
    writer_ext: String,
    chain: Vec<String>,
}

impl TemplateContext {
    /// Resolve a template file by name: try exact name, then name.{writer_ext}.
    /// Returns (path, filename) if found in the sidecar.
    fn resolve_local(&self, name: &str) -> Option<(PathBuf, String)> {
        let exact = self.tpl_dir.join(name);
        if exact.exists() {
            return Some((exact, name.to_string()));
        }
        let with_ext = format!("{}.{}", name, self.writer_ext);
        let path = self.tpl_dir.join(&with_ext);
        if path.exists() {
            return Some((path, with_ext));
        }
        None
    }
}

/// Resolve the sidecar and target from an input .qmd file.
fn resolve_context(input: &Path, target_override: Option<&str>) -> Result<TemplateContext> {
    if !input.exists() {
        bail!("File not found: {}", input.display());
    }
    let stem = input.file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("Cannot determine stem for: {}", input.display()))?;
    let parent = input.parent().unwrap_or(Path::new("."));
    let sidecar = parent.join(format!("{}_calepin", stem));

    let target_name = if let Some(t) = target_override {
        t.to_string()
    } else {
        let text = std::fs::read_to_string(input)?;
        crate::config::split_frontmatter(&text)
            .ok()
            .and_then(|(fm, _)| fm.target)
            .unwrap_or_else(|| "html".to_string())
    };

    let empty = std::collections::HashMap::new();
    let target = crate::config::resolve_target(&target_name, &empty).ok();
    let writer = target.as_ref().map(|t| t.writer.as_str()).unwrap_or(&target_name).to_string();
    let project_root = parent.to_path_buf();
    let chain = crate::config::extension::inheritance_chain(&project_root, &target_name, &empty);
    crate::paths::set_active_target_with_chain(Some(&target_name), chain.clone());

    let tpl_dir = sidecar.join("templates").join(&target_name);

    let writer_ext = crate::paths::resolve_extension(&writer).to_string();
    Ok(TemplateContext { tpl_dir, target_name, writer, writer_ext, chain })
}

/// Collect all built-in template filenames for the given inheritance chain.
fn collect_builtin_files(chain: &[String]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for chain_target in chain {
        let tpl_path = format!("{}/templates", chain_target);
        if let Some(dir) = BUILTIN_EXTENSIONS.get_dir(&tpl_path) {
            let prefix = std::path::Path::new(&tpl_path);
            collect_dir_files(dir, prefix, &mut names);
        }
    }
    names.sort();
    names.dedup();
    names
}

fn collect_dir_files(dir: &include_dir::Dir<'static>, prefix: &Path, names: &mut Vec<String>) {
    for file in dir.files() {
        let rel = file.path().strip_prefix(prefix).unwrap_or(file.path());
        let name = rel.display().to_string();
        if !names.contains(&name) {
            names.push(name);
        }
    }
    for subdir in dir.dirs() {
        collect_dir_files(subdir, prefix, names);
    }
}

/// Get built-in template content by filename, walking the chain (child-first).
fn get_builtin_content(chain: &[String], filename: &str) -> Option<String> {
    for chain_target in chain {
        let path = format!("{}/templates/{}", chain_target, filename);
        if let Some(file) = BUILTIN_EXTENSIONS.get_file(&path) {
            if let Some(content) = file.contents_utf8() {
                return Some(content.to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

pub fn handle_templates(action: TemplatesAction) -> Result<()> {
    match action {
        TemplatesAction::List { input, target } => handle_list(&input, target.as_deref()),
        TemplatesAction::Show { name, input, target } => handle_show(&name, &input, target.as_deref()),
        TemplatesAction::Diff { input, name, target } => handle_diff(&input, name.as_deref(), target.as_deref()),
        TemplatesAction::Eject { name, input, target, yes } => handle_eject(&name, &input, target.as_deref(), yes),
        TemplatesAction::Vars { name, input, target } => handle_vars(&name, &input, target.as_deref()),
        TemplatesAction::Preview { name, input, target } => handle_preview(&name, &input, target.as_deref()),
        TemplatesAction::Lint { input, name, target } => handle_lint(&input, name.as_deref(), target.as_deref()),
    }
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

fn handle_list(input: &Path, target: Option<&str>) -> Result<()> {
    let ctx = resolve_context(input, target)?;
    let builtin_files = collect_builtin_files(&ctx.chain);

    println!("Templates for '{}' (target: {}):\n", input.display(), ctx.target_name);

    // Collect local files
    let mut local_files: Vec<String> = Vec::new();
    if ctx.tpl_dir.is_dir() {
        let pattern = ctx.tpl_dir.join("**").join("*.*");
        for entry in crate::util::safe_glob(&pattern.display().to_string()) {
            if let Ok(path) = entry {
                if path.is_file() {
                    let rel = path.strip_prefix(&ctx.tpl_dir).unwrap_or(&path);
                    local_files.push(rel.display().to_string());
                }
            }
        }
        local_files.sort();
    }

    // Show built-in templates with status
    for name in &builtin_files {
        let path = ctx.tpl_dir.join(name);
        if path.exists() {
            let local = std::fs::read_to_string(&path).unwrap_or_default();
            let builtin = get_builtin_content(&ctx.chain, name).unwrap_or_default();
            if local == builtin {
                println!("  {}  \x1b[36mdefault\x1b[0m", name);
            } else {
                println!("  {}  \x1b[33mmodified\x1b[0m", name);
            }
        } else {
            println!("  {}  \x1b[90mmissing\x1b[0m", name);
        }
    }

    // Show custom templates (in sidecar but not in built-in)
    for name in &local_files {
        if !builtin_files.contains(name) {
            println!("  {}  \x1b[32mcustom\x1b[0m", name);
        }
    }

    if ctx.tpl_dir.is_dir() {
        println!("\nTemplates: {}", ctx.tpl_dir.display());
    } else {
        println!("\nNo sidecar. Run `calepin init {}` to create one.", input.display());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// show
// ---------------------------------------------------------------------------

fn handle_show(name: &str, input: &Path, target: Option<&str>) -> Result<()> {
    let ctx = resolve_context(input, target)?;

    // Try local sidecar file
    if let Some((path, _)) = ctx.resolve_local(name) {
        print!("{}", std::fs::read_to_string(&path)?);
        return Ok(());
    }

    // Fall back to built-in (by short name, then full filename)
    if let Some(content) = resolve_builtin_template(name, &ctx.writer)
        .map(|s| s.to_string())
        .or_else(|| get_builtin_content(&ctx.chain, name))
    {
        print!("{}", content);
        return Ok(());
    }

    bail!("Template '{}' not found for target '{}'", name, ctx.target_name);
}

// ---------------------------------------------------------------------------
// diff
// ---------------------------------------------------------------------------

fn handle_diff(input: &Path, name: Option<&str>, target: Option<&str>) -> Result<()> {
    let ctx = resolve_context(input, target)?;

    if !ctx.tpl_dir.is_dir() {
        bail!("No sidecar templates at {}. Run `calepin init {}` first.", ctx.tpl_dir.display(), input.display());
    }

    let files: Vec<String> = if let Some(name) = name {
        if let Some((_, filename)) = ctx.resolve_local(name) {
            vec![filename]
        } else {
            bail!("Template '{}' not found in {}", name, ctx.tpl_dir.display());
        }
    } else {
        collect_builtin_files(&ctx.chain)
    };

    let mut found_diff = false;
    for filename in &files {
        let path = ctx.tpl_dir.join(filename);
        if !path.exists() { continue; }

        let local = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let builtin = match get_builtin_content(&ctx.chain, filename) {
            Some(b) => b,
            None => continue,
        };

        if local == builtin { continue; }

        found_diff = true;
        println!("\x1b[1m--- built-in: {}\x1b[0m", filename);
        println!("\x1b[1m+++ local:    {}\x1b[0m", filename);

        let builtin_lines: Vec<&str> = builtin.lines().collect();
        let local_lines: Vec<&str> = local.lines().collect();
        let max = builtin_lines.len().max(local_lines.len());
        for i in 0..max {
            match (builtin_lines.get(i), local_lines.get(i)) {
                (Some(b), Some(l)) if b == l => println!(" {}", b),
                (Some(b), Some(l)) => {
                    println!("\x1b[31m-{}\x1b[0m", b);
                    println!("\x1b[32m+{}\x1b[0m", l);
                }
                (Some(b), None) => println!("\x1b[31m-{}\x1b[0m", b),
                (None, Some(l)) => println!("\x1b[32m+{}\x1b[0m", l),
                (None, None) => {}
            }
        }
        println!();
    }

    if !found_diff {
        eprintln!("No differences found.");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// eject: copy a built-in template into the sidecar for editing
// ---------------------------------------------------------------------------

fn handle_eject(name: &str, input: &Path, target: Option<&str>, yes: bool) -> Result<()> {
    let ctx = resolve_context(input, target)?;

    // Resolve the built-in template content
    let filename = format!("{}.{}", name, ctx.writer_ext);
    let builtin = resolve_builtin_template(name, &ctx.writer)
        .map(|s| s.to_string())
        .or_else(|| get_builtin_content(&ctx.chain, &filename))
        .or_else(|| get_builtin_content(&ctx.chain, name))
        .ok_or_else(|| anyhow::anyhow!("No built-in template '{}' for target '{}'", name, ctx.target_name))?;

    let dest = ctx.tpl_dir.join(&filename);

    // Confirm before overwriting
    if dest.exists() && !yes {
        let existing = std::fs::read_to_string(&dest).unwrap_or_default();
        if existing == builtin {
            eprintln!("{} already exists (identical to built-in).", filename);
            return Ok(());
        }
        eprint!("{} already exists in sidecar. Overwrite with built-in? [y/N] ", filename);
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if !answer.trim().eq_ignore_ascii_case("y") {
            eprintln!("Aborted.");
            return Ok(());
        }
    }

    // Ensure directories exist
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&dest, &builtin)?;
    eprintln!("Ejected {} to {}", filename, dest.display());

    Ok(())
}

// ---------------------------------------------------------------------------
// vars: show available template variables for a given template
// ---------------------------------------------------------------------------

fn handle_vars(name: &str, input: &Path, target: Option<&str>) -> Result<()> {
    let ctx = resolve_context(input, target)?;

    println!("Template variables for '{}' (target: {}):\n", name, ctx.target_name);

    // Common variables available to all templates
    println!("\x1b[1mcfg.*\x1b[0m (user-authored, from front matter/config/attributes):");

    match name {
        "figure" => {
            println!("  cfg.id            Figure ID (e.g., fig-scatter)");
            println!("  cfg.caption       Rendered caption text");
            println!("  cfg.fig_width     Figure width");
            println!("  cfg.fig_height    Figure height");
            println!("  cfg.fig_align     Figure alignment (left, center, right)");
            println!("  cfg.fig_alt       Alt text for accessibility");
            println!("  cfg.label         Figure label");
            println!("  cfg.*             Any div attribute passed as key=value");
        }
        "code_source" => {
            println!("  cfg.lang          Language identifier (r, python, js, ...)");
            println!("  cfg.label         Code chunk label");
            println!("  cfg.filename      Filename annotation");
        }
        "code_output" => {
            // Minimal
        }
        "code_diagnostic" | "code_error" | "code_warning" | "code_message" => {
            println!("  cfg.type          Diagnostic type (error, warning, message)");
        }
        "div" => {
            println!("  cfg.id            Div ID");
            println!("  cfg.classes       Space-separated class list");
            println!("  cfg.class_list    Array of individual classes");
            println!("  cfg.template      Explicit template override name");
            println!("  cfg.*             Any div attribute passed as key=value");
        }
        "main" | "base" => {
            println!("  cfg.title         Document title (rendered)");
            println!("  cfg.title_plain   Document title (plain text, no markup)");
            println!("  cfg.date          Formatted date string");
            println!("  cfg.subtitle      Document subtitle");
            println!("  cfg.abstract      Abstract text");
            println!("  cfg.keywords      Comma-separated keywords");
            println!("  cfg.lang          Document language (default: en)");
            println!("  cfg.target        Target name");
            println!("  cfg.*             Any custom front matter field");
        }
        "toc" => {
            println!("  cfg.title         TOC title");
        }
        _ => {
            println!("  cfg.*             Attributes and front matter fields");
        }
    }

    println!();
    println!("\x1b[1mclp.*\x1b[0m (engine-computed):");

    match name {
        "figure" => {
            println!("  clp.content       Rendered figure children (images, code output)");
            println!("  clp.writer        Output format (html, latex, typst, markdown)");
        }
        "code_source" => {
            println!("  clp.content       Syntax-highlighted code (html/latex) or escaped code");
            println!("  clp.writer        Output format");
        }
        "code_output" | "code_diagnostic" | "code_error" | "code_warning" | "code_message" => {
            println!("  clp.content       Escaped output/diagnostic text");
            println!("  clp.writer        Output format");
        }
        "div" => {
            println!("  clp.content       Rendered children");
            println!("  clp.writer        Output format");
            println!("  clp.number        Auto-number (if numbering is enabled)");
            println!("  clp.type_class    Type class for numbered divs (e.g., theorem)");
        }
        "main" | "base" => {
            println!("  clp.body          Rendered document body");
            println!("  clp.toc           Table of contents HTML");
            println!("  clp.css           Concatenated CSS (standalone mode)");
            println!("  clp.js            Concatenated JS (standalone mode)");
            println!("  clp.css_path      Relative path to external CSS");
            println!("  clp.js_path       Relative path to external JS");
            println!("  clp.colors_css    CSS custom properties for color scheme");
            println!("  clp.tailwind_colors   Tailwind color config (CDN mode)");
            println!("  clp.tailwind_theme_css  Tailwind @theme CSS (CLI mode)");
            println!("  clp.tailwind_mode     'cdn' or 'cli'");
            println!("  clp.math          Math library include (KaTeX/MathJax)");
            println!("  clp.authors       Rendered author block");
            println!("  clp.appendix      Rendered appendix");
            println!("  clp.preamble      Preamble (LaTeX packages, HTML head tags)");
            println!("  clp.bibliography  Rendered bibliography block");
            println!("  clp.ext_css       Extension-only CSS");
            println!("  clp.writer        Output format");
        }
        "toc" => {
            println!("  clp.content       Rendered TOC list HTML");
            println!("  clp.items         Structured array of TOC entries");
            println!("    .id             Heading ID");
            println!("    .text           Heading text");
            println!("    .level          Heading level (1-6)");
            println!("    .depth          Depth relative to minimum level");
        }
        _ => {
            println!("  clp.content       Rendered content");
            println!("  clp.writer        Output format");
        }
    }

    println!();
    println!("\x1b[1mtpl.*\x1b[0m (template variant selections from [tpl] config):");
    println!("  tpl.<name>        Variant selection for template <name>");

    Ok(())
}

// ---------------------------------------------------------------------------
// preview: render a sample element through a template
// ---------------------------------------------------------------------------

fn handle_preview(name: &str, input: &Path, target: Option<&str>) -> Result<()> {
    let ctx = resolve_context(input, target)?;

    // Get the template content
    let tpl = if let Some((path, _)) = ctx.resolve_local(name) {
        std::fs::read_to_string(&path)?
    } else if let Some(content) = resolve_builtin_template(name, &ctx.writer)
        .map(|s| s.to_string())
        .or_else(|| {
            let filename = format!("{}.{}", name, ctx.writer_ext);
            get_builtin_content(&ctx.chain, &filename)
        })
    {
        content
    } else {
        bail!("Template '{}' not found for target '{}'", name, ctx.target_name);
    };

    // Build sample variables based on template type
    let mut vars = crate::render::template::TemplateVars::with_writer(&ctx.writer);

    match name {
        "figure" => {
            vars.cfg.insert("id".to_string(), minijinja::Value::from("fig-example"));
            vars.cfg.insert("caption".to_string(), minijinja::Value::from("An example figure caption"));
            vars.clp.insert("content".to_string(), minijinja::Value::from(
                if ctx.writer == "html" { "<img src=\"example.png\" alt=\"Example\">" }
                else { "example.png" }
            ));
        }
        "code_source" => {
            vars.cfg.insert("lang".to_string(), minijinja::Value::from("python"));
            vars.cfg.insert("label".to_string(), minijinja::Value::from(""));
            let sample = if ctx.writer == "html" {
                "<span class=\"kw\">def</span> <span class=\"fn\">hello</span>():\n    <span class=\"kw\">print</span>(<span class=\"st\">\"world\"</span>)"
            } else {
                "def hello():\n    print(\"world\")"
            };
            vars.clp.insert("content".to_string(), minijinja::Value::from(sample));
        }
        "code_output" => {
            vars.clp.insert("content".to_string(), minijinja::Value::from("Hello, world!\n[1] 42"));
        }
        "code_diagnostic" | "code_error" | "code_warning" | "code_message" => {
            let kind = if name == "code_error" { "error" }
                else if name == "code_warning" { "warning" }
                else { "message" };
            vars.cfg.insert("type".to_string(), minijinja::Value::from(kind));
            vars.clp.insert("content".to_string(), minijinja::Value::from("Sample diagnostic message"));
        }
        "div" => {
            vars.cfg.insert("id".to_string(), minijinja::Value::from("example-div"));
            vars.cfg.insert("classes".to_string(), minijinja::Value::from("callout note"));
            vars.cfg.insert("class_list".to_string(), minijinja::Value::from(vec!["callout", "note"]));
            vars.clp.insert("content".to_string(), minijinja::Value::from("<p>Sample div content.</p>"));
        }
        "toc" => {
            vars.cfg.insert("title".to_string(), minijinja::Value::from("Contents"));
            let sample_items = vec![
                {
                    let mut m = std::collections::BTreeMap::new();
                    m.insert("id".to_string(), minijinja::Value::from("introduction"));
                    m.insert("text".to_string(), minijinja::Value::from("Introduction"));
                    m.insert("level".to_string(), minijinja::Value::from(2i64));
                    m.insert("depth".to_string(), minijinja::Value::from(0i64));
                    minijinja::Value::from(m)
                },
                {
                    let mut m = std::collections::BTreeMap::new();
                    m.insert("id".to_string(), minijinja::Value::from("methods"));
                    m.insert("text".to_string(), minijinja::Value::from("Methods"));
                    m.insert("level".to_string(), minijinja::Value::from(2i64));
                    m.insert("depth".to_string(), minijinja::Value::from(0i64));
                    minijinja::Value::from(m)
                },
            ];
            vars.clp.insert("items".to_string(), minijinja::Value::from(sample_items));
            vars.clp.insert("content".to_string(), minijinja::Value::from(
                "<ul>\n<li><a href=\"#introduction\">Introduction</a></li>\n<li><a href=\"#methods\">Methods</a></li>\n</ul>"
            ));
        }
        "main" | "base" => {
            vars.cfg.insert("title".to_string(), minijinja::Value::from("Sample Document"));
            vars.cfg.insert("title_plain".to_string(), minijinja::Value::from("Sample Document"));
            vars.cfg.insert("date".to_string(), minijinja::Value::from("2025-01-15"));
            vars.cfg.insert("lang".to_string(), minijinja::Value::from("en"));
            vars.cfg.insert("target".to_string(), minijinja::Value::from(ctx.target_name.clone()));
            vars.clp.insert("body".to_string(), minijinja::Value::from("<h2 id=\"intro\">Introduction</h2>\n<p>Sample body text.</p>"));
            vars.clp.insert("toc".to_string(), minijinja::Value::from(""));
            vars.clp.insert("css".to_string(), minijinja::Value::from("/* sample css */"));
            vars.clp.insert("js".to_string(), minijinja::Value::from("// sample js"));
            vars.clp.insert("css_path".to_string(), minijinja::Value::from(""));
            vars.clp.insert("js_path".to_string(), minijinja::Value::from(""));
            vars.clp.insert("colors_css".to_string(), minijinja::Value::from(""));
            vars.clp.insert("tailwind_colors".to_string(), minijinja::Value::from(""));
            vars.clp.insert("tailwind_theme_css".to_string(), minijinja::Value::from(""));
            vars.clp.insert("tailwind_mode".to_string(), minijinja::Value::from("cdn"));
            vars.clp.insert("math".to_string(), minijinja::Value::from(""));
            vars.clp.insert("authors".to_string(), minijinja::Value::from(""));
            vars.clp.insert("appendix".to_string(), minijinja::Value::from(""));
            vars.clp.insert("preamble".to_string(), minijinja::Value::from(""));
            vars.clp.insert("bibliography".to_string(), minijinja::Value::from(""));
            vars.clp.insert("ext_css".to_string(), minijinja::Value::from(""));
        }
        _ => {
            vars.clp.insert("content".to_string(), minijinja::Value::from("<p>Sample content.</p>"));
        }
    }

    let rendered = crate::render::template::apply_template(&tpl, &vars);
    print!("{}", rendered);

    Ok(())
}

// ---------------------------------------------------------------------------
// lint: validate templates for undefined variable references
// ---------------------------------------------------------------------------

fn handle_lint(input: &Path, name: Option<&str>, target: Option<&str>) -> Result<()> {
    let ctx = resolve_context(input, target)?;

    if !ctx.tpl_dir.is_dir() {
        bail!("No sidecar templates at {}. Run `calepin init {}` first.", ctx.tpl_dir.display(), input.display());
    }

    let files: Vec<(String, String)> = if let Some(name) = name {
        let (path, filename) = ctx.resolve_local(name)
            .ok_or_else(|| anyhow::anyhow!("Template '{}' not found in {}", name, ctx.tpl_dir.display()))?;
        let content = std::fs::read_to_string(&path)?;
        vec![(filename, content)]
    } else {
        let mut result = Vec::new();
        if ctx.tpl_dir.is_dir() {
            let pattern = ctx.tpl_dir.join("**").join("*.*");
            for entry in crate::util::safe_glob(&pattern.display().to_string()) {
                if let Ok(path) = entry {
                    if path.is_file() {
                        let rel = path.strip_prefix(&ctx.tpl_dir).unwrap_or(&path);
                        let name = rel.display().to_string();
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            result.push((name, content));
                        }
                    }
                }
            }
        }
        result
    };

    let mut total_issues = 0;

    for (filename, content) in &files {
        let issues = lint_template(&filename, &content);
        if !issues.is_empty() {
            println!("\x1b[1m{}\x1b[0m", filename);
            for issue in &issues {
                println!("  \x1b[33m{}\x1b[0m", issue);
                total_issues += 1;
            }
        }
    }

    if total_issues == 0 {
        eprintln!("No issues found.");
    } else {
        eprintln!("\n{} issue(s) found.", total_issues);
    }

    Ok(())
}

/// Lint a single template for common issues.
fn lint_template(filename: &str, content: &str) -> Vec<String> {
    let mut issues = Vec::new();

    // Use MiniJinja strict mode to find undefined variables
    let mut env = minijinja::Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);

    // Try to parse the template
    if let Err(e) = env.add_template("__lint__", content) {
        issues.push(format!("Parse error: {}", e));
        return issues;
    }

    // Collect loop variable names from {% for X in ... %} and {% set X = ... %}
    let re_for = regex::Regex::new(r"\{%[-\s]*for\s+(\w+)").unwrap();
    let re_set = regex::Regex::new(r"\{%[-\s]*set\s+(\w+)").unwrap();
    let mut local_vars: std::collections::HashSet<&str> = re_for.captures_iter(content)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str()))
        .collect();
    for cap in re_set.captures_iter(content) {
        if let Some(m) = cap.get(1) {
            local_vars.insert(m.as_str());
        }
    }

    // Check for common issues via regex
    let re_var = regex::Regex::new(r"\{\{[\s]*([a-zA-Z_][a-zA-Z0-9_.]*)\s*\}\}").unwrap();
    let known_namespaces = ["cfg", "clp", "tpl", "env"];

    for cap in re_var.captures_iter(content) {
        let var = &cap[1];
        let parts: Vec<&str> = var.split('.').collect();
        if let Some(&ns) = parts.first() {
            // Skip local variables (loop vars, set vars)
            if local_vars.contains(ns) {
                continue;
            }
            // Skip single-word variables (no dot) -- these may come from
            // set statements, macros, or legacy templates
            if parts.len() == 1 {
                continue;
            }
            if !known_namespaces.contains(&ns) {
                issues.push(format!(
                    "Unknown namespace '{}' in {{{{ {} }}}} (expected cfg.*, clp.*, tpl.*, or env.*)",
                    ns, var
                ));
            }
        }
    }

    // Check for deprecated patterns
    if content.contains("{{ config.") {
        issues.push("Deprecated: use 'cfg.' instead of 'config.'".to_string());
    }
    if content.contains("{{ calepin.") {
        issues.push("Deprecated: use 'clp.' instead of 'calepin.'".to_string());
    }

    // Check for unclosed blocks
    let open_blocks = content.matches("{%").count();
    let close_blocks = content.matches("%}").count();
    if open_blocks != close_blocks {
        issues.push(format!(
            "Mismatched block tags: {} opening '{{% ... '  vs {} closing ' ... %}}'",
            open_blocks, close_blocks
        ));
    }

    // Warn about direct HTML in non-HTML templates
    if (filename.ends_with(".tex") || filename.ends_with(".typ")) && content.contains("<div") {
        issues.push("HTML tags found in non-HTML template".to_string());
    }

    issues
}
