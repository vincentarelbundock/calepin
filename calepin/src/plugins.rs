//! WASM plugin system using extism.
//!
//! Plugins are `.wasm` files in `_calepin/plugins/` or `~/.config/calepin/plugins/`.
//! They export `filter` and/or `shortcode` functions that receive JSON and return JSON.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use extism::Manifest;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Shared types (used by both calepin and plugin crates via serde JSON)
// ---------------------------------------------------------------------------

/// Context passed to a plugin's `filter` function.
///
/// Used for both div filtering and span filtering.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FilterContext {
    /// `"div"` or `"span"`
    pub context: String,
    /// Rendered children (divs) or span text content (spans).
    pub content: String,
    /// CSS classes on the element.
    pub classes: Vec<String>,
    /// Element ID (empty string if none).
    pub id: String,
    /// Output format: `"html"`, `"tex"`, `"typ"`, `"md"`.
    pub format: String,
    /// Key-value attributes from the element.
    pub attrs: HashMap<String, String>,
}

/// Context passed to a plugin's `shortcode` function.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShortcodeContext {
    /// Shortcode name.
    pub name: String,
    /// Positional arguments.
    pub args: Vec<String>,
    /// Keyword arguments.
    pub kwargs: HashMap<String, String>,
    /// Output format: `"html"`, `"tex"`, `"typ"`, `"md"`.
    pub format: String,
}

/// Result from a filter call.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FilterResult {
    /// Filter produced final rendered output. Skip template.
    Rendered(String),
    /// Filter doesn't handle this element. Try next handler.
    Pass,
}

// ---------------------------------------------------------------------------
// Plugin host
// ---------------------------------------------------------------------------

pub struct PluginHandle {
    name: String,
    plugin: RefCell<extism::Plugin>,
    has_filter: bool,
    has_shortcode: bool,
    has_postprocess: bool,
    has_build_site: bool,
}

impl PluginHandle {
    pub fn call_filter(&self, ctx: &FilterContext) -> FilterResult {
        if !self.has_filter {
            return FilterResult::Pass;
        }
        let input = match serde_json::to_string(ctx) {
            Ok(s) => s,
            Err(_) => return FilterResult::Pass,
        };
        let mut plugin = self.plugin.borrow_mut();
        match plugin.call::<&str, String>("filter", &input) {
            Ok(output) => serde_json::from_str(&output).unwrap_or(FilterResult::Pass),
            Err(e) => {
                eprintln!("Warning: plugin '{}' filter error: {}", self.name, e);
                FilterResult::Pass
            }
        }
    }

    pub fn call_shortcode(&self, ctx: &ShortcodeContext) -> Option<String> {
        if !self.has_shortcode {
            return None;
        }
        let input = match serde_json::to_string(ctx) {
            Ok(s) => s,
            Err(_) => return None,
        };
        let mut plugin = self.plugin.borrow_mut();
        match plugin.call::<&str, String>("shortcode", &input) {
            Ok(output) => serde_json::from_str(&output).unwrap_or(None),
            Err(_) => None,
        }
    }

    pub fn call_build_site(&self, ctx_json: &str) -> Option<String> {
        if !self.has_build_site {
            return None;
        }
        let mut plugin = self.plugin.borrow_mut();
        match plugin.call::<&str, String>("build_site", ctx_json) {
            Ok(output) => Some(output),
            Err(e) => {
                eprintln!("Warning: plugin '{}' build_site error: {}", self.name, e);
                None
            }
        }
    }

    pub fn call_postprocess(
        &self,
        body: &str,
        format: &str,
        title: &str,
        css: &str,
    ) -> Option<String> {
        if !self.has_postprocess {
            return None;
        }
        let input = serde_json::json!({
            "body": body,
            "format": format,
            "title": title,
            "css": css,
        });
        let input_str = serde_json::to_string(&input).ok()?;
        let mut plugin = self.plugin.borrow_mut();
        match plugin.call::<&str, String>("postprocess", &input_str) {
            Ok(output) => Some(output),
            Err(e) => {
                eprintln!("Warning: plugin '{}' postprocess error: {}", self.name, e);
                None
            }
        }
    }
}

/// Resolve a plugin `.wasm` file by name.
fn resolve_plugin(name: &str) -> Option<PathBuf> {
    crate::util::resolve_path("plugins", &format!("{}.wasm", name))
}

/// Load all plugins listed by name.
pub fn load_plugins(names: &[String]) -> Vec<PluginHandle> {
    let mut handles = Vec::new();
    for name in names {
        match resolve_plugin(name) {
            Some(path) => match load_one(&path, name) {
                Ok(handle) => {
                    handles.push(handle);
                }
                Err(e) => {
                    eprintln!("Warning: failed to load plugin '{}': {}", name, e);
                }
            },
            None => {
                eprintln!("Warning: plugin '{}' not found", name);
            }
        }
    }
    handles
}

/// Load a single plugin by name. Returns None if not found.
pub fn load_plugin(name: &str) -> Option<PluginHandle> {
    let path = resolve_plugin(name)?;
    load_one(&path, name).ok()
}

fn load_one(path: &Path, name: &str) -> anyhow::Result<PluginHandle> {
    let wasm = extism::Wasm::file(path);
    let manifest = Manifest::new([wasm])
        .with_allowed_host("api.imgur.com")
        .with_allowed_host("*.imgur.com");

    let plugin = extism::Plugin::new(&manifest, [], true)?;

    let has_filter = plugin.function_exists("filter");
    let has_shortcode = plugin.function_exists("shortcode");
    let has_postprocess = plugin.function_exists("postprocess");
    let has_build_site = plugin.function_exists("build_site");

    Ok(PluginHandle {
        name: name.to_string(),
        plugin: RefCell::new(plugin),
        has_filter,
        has_shortcode,
        has_postprocess,
        has_build_site,
    })
}
