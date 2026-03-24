//! Plugin manifest parsing.
//!
//! Each plugin is a directory containing a `plugin.toml` file
//! that declares its capabilities: filters,
//! shortcodes, postprocessors, element/page templates, CSL styles, and
//! custom format definitions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use crate::value::{self, Value, value_string_list};

// ---------------------------------------------------------------------------
// Manifest types
// ---------------------------------------------------------------------------

/// Parsed plugin manifest.
pub struct PluginManifest {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub provides: PluginProvides,
    /// Absolute path to the plugin directory.
    pub plugin_dir: PathBuf,
}

/// All capabilities a plugin can provide (all optional).
#[derive(Default)]
pub struct PluginProvides {
    /// Multiple filters, each with its own match rules and executable.
    pub filters: Vec<FilterSpec>,
    pub shortcode: Option<ShortcodeSpec>,
    pub postprocess: Option<PostprocessSpec>,
    pub elements: Option<ElementsSpec>,
    pub templates: Option<TemplatesSpec>,
    pub csl: Option<String>,
    pub format: Option<FormatSpec>,
}

/// Filter specification: executable path, match rules, contexts.
pub struct FilterSpec {
    /// Path to executable, relative to plugin dir. None for built-in plugins.
    pub run: Option<PathBuf>,
    /// Rules that determine when this filter is dispatched.
    pub match_rule: FilterMatch,
    /// Which contexts this filter handles: "div", "span", or both.
    pub contexts: Vec<String>,
    /// If true, use persistent JSON-lines subprocess protocol.
    pub persistent: bool,
}

/// Rules for matching a filter to a div/span element.
#[derive(Default)]
pub struct FilterMatch {
    /// CSS classes that trigger this filter (OR'd).
    pub classes: Vec<String>,
    /// Attribute names whose presence triggers this filter (OR'd).
    pub attrs: Vec<String>,
    /// ID prefix that triggers this filter.
    pub id_prefix: Option<String>,
    /// Output formats this filter applies to. Empty = all formats.
    pub formats: Vec<String>,
}

/// Shortcode specification.
pub struct ShortcodeSpec {
    /// Path to executable, relative to plugin dir. None for built-in.
    pub run: Option<PathBuf>,
    /// Shortcode names this plugin handles.
    pub names: Vec<String>,
}

/// Postprocess specification.
pub struct PostprocessSpec {
    /// Path to executable, relative to plugin dir.
    pub run: Option<PathBuf>,
    /// Output formats this postprocessor applies to. Empty = all.
    pub formats: Vec<String>,
}

/// Element template directory specification.
pub struct ElementsSpec {
    /// Directory containing `{name}.{format}` template files, relative to plugin dir.
    pub dir: PathBuf,
}

/// Page template directory specification.
pub struct TemplatesSpec {
    /// Directory containing `calepin.{format}` and `calepin.css`, relative to plugin dir.
    pub dir: PathBuf,
}

