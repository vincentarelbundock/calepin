//! Output format backends.
//!
//! Built-in formats: html, latex, typst, markdown.
//! Custom formats: defined via `_calepin/formats/{name}.toml` with a base
//! format, optional file extension override, and optional script
//! for pre/post-processing.

pub mod html;
pub mod latex;
pub mod markdown;
pub mod revealjs;
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
    fn engine(&self) -> &str {
        self.format()
    }

    /// Default figure file extension, derived from the built-in config.
    fn default_fig_ext(&self) -> &str {
        crate::project::builtin_config()
            .targets.get(self.engine())
            .and_then(|t| t.fig_extension.as_deref())
            .unwrap_or("png")
    }

    /// Render a list of elements into the final document body.
    fn render(&self, elements: &[Element], renderer: &ElementRenderer) -> Result<String> {
        use std::time::Instant;
        use std::sync::LazyLock;

        // Pre-collect footnote definitions across all Text elements so that
        // references in one block can resolve against definitions in another.
        renderer.collect_footnote_defs(elements);

        static TIMING_ENABLED: LazyLock<bool> = LazyLock::new(|| std::env::var("CALEPIN_TIMING").is_ok());
        let timing = *TIMING_ENABLED;
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
        Ok(body)
    }

    /// Format-specific transformation of the rendered body string.
    /// Called after render(), before cross-ref resolution.
    fn transform_body(&self, body: &str, _renderer: &ElementRenderer) -> String {
        body.to_string()
    }

    /// Wrap the rendered body in a page template. Return None to skip.
    fn assemble_page(&self, body: &str, meta: &Metadata, renderer: &ElementRenderer)
        -> Option<String>;

    /// Format-specific transformation of the complete document (after page template).
    fn transform_document(&self, document: &str, _renderer: &ElementRenderer) -> String {
        document.to_string()
    }

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
        "latex" => Ok(Box::new(latex::LatexRenderer)),
        "markdown" => Ok(Box::new(markdown::MarkdownRenderer)),
        "typst" => Ok(Box::new(typst::TypstRenderer)),
        "word" => Ok(Box::new(word::WordRenderer)),
        "revealjs" => Ok(Box::new(revealjs::RevealJsRenderer)),
        other => load_custom_format(other),
    }
}

/// Map a file extension to a canonical format name.
pub fn resolve_format_from_extension(ext: &str) -> &str {
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

/// A custom format defined via `_calepin/formats/{name}.toml`.
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

    fn engine(&self) -> &str {
        self.base.format()
    }

    fn extension(&self) -> &str {
        &self.ext
    }

    fn default_fig_ext(&self) -> &str {
        self.base.default_fig_ext()
    }

    fn transform_body(&self, body: &str, renderer: &ElementRenderer) -> String {
        self.base.transform_body(body, renderer)
    }

    fn assemble_page(
        &self,
        body: &str,
        meta: &Metadata,
        renderer: &ElementRenderer,
    ) -> Option<String> {
        // Try custom page template first: page.{name}
        let custom_tpl = crate::render::template::load_page_template(
            &format!("page.{}", self.name),
            self.base.format(),
        );
        if !custom_tpl.is_empty() {
            let base = self.base.format();
            if !matches!(base, "html" | "latex" | "typst") {
                return None;
            }
            let mut vars = crate::render::template::build_template_vars(meta, body, base);
            crate::render::template::inject_preamble(&mut vars, renderer.preamble());
            if base == "html" {
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
            Some(crate::render::template::render_page_template(&custom_tpl, &vars, base))
        } else {
            // No custom template: delegate to base format
            self.base.assemble_page(body, meta, renderer)
        }
    }

    fn transform_document(&self, document: &str, _renderer: &ElementRenderer) -> String {
        if let Some(ref script) = self.postprocess_script {
            match run_script(script, document, &[&self.name]) {
                Ok(output) => return output,
                Err(e) => {
                    eprintln!("Warning: transform_document script failed: {}", e);
                }
            }
        }
        document.to_string()
    }

    fn preprocess(&self) -> Option<&Path> {
        self.preprocess_script.as_deref()
    }
}

/// Run a script, piping `stdin_data` to its stdin and returning stdout.
/// Writes stdin in a separate thread to avoid deadlock when pipe buffers fill.
pub fn run_script(script: &Path, stdin_data: &str, args: &[&str]) -> Result<String> {
    let mut child = Command::new(script)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to run script: {}", script.display()))?;

    // Write stdin in a separate thread to prevent deadlock: if the child's
    // stdout buffer fills before we finish writing, both sides block.
    let mut stdin = child.stdin.take().unwrap();
    let data = stdin_data.to_string();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(data.as_bytes());
        // stdin is dropped here, closing the pipe
    });

    let output = child.wait_with_output()?;
    let _ = writer.join();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Script {} failed: {}", script.display(), stderr);
    }
    Ok(String::from_utf8(output.stdout)?)
}

/// Load a custom format from `_calepin/formats/{name}.toml`.
fn load_custom_format(name: &str) -> Result<Box<dyn OutputRenderer>> {
    let path = crate::paths::resolve_path_cwd("formats", &format!("{}.toml", name))
        .ok_or_else(|| anyhow::anyhow!(
            "Unknown format: '{}'. No built-in format or config at _calepin/formats/{}.toml",
            name, name
        ))?;

    let content = std::fs::read_to_string(&path)?;
    let tv: toml::Value = toml::from_str(&content)?;
    let config = crate::value::from_toml(tv);

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
