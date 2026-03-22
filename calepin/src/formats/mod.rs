//! Output format backends.
//!
//! Built-in formats: html, latex, typst, markdown.
//! Custom formats: defined via `_calepin/formats/{name}.yaml` with a base
//! format, optional file extension override, and optional WASM plugin
//! for post-processing.

pub mod html;
pub mod latex;
pub mod markdown;
pub mod typst;
pub mod word;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::io::Write;

use anyhow::{Context, Result};

use crate::types::{Element, Metadata};
use crate::render::elements::ElementRenderer;

/// Trait for pluggable output formats.
pub trait OutputRenderer {
    /// Canonical format name (e.g., "html", "latex", "typst", "markdown").
    fn format(&self) -> &str;

    /// File extension for output files (e.g., "html", "tex", "typ", "md").
    fn extension(&self) -> &str;

    /// Base format name for element template lookup.
    /// For built-in formats, same as `format()`. For custom formats,
    /// returns the base format (e.g., "html" for a "blog" format).
    fn base_format(&self) -> &str {
        self.format()
    }

    /// Default figure file extension.
    fn default_fig_ext(&self) -> &str {
        "svg"
    }

    /// Render a list of elements into the final document body.
    fn render(&self, elements: &[Element], renderer: &ElementRenderer) -> Result<String> {
        use std::time::Instant;

        // Pre-collect footnote definitions across all Text elements so that
        // references in one block can resolve against definitions in another.
        renderer.collect_footnote_defs(elements);

        let timing = std::env::var("CALEPIN_TIMING").is_ok();
        let mut t_text = 0f64;
        let mut t_div = 0f64;
        let mut n_text = 0usize;
        let mut n_div = 0usize;
        let mut by_kind: std::collections::HashMap<&str, (f64, usize)> =
            std::collections::HashMap::new();

        let parts: Vec<String> = elements
            .iter()
            .map(|el| {
                if timing {
                    let t = Instant::now();
                    let r = renderer.render(el);
                    let elapsed = t.elapsed().as_secs_f64() * 1000.0;
                    match el {
                        Element::Text { .. } => { t_text += elapsed; n_text += 1; }
                        Element::Div { .. } => { t_div += elapsed; n_div += 1; }
                        _ => {
                            let kind = el.template_name();
                            let kind = if kind.is_empty() { "other" } else { kind };
                            let entry = by_kind.entry(kind).or_insert((0.0, 0));
                            entry.0 += elapsed;
                            entry.1 += 1;
                        }
                    }
                    r
                } else {
                    renderer.render(el)
                }
            })
            .filter(|s| !s.is_empty())
            .collect();

        if timing {
            eprintln!("[timing]   text  ({:>3} blocks) {:>8.3}ms", n_text, t_text);
            eprintln!("[timing]   div   ({:>3} blocks) {:>8.3}ms", n_div, t_div);
            let mut kinds: Vec<_> = by_kind.iter().collect();
            kinds.sort_by(|a, b| b.1.0.partial_cmp(&a.1.0).unwrap());
            for (kind, (ms, count)) in kinds {
                eprintln!("[timing]   {:5} ({:>3} blocks) {:>8.3}ms", kind, count, ms);
            }
        }

        let body = parts.join("\n\n");
        Ok(self.postprocess(&body, renderer))
    }

    /// Format-specific post-processing on the rendered body.
    fn postprocess(&self, body: &str, _renderer: &ElementRenderer) -> String {
        body.to_string()
    }

    /// Wrap the rendered body in a page template. Return None to skip.
    fn apply_template(&self, body: &str, meta: &Metadata, renderer: &ElementRenderer)
        -> Option<String>;

    /// Optional preprocess script path. If set, the raw .qmd body is piped
    /// through this script before block parsing.
    fn preprocess(&self) -> Option<&Path> {
        None
    }

    /// Write the final rendered content to the output file.
    /// The default implementation writes the string directly. Formats that
    /// need external conversion (e.g., Word via pandoc) override this.
    fn write_output(&self, content: &str, output_path: &Path) -> Result<()> {
        std::fs::write(output_path, content)
            .with_context(|| format!("Failed to write output file: {}", output_path.display()))?;
        Ok(())
    }
}

/// Create a renderer from a format name string.
/// Checks built-in formats first, then custom format configs.
pub fn create_renderer(format: &str) -> Result<Box<dyn OutputRenderer>> {
    match format {
        "html" => Ok(Box::new(html::HtmlRenderer)),
        "latex" | "tex" => Ok(Box::new(latex::LatexRenderer)),
        "markdown" | "md" | "reprex" => Ok(Box::new(markdown::MarkdownRenderer)),
        "typst" | "typ" => Ok(Box::new(typst::TypstRenderer)),
        "word" | "docx" => Ok(Box::new(word::WordRenderer)),
        other => load_custom_format(other),
    }
}

/// Map a file extension to a canonical format name.
pub fn format_from_extension(ext: &str) -> &str {
    match ext {
        "tex" => "latex",
        "pdf" => "latex",
        "typ" => "typst",
        "md" => "markdown",
        "docx" => "word",
        "html" => "html",
        other => other,
    }
}

// ---------------------------------------------------------------------------
// Custom format support
// ---------------------------------------------------------------------------