/// Custom format specification.
pub struct FormatSpec {
    pub name: String,
    pub base: String,
    pub extension: Option<String>,
    pub preprocess: Option<PathBuf>,
    pub postprocess: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Matching
// ---------------------------------------------------------------------------

impl FilterMatch {
    /// Check if this match rule applies to the given element properties.
    pub fn matches(
        &self,
        classes: &[String],
        attrs: &HashMap<String, String>,
        id: Option<&str>,
        format: &str,
    ) -> bool {
        if !self.formats.is_empty() && !self.formats.iter().any(|f| f == format) {
            return false;
        }
        if self.classes.iter().any(|c| classes.iter().any(|cls| cls == c)) {
            return true;
        }
        if self.attrs.iter().any(|a| attrs.contains_key(a)) {
            return true;
        }
        if let (Some(prefix), Some(id_val)) = (&self.id_prefix, id) {
            if id_val.starts_with(prefix.as_str()) {
                return true;
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

impl PluginManifest {
    /// Load a plugin manifest from a directory.
    /// Load a plugin manifest from `plugin.toml` in the given directory.
    pub fn load(dir: &Path) -> Result<Self> {
        let toml_path = dir.join("plugin.toml");

        let content = std::fs::read_to_string(&toml_path)
            .with_context(|| format!("Failed to read {}", toml_path.display()))?;

        let root = {
            let tv: toml::Value = toml::from_str(&content)
                .map_err(|e| anyhow::anyhow!("TOML parse error in {}: {}", toml_path.display(), e))?;
            value::from_toml(tv)
        };

        let manifest_path = &toml_path;

        let name = root.get("name")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| anyhow::anyhow!("Plugin manifest missing 'name' field: {}", manifest_path.display()))?;

        let plugin_dir = dir.canonicalize()
            .unwrap_or_else(|_| dir.to_path_buf());

        let provides = parse_provides(&root, &plugin_dir)?;

        Ok(PluginManifest {
            name,
            version: root.get("version").and_then(|v| v.as_str()).map(String::from),
            description: root.get("description").and_then(|v| v.as_str()).map(String::from),
            provides,
            plugin_dir,
        })
    }
}

fn parse_provides(root: &Value, plugin_dir: &Path) -> Result<PluginProvides> {
    // New flat format: [[filter]], [[shortcode]], [[postprocess]] at root level.
    // Old format: [provides.filter], [provides.shortcode], etc. under [provides].
    // Accept both during transition.

    let has_flat = root.get("filter").is_some()
        || root.get("shortcode").is_some()
        || root.get("postprocess").is_some();

    if has_flat {
        // New flat format
        return Ok(PluginProvides {
            filters: parse_filter_specs(root, plugin_dir),
            shortcode: parse_shortcode_spec(root, plugin_dir),
            postprocess: parse_postprocess_spec(root, plugin_dir),
            elements: parse_elements_spec(root),
            templates: parse_templates_spec(root),
            csl: root.get("csl").and_then(|v| v.as_str()).map(String::from),
            format: parse_format_spec(root, plugin_dir),
        });
    }

    // Legacy [provides] format
    let provides_node = match root.get("provides") {
        Some(v) => v,
        None => return Ok(PluginProvides::default()),
    };

    Ok(PluginProvides {
        filters: parse_filter_specs(provides_node, plugin_dir),
        shortcode: parse_shortcode_spec(provides_node, plugin_dir),
        postprocess: parse_postprocess_spec(provides_node, plugin_dir),
        elements: parse_elements_spec(provides_node),
        templates: parse_templates_spec(provides_node),
        csl: provides_node.get("csl").and_then(|v| v.as_str()).map(String::from),
        format: parse_format_spec(provides_node, plugin_dir),
    })
}

fn parse_filter_specs(provides: &Value, plugin_dir: &Path) -> Vec<FilterSpec> {
    let mut specs = Vec::new();

    // Try plural `filters:` (array)
    if let Some(filters_node) = provides.get("filters") {
        if let Some(items) = filters_node.as_array() {
            for item in items {
                if let Some(spec) = parse_one_filter_spec(item, plugin_dir) {
                    specs.push(spec);
                }
            }
        }
    }

    // Try singular `filter:` (single object)
    if let Some(filter_node) = provides.get("filter") {
        if let Some(spec) = parse_one_filter_spec(filter_node, plugin_dir) {
            specs.push(spec);
        }
    }

    specs
}

fn parse_one_filter_spec(node: &Value, plugin_dir: &Path) -> Option<FilterSpec> {
    let run = node.get("run")
        .and_then(|v| v.as_str())
        .map(|s| plugin_dir.join(s));

    let match_rule = match node.get("match") {
        Some(match_node) => FilterMatch {
            classes: val_str_vec_alias(match_node, "class", "classes"),
            attrs: val_str_vec_alias(match_node, "attr", "attrs"),
            id_prefix: match_node.get("id_prefix").and_then(|v| v.as_str()).map(String::from),
            formats: val_str_vec_alias(match_node, "formats", "format"),
        },
        None => FilterMatch::default(),
    };

    let contexts = {
        let v = val_str_vec_alias(node, "context", "contexts");
        if v.is_empty() {
            vec!["div".to_string(), "span".to_string()]
        } else {
            v
        }
    };

    let persistent = node.get("persistent")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Some(FilterSpec { run, match_rule, contexts, persistent })
}

fn parse_shortcode_spec(provides: &Value, plugin_dir: &Path) -> Option<ShortcodeSpec> {
    let node = provides.get("shortcode")?;
    Some(ShortcodeSpec {
        run: node.get("run").and_then(|v| v.as_str()).map(|s| plugin_dir.join(s)),
        names: val_str_vec_alias(node, "name", "names"),
    })
}

fn parse_postprocess_spec(provides: &Value, plugin_dir: &Path) -> Option<PostprocessSpec> {
    let node = provides.get("postprocess")?;
    Some(PostprocessSpec {
        run: node.get("run").and_then(|v| v.as_str()).map(|s| plugin_dir.join(s)),
        formats: val_str_vec(node, "formats"),
    })
}

fn parse_elements_spec(provides: &Value) -> Option<ElementsSpec> {
    let node = provides.get("elements")?;
    node.get("dir").and_then(|v| v.as_str()).map(|s| ElementsSpec { dir: PathBuf::from(s) })
}

fn parse_templates_spec(provides: &Value) -> Option<TemplatesSpec> {
    let node = provides.get("templates")?;
    node.get("dir").and_then(|v| v.as_str()).map(|s| TemplatesSpec { dir: PathBuf::from(s) })
}

fn parse_format_spec(provides: &Value, plugin_dir: &Path) -> Option<FormatSpec> {
    let node = provides.get("format")?;
    let name = node.get("name")?.as_str()?.to_string();
    let base = node.get("base")?.as_str()?.to_string();

    Some(FormatSpec {
        name,
        base,
        extension: node.get("extension").and_then(|v| v.as_str()).map(String::from),
        preprocess: node.get("preprocess").and_then(|v| v.as_str()).map(|s| plugin_dir.join(s)),
        postprocess: node.get("postprocess").and_then(|v| v.as_str()).map(|s| plugin_dir.join(s)),
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn val_str_vec(node: &Value, key: &str) -> Vec<String> {
    match node.get(key) {
        Some(v) => value_string_list(v),
        None => Vec::new(),
    }
}

/// Try `primary` key first, then `fallback`. Supports both singular (string)
/// and plural (array) forms, e.g., `class = ["note"]` or `classes = ["note"]`.
fn val_str_vec_alias(node: &Value, primary: &str, fallback: &str) -> Vec<String> {
    let v = val_str_vec(node, primary);
    if !v.is_empty() { return v; }
    val_str_vec(node, fallback)
}
