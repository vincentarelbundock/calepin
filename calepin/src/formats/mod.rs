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

use anyhow::Result;

use crate::types::{Element, Metadata};
use crate::render::elements::ElementRenderer;
use crate::plugins::PluginHandle;

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
        "png"
    }

    /// Render a list of elements into the final document body.
    fn render(&self, elements: &[Element], renderer: &ElementRenderer) -> Result<String> {
        let parts: Vec<String> = elements
            .iter()
            .map(|el| renderer.render(el))
            .filter(|s| !s.is_empty())
            .collect();
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
}

/// Create a renderer from a format name string.
/// Checks built-in formats first, then custom format configs.
pub fn create_renderer(format: &str) -> Result<Box<dyn OutputRenderer>> {
    match format {
        "html" => Ok(Box::new(html::HtmlRenderer)),
        "latex" | "tex" => Ok(Box::new(latex::LatexRenderer)),
        "markdown" | "md" | "reprex" => Ok(Box::new(markdown::MarkdownRenderer)),
        "typst" | "typ" => Ok(Box::new(typst::TypstRenderer)),
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
    postprocess_plugin: Option<PluginHandle>,
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
        // If a postprocess plugin exists, pass it the body + metadata
        if let Some(ref plugin) = self.postprocess_plugin {
            let title = meta.title.as_deref().unwrap_or("");
            let syntax_css = renderer.syntax_css();
            return plugin.call_postprocess(body, &self.name, title, &syntax_css);
        }

        // Try custom page template first: calepin.{name}
        let custom_tpl = crate::render::template::load_page_template(
            &format!("calepin.{}", self.name),
        );
        if !custom_tpl.is_empty() {
            let vars = match self.base.format() {
                "html" => crate::render::template::build_html_vars(meta, body),
                "latex" => crate::render::template::build_latex_vars(meta, body),
                "typst" => crate::render::template::build_typst_vars(meta, body),
                _ => return None,
            };
            return Some(crate::render::template::apply_template(&custom_tpl, &vars));
        }

        // Fall back to base format's template
        self.base.apply_template(body, meta, renderer)
    }
}

/// Load a custom format from `_calepin/formats/{name}.yaml`.
fn load_custom_format(name: &str) -> Result<Box<dyn OutputRenderer>> {
    let config_file = format!("{}.yaml", name);
    let path = crate::util::resolve_path("formats", &config_file)
        .ok_or_else(|| anyhow::anyhow!(
            "Unknown format: '{}'. No built-in format or config at _calepin/formats/{}.yaml",
            name, name
        ))?;

    let content = std::fs::read_to_string(&path)?;
    use saphyr::LoadableYamlNode;
    let docs = saphyr::YamlOwned::load_from_str(&content)?;
    let config = docs.into_iter().next().unwrap_or(saphyr::YamlOwned::BadValue);

    let base_name = config["base"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Custom format '{}': missing 'base' field", name))?;

    // Create the base renderer (must be a built-in)
    let base: Box<dyn OutputRenderer> = match base_name {
        "html" => Box::new(html::HtmlRenderer),
        "latex" => Box::new(latex::LatexRenderer),
        "markdown" => Box::new(markdown::MarkdownRenderer),
        "typst" => Box::new(typst::TypstRenderer),
        other => anyhow::bail!("Custom format '{}': unknown base format '{}'", name, other),
    };

    let ext = config["extension"]
        .as_str()
        .unwrap_or(base.extension())
        .to_string();

    let postprocess_plugin = config["plugin"]
        .as_str()
        .and_then(|plugin_name| {
            match crate::plugins::load_plugin(plugin_name) {
                Some(p) => Some(p),
                None => {
                    eprintln!("Warning: format plugin '{}' not found", plugin_name);
                    None
                }
            }
        });

    Ok(Box::new(CustomRenderer {
        name: name.to_string(),
        ext,
        base,
        postprocess_plugin,
    }))
}