/// A custom format defined via `_calepin/formats/{name}.yaml`.
struct CustomRenderer {
    name: String,
    ext: String,
    base: Box<dyn OutputRenderer>,
    preprocess_script: Option<PathBuf>,
    postprocess_script: Option<PathBuf>,
}

impl OutputRenderer for CustomRenderer {
    fn format(&self) -> &str {
        &self.name
    }

    fn base_format(&self) -> &str {
        self.base.format()
    }

    fn extension(&self) -> &str {
        &self.ext
    }

    fn default_fig_ext(&self) -> &str {
        self.base.default_fig_ext()
    }

    fn postprocess(&self, body: &str, renderer: &ElementRenderer) -> String {
        self.base.postprocess(body, renderer)
    }

    fn apply_template(
        &self,
        body: &str,
        meta: &Metadata,
        renderer: &ElementRenderer,
    ) -> Option<String> {
        // Base format template (produces complete document)
        let templated = {
            // Try custom page template first: calepin.{name}
            let custom_tpl = crate::render::template::load_page_template(
                &format!("calepin.{}", self.name),
            );
            if !custom_tpl.is_empty() {
                let mut vars = match self.base.format() {
                    "html" => crate::render::template::build_html_vars(meta, body),
                    "latex" => crate::render::template::build_latex_vars(meta, body),
                    "typst" => crate::render::template::build_typst_vars(meta, body),
                    _ => return None,
                };
                // Add syntax highlighting CSS (not included by build_*_vars)
                if self.base.format() == "html" {
                    let syntax_css = renderer.syntax_css();
                    if !syntax_css.is_empty() {
                        let css = vars.entry("css".to_string()).or_default();
                        css.push_str(&format!("\n<style>\n{}</style>", &syntax_css));
                        vars.insert("syntax_css".to_string(), syntax_css);
                    }
                    let datatheme_css = renderer.syntax_css_with_scope(
                        crate::filters::highlighting::ColorScope::DataTheme,
                    );
                    if !datatheme_css.is_empty() {
                        vars.insert("syntax_css_datatheme".to_string(), datatheme_css);
                    }
                }
                Some(crate::render::template::apply_template(&custom_tpl, &vars))
            } else {
                self.base.apply_template(body, meta, renderer)
            }
        };

        // 3. Script postprocess (transforms the complete document)
        if let Some(ref script) = self.postprocess_script {
            let input = templated.as_deref().unwrap_or(body);
            match run_script(script, input, &[&self.name]) {
                Ok(output) => return Some(output),
                Err(e) => {
                    eprintln!("Warning: postprocess script failed: {}", e);
                }
            }
        }

        templated
    }

    fn preprocess(&self) -> Option<&Path> {
        self.preprocess_script.as_deref()
    }
}

/// Run a script, piping `stdin_data` to its stdin and returning stdout.
pub fn run_script(script: &Path, stdin_data: &str, args: &[&str]) -> Result<String> {
    let mut child = Command::new(script)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to run script: {}", script.display()))?;
    child.stdin.take().unwrap().write_all(stdin_data.as_bytes())?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Script {} failed: {}", script.display(), stderr);
    }
    Ok(String::from_utf8(output.stdout)?)
}

/// Load a custom format from `_calepin/formats/{name}.toml` (or `{name}.yaml` fallback).
fn load_custom_format(name: &str) -> Result<Box<dyn OutputRenderer>> {
    // Try TOML first, then YAML fallback
    let path = crate::paths::resolve_path_cwd("formats", &format!("{}.toml", name))
        .or_else(|| crate::paths::resolve_path_cwd("formats", &format!("{}.yaml", name)))
        .ok_or_else(|| anyhow::anyhow!(
            "Unknown format: '{}'. No built-in format or config at _calepin/formats/{}.toml",
            name, name
        ))?;

    let content = std::fs::read_to_string(&path)?;
    let config = if path.extension().map_or(false, |e| e == "toml") {
        let tv: toml::Value = toml::from_str(&content)?;
        crate::value::from_toml(tv)
    } else {
        crate::value::Value::Table(crate::value::parse_minimal_yaml(&content))
    };

    let base_name = config.get("base")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Custom format '{}': missing 'base' field", name))?;

    // Create the base renderer (must be a built-in)
    let base: Box<dyn OutputRenderer> = match base_name {
        "html" => Box::new(html::HtmlRenderer),
        "latex" => Box::new(latex::LatexRenderer),
        "markdown" => Box::new(markdown::MarkdownRenderer),
        "typst" => Box::new(typst::TypstRenderer),
        other => anyhow::bail!("Custom format '{}': unknown base format '{}'", name, other),
    };

    let ext = config.get("extension")
        .and_then(|v| v.as_str())
        .unwrap_or(base.extension())
        .to_string();

    let config_dir = path.parent().unwrap_or(Path::new("."));
    let preprocess_script = config.get("preprocess")
        .and_then(|v| v.as_str())
        .map(|s| config_dir.join(s));
    let postprocess_script = config.get("postprocess")
        .and_then(|v| v.as_str())
        .map(|s| config_dir.join(s));

    Ok(Box::new(CustomRenderer {
        name: name.to_string(),
        ext,
        base,
        preprocess_script,
        postprocess_script,
    }))
}
