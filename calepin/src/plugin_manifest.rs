//! Plugin manifest parsing.
//!
//! Each plugin is a directory containing a `plugin.yml` file that declares
//! its capabilities: filters, shortcodes, postprocessors, element/page
//! templates, CSL styles, and custom format definitions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

// ---------------------------------------------------------------------------
// Manifest types
// ---------------------------------------------------------------------------

/// Parsed `plugin.yml` manifest.
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
    pub filter: Option<FilterSpec>,
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
    /// Match fields are OR'd: any match triggers the filter.
    pub fn matches(
        &self,
        classes: &[String],
        attrs: &HashMap<String, String>,
        id: Option<&str>,
        format: &str,
    ) -> bool {
        // Format check: if formats list is non-empty, format must be in it
        if !self.formats.is_empty() && !self.formats.iter().any(|f| f == format) {
            return false;
        }

        // Class match
        if self.classes.iter().any(|c| classes.iter().any(|cls| cls == c)) {
            return true;
        }

        // Attribute match
        if self.attrs.iter().any(|a| attrs.contains_key(a)) {
            return true;
        }

        // ID prefix match
        if let (Some(prefix), Some(id_val)) = (&self.id_prefix, id) {
            if id_val.starts_with(prefix.as_str()) {
                return true;
            }
        }

        false
    }
}

// ---------------------------------------------------------------------------
// YAML parsing
// ---------------------------------------------------------------------------

impl PluginManifest {
    /// Load a plugin manifest from a directory containing `plugin.yml`.
    pub fn load(dir: &Path) -> Result<Self> {
        let yml_path = dir.join("plugin.yml");
        let content = std::fs::read_to_string(&yml_path)
            .with_context(|| format!("Failed to read {}", yml_path.display()))?;

        use saphyr::LoadableYamlNode;
        let docs = saphyr::YamlOwned::load_from_str(&content)
            .map_err(|e| anyhow::anyhow!("YAML parse error in {}: {:?}", yml_path.display(), e))?;
        let root = docs.into_iter().next()
            .unwrap_or(saphyr::YamlOwned::BadValue);

        let name = yaml_str(&root, "name")
            .ok_or_else(|| anyhow::anyhow!("plugin.yml missing 'name' field: {}", yml_path.display()))?;

        let plugin_dir = dir.canonicalize()
            .unwrap_or_else(|_| dir.to_path_buf());

        let provides = parse_provides(&root, &plugin_dir)?;

        Ok(PluginManifest {
            name,
            version: yaml_str(&root, "version"),
            description: yaml_str(&root, "description"),
            provides,
            plugin_dir,
        })
    }
}

fn parse_provides(root: &saphyr::YamlOwned, plugin_dir: &Path) -> Result<PluginProvides> {
    let provides_node = &root["provides"];
    if provides_node.is_badvalue() {
        return Ok(PluginProvides::default());
    }

    Ok(PluginProvides {
        filter: parse_filter_spec(provides_node, plugin_dir),
        shortcode: parse_shortcode_spec(provides_node, plugin_dir),
        postprocess: parse_postprocess_spec(provides_node, plugin_dir),
        elements: parse_elements_spec(provides_node),
        templates: parse_templates_spec(provides_node),
        csl: yaml_str(provides_node, "csl"),
        format: parse_format_spec(provides_node, plugin_dir),
    })
}

fn parse_filter_spec(provides: &saphyr::YamlOwned, plugin_dir: &Path) -> Option<FilterSpec> {
    let node = &provides["filter"];
    if node.is_badvalue() {
        return None;
    }

    let run = yaml_str(node, "run").map(|s| plugin_dir.join(s));

    let match_node = &node["match"];
    let match_rule = if match_node.is_badvalue() {
        FilterMatch::default()
    } else {
        FilterMatch {
            classes: yaml_str_vec(match_node, "classes"),
            attrs: yaml_str_vec(match_node, "attrs"),
            id_prefix: yaml_str(match_node, "id_prefix"),
            formats: yaml_str_vec(match_node, "formats"),
        }
    };

    let contexts = {
        let v = yaml_str_vec(node, "contexts");
        if v.is_empty() {
            vec!["div".to_string(), "span".to_string()]
        } else {
            v
        }
    };

    let persistent = yaml_str(node, "persistent")
        .map(|s| s == "true")
        .unwrap_or(false);

    Some(FilterSpec { run, match_rule, contexts, persistent })
}

fn parse_shortcode_spec(provides: &saphyr::YamlOwned, plugin_dir: &Path) -> Option<ShortcodeSpec> {
    let node = &provides["shortcode"];
    if node.is_badvalue() {
        return None;
    }

    Some(ShortcodeSpec {
        run: yaml_str(node, "run").map(|s| plugin_dir.join(s)),
        names: yaml_str_vec(node, "names"),
    })
}

fn parse_postprocess_spec(provides: &saphyr::YamlOwned, plugin_dir: &Path) -> Option<PostprocessSpec> {
    let node = &provides["postprocess"];
    if node.is_badvalue() {
        return None;
    }

    Some(PostprocessSpec {
        run: yaml_str(node, "run").map(|s| plugin_dir.join(s)),
        formats: yaml_str_vec(node, "formats"),
    })
}

fn parse_elements_spec(provides: &saphyr::YamlOwned) -> Option<ElementsSpec> {
    let node = &provides["elements"];
    if node.is_badvalue() {
        return None;
    }
    yaml_str(node, "dir").map(|s| ElementsSpec { dir: PathBuf::from(s) })
}

fn parse_templates_spec(provides: &saphyr::YamlOwned) -> Option<TemplatesSpec> {
    let node = &provides["templates"];
    if node.is_badvalue() {
        return None;
    }
    yaml_str(node, "dir").map(|s| TemplatesSpec { dir: PathBuf::from(s) })
}

fn parse_format_spec(provides: &saphyr::YamlOwned, plugin_dir: &Path) -> Option<FormatSpec> {
    let node = &provides["format"];
    if node.is_badvalue() {
        return None;
    }

    let name = yaml_str(node, "name")?;
    let base = yaml_str(node, "base")?;

    Some(FormatSpec {
        name,
        base,
        extension: yaml_str(node, "extension"),
        preprocess: yaml_str(node, "preprocess").map(|s| plugin_dir.join(s)),
        postprocess: yaml_str(node, "postprocess").map(|s| plugin_dir.join(s)),
    })
}

// ---------------------------------------------------------------------------
// YAML helpers
// ---------------------------------------------------------------------------

fn yaml_str(node: &saphyr::YamlOwned, key: &str) -> Option<String> {
    node[key].as_str().map(|s| s.to_string())
}

fn yaml_str_vec(node: &saphyr::YamlOwned, key: &str) -> Vec<String> {
    let arr = &node[key];
    if arr.is_badvalue() {
        return Vec::new();
    }
    match arr.as_vec() {
        Some(items) => items.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        None => {
            // Single string → one-element vec
            arr.as_str().map(|s| vec![s.to_string()]).unwrap_or_default()
        }
    }
}
