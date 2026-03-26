//! Plugin manifest parsing.
//!
//! Each plugin is a directory containing a `module.toml` file
//! that declares its capabilities: filters, element/page templates,
//! CSL styles, and custom format definitions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use crate::value::{self, Value, value_string_list};

// ---------------------------------------------------------------------------
// Manifest types
// ---------------------------------------------------------------------------

/// Parsed module manifest.
#[allow(dead_code)]
pub struct ModuleManifest {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub provides: ModuleProvides,
    /// Absolute path to the plugin directory.
    pub module_dir: PathBuf,
}

/// All capabilities a plugin can provide (all optional).
#[derive(Default)]
#[allow(dead_code)]
pub struct ModuleProvides {
    /// Multiple filters, each with its own match rules and executable.
    pub matchers: Vec<MatchSpec>,
    pub elements: Option<ElementsSpec>,
    pub partials: Option<PartialsSpec>,
    pub csl: Option<String>,
    pub format: Option<FormatSpec>,
    /// Script for document transforms (receives document on stdin, writes to stdout).
    pub document_script: Option<PathBuf>,
}

/// Filter specification: executable path, match rules, contexts.
#[allow(dead_code)]
pub struct MatchSpec {
    /// Path to executable, relative to plugin dir. None for built-in plugins.
    pub run: Option<PathBuf>,
    /// Rules that determine when this filter is dispatched.
    pub match_rule: MatchRule,
    /// Which contexts this filter handles: "div", "span", or both.
    pub contexts: Vec<String>,
}

/// Rules for matching a filter to a div/span element.
#[derive(Default)]
pub struct MatchRule {
    /// CSS classes that trigger this filter (OR'd).
    pub classes: Vec<String>,
    /// Attribute names whose presence triggers this filter (OR'd).
    pub attrs: Vec<String>,
    /// ID prefix that triggers this filter.
    pub id_prefix: Option<String>,
    /// Output formats this filter applies to. Empty = all formats.
    pub formats: Vec<String>,
}

/// Element template directory specification.
pub struct ElementsSpec {
    /// Directory containing `{name}.{format}` template files, relative to plugin dir.
    pub dir: PathBuf,
}

/// Page template directory specification.
#[allow(dead_code)]
pub struct PartialsSpec {
    /// Directory containing `calepin.{format}` and `calepin.css`, relative to plugin dir.
    pub dir: PathBuf,
}

/// Custom format specification.
#[allow(dead_code)]
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

impl MatchRule {
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

impl ModuleManifest {
    /// Load a module manifest from a directory.
    /// Load a module manifest from `module.toml` in the given directory.
    pub fn load(dir: &Path) -> Result<Self> {
        let toml_path = dir.join("module.toml");

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

        let module_dir = dir.canonicalize()
            .unwrap_or_else(|_| dir.to_path_buf());

        let provides = parse_provides(&root, &module_dir)?;

        Ok(ModuleManifest {
            name,
            version: root.get("version").and_then(|v| v.as_str()).map(String::from),
            description: root.get("description").and_then(|v| v.as_str()).map(String::from),
            provides,
            module_dir,
        })
    }
}

fn parse_provides(root: &Value, module_dir: &Path) -> Result<ModuleProvides> {
    // Parse [document].run script path
    let document_script = root.get("document")
        .and_then(|d| d.get("run"))
        .and_then(|v| v.as_str())
        .map(|s| module_dir.join(s));

    Ok(ModuleProvides {
        matchers: parse_match_specs(root, module_dir),
        elements: parse_elements_spec(root),
        partials: parse_partials_spec(root),
        csl: root.get("csl").and_then(|v| v.as_str()).map(String::from),
        format: parse_format_spec(root, module_dir),
        document_script,
    })
}

fn parse_match_specs(provides: &Value, module_dir: &Path) -> Vec<MatchSpec> {
    let mut specs = Vec::new();

    // Try plural `filters:` (array)
    if let Some(filters_node) = provides.get("filters") {
        if let Some(items) = filters_node.as_array() {
            for item in items {
                if let Some(spec) = parse_one_match_spec(item, module_dir) {
                    specs.push(spec);
                }
            }
        }
    }

    // Try singular `filter:` (single object)
    if let Some(filter_node) = provides.get("filter") {
        if let Some(spec) = parse_one_match_spec(filter_node, module_dir) {
            specs.push(spec);
        }
    }

    specs
}

fn parse_one_match_spec(node: &Value, module_dir: &Path) -> Option<MatchSpec> {
    let run = node.get("run")
        .and_then(|v| v.as_str())
        .map(|s| module_dir.join(s));

    let match_rule = match node.get("match") {
        Some(match_node) => MatchRule {
            classes: val_str_vec(match_node, "classes"),
            attrs: val_str_vec(match_node, "attrs").into_iter().map(|a| crate::util::normalize_key(&a)).collect(),
            id_prefix: match_node.get("id_prefix").and_then(|v| v.as_str()).map(String::from),
            formats: val_str_vec(match_node, "formats"),
        },
        None => MatchRule::default(),
    };

    let contexts = {
        let v = val_str_vec(node, "contexts");
        if v.is_empty() {
            vec!["div".to_string(), "span".to_string()]
        } else {
            v
        }
    };

    Some(MatchSpec { run, match_rule, contexts })
}

fn parse_elements_spec(provides: &Value) -> Option<ElementsSpec> {
    let node = provides.get("elements")?;
    node.get("dir").and_then(|v| v.as_str()).map(|s| ElementsSpec { dir: PathBuf::from(s) })
}

fn parse_partials_spec(provides: &Value) -> Option<PartialsSpec> {
    let node = provides.get("templates")?;
    node.get("dir").and_then(|v| v.as_str()).map(|s| PartialsSpec { dir: PathBuf::from(s) })
}

fn parse_format_spec(provides: &Value, module_dir: &Path) -> Option<FormatSpec> {
    let node = provides.get("format")?;
    let name = node.get("name")?.as_str()?.to_string();
    let base = node.get("base")?.as_str()?.to_string();

    Some(FormatSpec {
        name,
        base,
        extension: node.get("extension").and_then(|v| v.as_str()).map(String::from),
        preprocess: node.get("preprocess").and_then(|v| v.as_str()).map(|s| module_dir.join(s)),
        postprocess: node.get("postprocess").and_then(|v| v.as_str()).map(|s| module_dir.join(s)),
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

