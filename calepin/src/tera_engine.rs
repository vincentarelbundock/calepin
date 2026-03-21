// Tera-based body processing.
//
// - expand_includes()      — Pre-parse `{% include "file" %}` expansion.
// - process_body()         — Main entry: Tera-render a text block (code-block-safe).
// - protect_code_blocks()  — Extract fenced/inline code before Tera evaluation.
// - restore_code_blocks()  — Re-insert protected code after Tera evaluation.
//
// Built-in Tera functions:
//   pagebreak(), env(name), video(url, ...), brand(type, name, mode?), kbd(keys)
//
// Context variables:
//   meta.title, meta.author, meta.date, ...  — from Metadata
//   var.key.subkey                            — from front matter `variables:` block
//   format                                   — current output format

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use regex::Regex;
use serde_json;
use tera::{self, Tera, Context, Value};

use crate::registry::PluginRegistry;
use crate::render::markers;
use crate::types::Metadata;

// ---------------------------------------------------------------------------
// Pre-parse include expansion
// ---------------------------------------------------------------------------

/// Expand `{% include "file" %}` directives before block parsing.
/// This must run on the raw body text so that included content gets
/// parsed as blocks (code chunks, divs, etc.) rather than inline text.
#[inline(never)]
pub fn expand_includes(text: &str) -> String {
    // {% include "file" %} or {% include 'file' %}
    static INCLUDE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"\{%[-\s]\s*include\s+["'](.+?)["']\s*[-\s]?%\}"#).unwrap()
    });
    // {% raw %} ... {% endraw %} blocks (protect from include expansion)
    static RAW_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{%-?\s*raw\s*-?%\}[\s\S]*?\{%-?\s*endraw\s*-?%\}").unwrap()
    });

    // Protect {% raw %} blocks from include expansion
    let mut raw_blocks = Vec::new();
    let text = RAW_RE.replace_all(text, |caps: &regex::Captures| {
        let idx = raw_blocks.len();
        raw_blocks.push(caps[0].to_string());
        format!("\u{FDD2}RAW{}\u{FDD3}", idx)
    }).to_string();

    // Expand includes
    let text = INCLUDE_RE.replace_all(&text, |caps: &regex::Captures| {
        let path = caps[1].trim();
        include_file(path)
    }).to_string();

    // Restore {% raw %} blocks
    let mut result = text;
    for (idx, block) in raw_blocks.iter().enumerate() {
        result = result.replace(&format!("\u{FDD2}RAW{}\u{FDD3}", idx), block);
    }
    result
}

/// Read and include a file, stripping YAML front matter if present.
fn include_file(path: &str) -> String {
    if path.is_empty() {
        cwarn!("{{% include %}} requires a file path");
        return String::new();
    }
    match std::fs::read_to_string(path) {
        Ok(content) => {
            // Strip YAML front matter if present
            if content.starts_with("---") {
                if let Some(end) = content[3..].find("\n---") {
                    let after = end + 3 + 4; // skip past closing ---
                    return content[after..].to_string();
                }
            }
            content
        }
        Err(e) => {
            cwarn!("{{% include \"{}\" %}}: {}", path, e);
            String::new()
        }
    }
}

// ---------------------------------------------------------------------------
// Tera body processing
// ---------------------------------------------------------------------------

/// Result of Tera body processing.
pub struct BodyTeraResult {
    pub text: String,
    pub sc_fragments: Vec<String>,
}

/// Process a text block through Tera, evaluating functions and variable references.
#[inline(never)]
pub fn process_body(
    text: &str,
    format: &str,
    metadata: &Metadata,
    registry: &PluginRegistry,
) -> BodyTeraResult {
    let fragments = Arc::new(Mutex::new(Vec::new()));

    // 1. Protect code blocks and inline code from Tera
    let (protected, code_blocks) = protect_code_blocks(text);

    // 1b. Escape heading attribute syntax {#id .class} which Tera
    //     interprets as comment openers ({# ... #}).
    static RE_HEADING_ATTR: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{(#[a-zA-Z][a-zA-Z0-9_-]*(?:\s+\.[a-zA-Z][a-zA-Z0-9_-]*)*)\}").unwrap()
    });
    let protected = RE_HEADING_ATTR.replace_all(&protected, "\u{FDD2}$1\u{FDD3}").to_string();

    // Quick exit: if no Tera syntax found, skip processing
    if !protected.contains("{{") && !protected.contains("{%") {
        return BodyTeraResult {
            text: text.to_string(),
            sc_fragments: Vec::new(),
        };
    }

    // 2. Build Tera instance with custom functions
    let mut tera = Tera::default();
    tera.register_function("pagebreak", PagebreakFn::new(format, &fragments));
    tera.register_function("env", EnvFn);
    tera.register_function("video", VideoFn::new(format, &fragments));
    tera.register_function("brand", BrandFn::new(format, &fragments, metadata.brand.as_ref()));
    tera.register_function("kbd", KbdFn::new(format, &fragments));

    // Register plugin shortcodes as Tera functions
    for (name, plugin_idx) in registry.shortcode_names() {
        let meta_json = serde_json::json!({
            "title": metadata.title,
            "author": metadata.author,
            "date": metadata.date,
        });
        tera.register_function(
            &name,
            PluginShortcodeFn::new(
                name.clone(),
                plugin_idx,
                format.to_string(),
                meta_json,
                Arc::clone(&fragments),
                registry,
            ),
        );
    }

    // 3. Build context with metadata and variables
    let mut context = Context::new();
    context.insert("format", format);
    context.insert("meta", &build_meta_map(metadata));
    context.insert("var", &build_variables_map(metadata));

    // 4. Render through Tera (on error, fall back to protected text so that
    //    restore_code_blocks can still recover code block placeholders)
    let rendered = match tera.add_raw_template("__body__", &protected) {
        Ok(()) => match tera.render("__body__", &context) {
            Ok(r) => r,
            Err(e) => {
                cwarn!("body template error: {}", e);
                protected.clone()
            }
        },
        Err(e) => {
            cwarn!("body template parse error: {}", e);
            protected.clone()
        }
    };

    // 5. Restore protected content
    let rendered = rendered.replace('\u{FDD2}', "{").replace('\u{FDD3}', "}");
    let text = restore_code_blocks(&rendered, &code_blocks);

    let sc_fragments = match Arc::try_unwrap(fragments) {
        Ok(mutex) => mutex.into_inner().unwrap(),
        Err(arc) => arc.lock().unwrap().clone(),
    };

    BodyTeraResult { text, sc_fragments }
}

// ---------------------------------------------------------------------------
// Code block protection
// ---------------------------------------------------------------------------

/// Placeholder prefix for protected code blocks (uses Unicode noncharacters).
const CODE_PLACEHOLDER_PREFIX: &str = "\u{FDD0}CODE";
const CODE_PLACEHOLDER_SUFFIX: &str = "\u{FDD1}";

/// Extract fenced code blocks and inline code spans, replacing them with
/// placeholders that Tera won't try to evaluate.
fn protect_code_blocks(text: &str) -> (String, Vec<String>) {
    let mut blocks: Vec<String> = Vec::new();
    let mut result = String::new();

    // First pass: protect fenced code blocks
    let mut in_fence = false;
    let mut fence_marker = String::new();
    let mut fence_content = String::new();

    for line in text.split('\n') {
        let trimmed = line.trim_start();
        if !in_fence {
            // Check for opening fence (3+ backticks or tildes)
            if let Some(marker) = detect_fence_open(trimmed) {
                in_fence = true;
                fence_marker = marker;
                fence_content = line.to_string();
                fence_content.push('\n');
                continue;
            }
            result.push_str(line);
            result.push('\n');
        } else {
            fence_content.push_str(line);
            fence_content.push('\n');
            // Check for closing fence (same marker)
            if trimmed.starts_with(&fence_marker) && trimmed.trim_end().len() <= fence_marker.len() + 1 {
                // Fence closed — store and emit placeholder
                let idx = blocks.len();
                // Remove trailing newline from content
                if fence_content.ends_with('\n') {
                    fence_content.pop();
                }
                blocks.push(fence_content.clone());
                result.push_str(&format!("{}{}{}", CODE_PLACEHOLDER_PREFIX, idx, CODE_PLACEHOLDER_SUFFIX));
                result.push('\n');
                fence_content.clear();
                in_fence = false;
            }
        }
    }
    // Handle unclosed fence (shouldn't happen in valid qmd)
    if in_fence {
        result.push_str(&fence_content);
    }

    // Remove trailing newline added by split/join
    if result.ends_with('\n') && !text.ends_with('\n') {
        result.pop();
    }

    // Second pass: protect inline code spans
    let result = protect_inline_code(&result, &mut blocks);

    (result, blocks)
}

/// Detect a fenced code block opening (3+ backticks or tildes).
fn detect_fence_open(trimmed: &str) -> Option<String> {
    let ch = trimmed.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let count = trimmed.chars().take_while(|&c| c == ch).count();
    if count >= 3 {
        Some(std::iter::repeat(ch).take(count).collect())
    } else {
        None
    }
}

/// Replace inline code spans (`` `...` ``) with placeholders.
/// Only protects spans that contain Tera-like syntax.
fn protect_inline_code(text: &str, blocks: &mut Vec<String>) -> String {
    let mut result = String::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '`' {
            // Count opening backticks
            let start = i;
            let mut tick_count = 0;
            while i < len && chars[i] == '`' {
                tick_count += 1;
                i += 1;
            }
            // Find matching closing backticks
            let mut found_end = false;
            let _content_start = i;
            while i <= len - tick_count {
                if chars[i] == '`' {
                    let mut closing = 0;
                    let _close_start = i;
                    while i < len && chars[i] == '`' {
                        closing += 1;
                        i += 1;
                    }
                    if closing == tick_count {
                        // Found matching close
                        let full: String = chars[start..i].iter().collect();
                        if full.contains("{{") || full.contains("{%") || full.contains("{#") {
                            let idx = blocks.len();
                            blocks.push(full);
                            result.push_str(&format!("{}{}{}", CODE_PLACEHOLDER_PREFIX, idx, CODE_PLACEHOLDER_SUFFIX));
                        } else {
                            result.push_str(&full);
                        }
                        found_end = true;
                        break;
                    }
                    // Not matching — continue searching
                } else {
                    i += 1;
                }
            }
            if !found_end {
                // No matching close — emit as-is
                let unmatched: String = chars[start..i].iter().collect();
                result.push_str(&unmatched);
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

/// Restore protected code blocks from placeholders.
fn restore_code_blocks(text: &str, blocks: &[String]) -> String {
    if blocks.is_empty() || !text.contains(CODE_PLACEHOLDER_PREFIX) {
        return text.to_string();
    }
    static RE_PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(&format!(
            "{}(\\d+){}",
            regex::escape(CODE_PLACEHOLDER_PREFIX),
            regex::escape(CODE_PLACEHOLDER_SUFFIX)
        )).unwrap()
    });
    RE_PLACEHOLDER.replace_all(text, |caps: &regex::Captures| {
        let idx: usize = caps[1].parse().unwrap_or(usize::MAX);
        blocks.get(idx).cloned().unwrap_or_default()
    }).to_string()
}

// ---------------------------------------------------------------------------
// Marker wrapping for format-specific output
// ---------------------------------------------------------------------------

/// Wrap output in shortcode markers if needed (for LaTeX/Typst protection).
fn wrap_if_needed(output: &str, format: &str, fragments: &Arc<Mutex<Vec<String>>>) -> String {
    match format {
        "html" | "markdown" | "md" => output.to_string(),
        _ => {
            let mut frags = fragments.lock().unwrap();
            markers::wrap_shortcode_raw(&mut frags, output.to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// Metadata → Tera context helpers
// ---------------------------------------------------------------------------

/// Build a serde_json::Value map from Metadata for the `meta` context variable.
fn build_meta_map(meta: &Metadata) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    if let Some(ref t) = meta.title {
        map.insert("title".into(), serde_json::Value::String(t.clone()));
    }
    if let Some(ref s) = meta.subtitle {
        map.insert("subtitle".into(), serde_json::Value::String(s.clone()));
    }
    if let Some(ref authors) = meta.author {
        map.insert("author".into(), serde_json::json!(authors.join(", ")));
    }
    if let Some(ref d) = meta.date {
        map.insert("date".into(), serde_json::Value::String(d.clone()));
    }
    if let Some(ref abs) = meta.abstract_text {
        map.insert("abstract".into(), serde_json::Value::String(abs.clone()));
    }
    if !meta.keywords.is_empty() {
        map.insert("keywords".into(), serde_json::Value::String(meta.keywords.join(", ")));
    }
    // Extra metadata fields
    for (key, value) in &meta.extra {
        if let Some(s) = value.as_str() {
            map.insert(key.clone(), serde_json::Value::String(s.to_string()));
        } else if let Some(b) = value.as_bool() {
            map.insert(key.clone(), serde_json::Value::Bool(b));
        } else if let Some(n) = value.as_integer() {
            map.insert(key.clone(), serde_json::json!(n));
        } else if let Some(f) = value.as_floating_point() {
            map.insert(key.clone(), serde_json::json!(f));
        }
    }
    serde_json::Value::Object(map)
}

/// Build the `var` context from front matter `variables:` block.
fn build_variables_map(metadata: &Metadata) -> serde_json::Value {
    match &metadata.variables {
        Some(yaml) => yaml_to_json(yaml),
        None => serde_json::Value::Object(serde_json::Map::new()),
    }
}

/// Convert a saphyr YAML value to serde_json::Value.
fn yaml_to_json(yaml: &saphyr::YamlOwned) -> serde_json::Value {
    if let Some(s) = yaml.as_str() {
        return serde_json::Value::String(s.to_string());
    }
    if let Some(b) = yaml.as_bool() {
        return serde_json::Value::Bool(b);
    }
    if let Some(n) = yaml.as_integer() {
        return serde_json::json!(n);
    }
    if let Some(f) = yaml.as_floating_point() {
        return serde_json::json!(f);
    }
    if let Some(mapping) = yaml.as_mapping() {
        let mut map = serde_json::Map::new();
        for (k, v) in mapping.iter() {
            if let Some(key) = k.as_str() {
                map.insert(key.to_string(), yaml_to_json(v));
            }
        }
        return serde_json::Value::Object(map);
    }
    if let Some(seq) = yaml.as_sequence() {
        return serde_json::Value::Array(seq.iter().map(yaml_to_json).collect());
    }
    serde_json::Value::Null
}

// ---------------------------------------------------------------------------
// Built-in Tera functions
// ---------------------------------------------------------------------------

/// `{{ pagebreak() }}` — format-specific page break.
struct PagebreakFn {
    format: String,
    fragments: Arc<Mutex<Vec<String>>>,
}

impl PagebreakFn {
    fn new(format: &str, fragments: &Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            format: format.to_string(),
            fragments: Arc::clone(fragments),
        }
    }
}

impl tera::Function for PagebreakFn {
    fn call(&self, _args: &HashMap<String, Value>) -> tera::Result<Value> {
        let output = match self.format.as_str() {
            "html" => "<div style=\"page-break-after: always;\"></div>",
            "latex" | "tex" => "\\newpage{}",
            "typst" | "typ" => "#pagebreak()",
            "markdown" | "md" => "\n---\n",
            _ => "\u{0C}",
        };
        Ok(Value::String(wrap_if_needed(output, &self.format, &self.fragments)))
    }

    fn is_safe(&self) -> bool { true }
}

/// `{{ env(name="VAR") }}` — environment variable.
struct EnvFn;

impl tera::Function for EnvFn {
    fn call(&self, args: &HashMap<String, Value>) -> tera::Result<Value> {
        let name = args.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("env() requires a `name` argument"))?;
        Ok(Value::String(std::env::var(name).unwrap_or_default()))
    }

    fn is_safe(&self) -> bool { true }
}

/// `{{ video(url="...", width="...", height="...", title="...") }}` — video embed.
struct VideoFn {
    format: String,
    fragments: Arc<Mutex<Vec<String>>>,
}

impl VideoFn {
    fn new(format: &str, fragments: &Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            format: format.to_string(),
            fragments: Arc::clone(fragments),
        }
    }
}

impl tera::Function for VideoFn {
    fn call(&self, args: &HashMap<String, Value>) -> tera::Result<Value> {
        let url = args.get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("video() requires a `url` argument"))?;
        let width = args.get("width").and_then(|v| v.as_str()).unwrap_or("100%");
        let height = args.get("height").and_then(|v| v.as_str()).unwrap_or("400");
        let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("Video");

        // Detect YouTube/Vimeo and convert to embed URLs
        let embed_url = if url.contains("youtube.com/watch") || url.contains("youtu.be") {
            let id = url
                .split("v=").nth(1).map(|s| s.split('&').next().unwrap_or(s))
                .or_else(|| url.split("youtu.be/").nth(1).map(|s| s.split('?').next().unwrap_or(s)))
                .unwrap_or(url);
            format!("https://www.youtube.com/embed/{}", id)
        } else if url.contains("vimeo.com/") {
            let id = url.rsplit('/').next().unwrap_or(url);
            format!("https://player.vimeo.com/video/{}", id)
        } else {
            url.to_string()
        };

        let output = match self.format.as_str() {
            "html" => {
                if embed_url.contains("youtube.com/embed") || embed_url.contains("player.vimeo.com") {
                    format!(
                        "<iframe src=\"{}\" width=\"{}\" height=\"{}\" title=\"{}\" frameborder=\"0\" allowfullscreen></iframe>",
                        embed_url, width, height, title
                    )
                } else {
                    format!(
                        "<video controls width=\"{}\"><source src=\"{}\">Your browser does not support the video tag.</video>",
                        width, url
                    )
                }
            }
            "latex" | "tex" => format!("\\url{{{}}}", url),
            "typst" | "typ" => format!("#link(\"{}\")[{}]", url, title),
            _ => format!("[{}]({})", title, url),
        };
        Ok(Value::String(wrap_if_needed(&output, &self.format, &self.fragments)))
    }

    fn is_safe(&self) -> bool { true }
}

/// `{{ brand(type="color", name="primary", mode="light") }}` — brand assets.
///
/// Safety: `brand_ptr` is valid for the duration of `process_body()` where the
/// metadata reference is valid. Tera doesn't use threads.
struct BrandFn {
    format: String,
    fragments: Arc<Mutex<Vec<String>>>,
    brand_ptr: *const crate::brand::Brand,
}

impl BrandFn {
    fn new(format: &str, fragments: &Arc<Mutex<Vec<String>>>, brand: Option<&crate::brand::Brand>) -> Self {
        Self {
            format: format.to_string(),
            fragments: Arc::clone(fragments),
            brand_ptr: brand.map_or(std::ptr::null(), |b| b as *const _),
        }
    }
}

unsafe impl Send for BrandFn {}
unsafe impl Sync for BrandFn {}

impl tera::Function for BrandFn {
    fn call(&self, args: &HashMap<String, Value>) -> tera::Result<Value> {
        if self.brand_ptr.is_null() {
            return Ok(Value::String(String::new()));
        }
        // Safety: brand_ptr is valid for the duration of process_body()
        let brand = unsafe { &*self.brand_ptr };

        let typ = args.get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("brand() requires a `type` argument (\"color\" or \"logo\")"))?;
        let name = args.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| tera::Error::msg("brand() requires a `name` argument"))?;
        let mode = args.get("mode").and_then(|v| v.as_str());

        let output = match typ {
            "color" => crate::brand::brand_color(brand, name, mode).unwrap_or_default(),
            "logo" => {
                let m = mode.unwrap_or("both");
                crate::brand::brand_logo_tag(brand, name, m, &self.format).unwrap_or_default()
            }
            _ => {
                cwarn!("brand(): unknown type '{}'", typ);
                String::new()
            }
        };

        // brand color returns a plain string (CSS color), doesn't need wrapping
        if typ == "color" {
            Ok(Value::String(output))
        } else {
            Ok(Value::String(wrap_if_needed(&output, &self.format, &self.fragments)))
        }
    }

    fn is_safe(&self) -> bool { true }
}

/// `{{ kbd(keys=["Ctrl", "C"]) }}` — keyboard shortcut rendering.
struct KbdFn {
    format: String,
    fragments: Arc<Mutex<Vec<String>>>,
}

impl KbdFn {
    fn new(format: &str, fragments: &Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            format: format.to_string(),
            fragments: Arc::clone(fragments),
        }
    }
}

impl tera::Function for KbdFn {
    fn call(&self, args: &HashMap<String, Value>) -> tera::Result<Value> {
        let keys: Vec<String> = args.get("keys")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();

        if keys.is_empty() {
            return Ok(Value::String(String::new()));
        }

        let output = match self.format.as_str() {
            "html" => {
                let parts: Vec<String> = keys.iter()
                    .map(|k| format!("<kbd>{}</kbd>", k))
                    .collect();
                parts.join("+")
            }
            "latex" | "tex" => {
                let parts: Vec<String> = keys.iter()
                    .map(|k| format!("\\texttt{{{}}}", k))
                    .collect();
                parts.join("+")
            }
            "typst" | "typ" => {
                let parts: Vec<String> = keys.iter()
                    .map(|k| format!("#raw(\"{}\")", k))
                    .collect();
                parts.join("+")
            }
            _ => keys.join("+"),
        };
        Ok(Value::String(wrap_if_needed(&output, &self.format, &self.fragments)))
    }

    fn is_safe(&self) -> bool { true }
}

// ---------------------------------------------------------------------------
// Plugin shortcode bridge
// ---------------------------------------------------------------------------

/// Bridge a plugin shortcode to a Tera function.
/// Calls the subprocess via the plugin registry.
struct PluginShortcodeFn {
    name: String,
    plugin_idx: usize,
    format: String,
    meta_json: serde_json::Value,
    fragments: Arc<Mutex<Vec<String>>>,
    // We store a raw pointer to the registry because tera::Function requires
    // 'static + Send + Sync. The registry outlives all Tera calls within
    // process_body(), so this is safe. We only read from it.
    registry_ptr: *const PluginRegistry,
}

// Safety: PluginShortcodeFn is only used within process_body() where the
// registry reference is valid. Tera doesn't use threads.
unsafe impl Send for PluginShortcodeFn {}
unsafe impl Sync for PluginShortcodeFn {}

impl PluginShortcodeFn {
    fn new(
        name: String,
        plugin_idx: usize,
        format: String,
        meta_json: serde_json::Value,
        fragments: Arc<Mutex<Vec<String>>>,
        registry: &PluginRegistry,
    ) -> Self {
        Self {
            name,
            plugin_idx,
            format,
            meta_json,
            fragments,
            registry_ptr: registry as *const PluginRegistry,
        }
    }
}

impl tera::Function for PluginShortcodeFn {
    fn call(&self, args: &HashMap<String, Value>) -> tera::Result<Value> {
        // Safety: registry_ptr is valid for the duration of process_body()
        let registry = unsafe { &*self.registry_ptr };

        let plugin = match registry.plugin_by_index(self.plugin_idx) {
            Some(p) => p,
            None => return Ok(Value::String(String::new())),
        };

        // Convert Tera args to the format expected by call_subprocess_shortcode
        let mut kwargs = HashMap::new();
        for (k, v) in args {
            if let Some(s) = v.as_str() {
                kwargs.insert(k.clone(), s.to_string());
            }
        }
        let positional: Vec<String> = Vec::new();

        if let Some(output) = registry.call_subprocess_shortcode(
            plugin, &self.name, &positional, &kwargs, &self.format, &self.meta_json,
        ) {
            let trimmed = output.trim().to_string();
            Ok(Value::String(wrap_if_needed(&trimmed, &self.format, &self.fragments)))
        } else {
            Ok(Value::String(String::new()))
        }
    }

    fn is_safe(&self) -> bool { true }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protect_restore_fenced_code() {
        let text = "before\n```python\nx = {{ hello }}\n```\nafter";
        let (protected, blocks) = protect_code_blocks(text);
        assert!(!protected.contains("{{ hello }}"));
        assert_eq!(blocks.len(), 1);
        let restored = restore_code_blocks(&protected, &blocks);
        assert_eq!(restored, text);
    }

    #[test]
    fn test_protect_restore_inline_code() {
        let text = "use `{{ var }}` in code";
        let (protected, blocks) = protect_code_blocks(text);
        assert!(!protected.contains("{{ var }}"));
        assert_eq!(blocks.len(), 1);
        let restored = restore_code_blocks(&protected, &blocks);
        assert_eq!(restored, text);
    }

    #[test]
    fn test_no_tera_syntax_passthrough() {
        let text = "plain text with no template syntax";
        let meta = Metadata::default();
        let registry = PluginRegistry::empty();
        let result = process_body(text, "html", &meta, &registry);
        assert_eq!(result.text, text);
        assert!(result.sc_fragments.is_empty());
    }

    #[test]
    fn test_meta_variable_access() {
        let mut meta = Metadata::default();
        meta.title = Some("My Title".to_string());
        let registry = PluginRegistry::empty();
        let result = process_body("Title: {{ meta.title }}", "html", &meta, &registry);
        assert_eq!(result.text, "Title: My Title");
    }

    #[test]
    fn test_env_function() {
        std::env::set_var("CALEPIN_TEST_VAR", "hello_tera");
        let meta = Metadata::default();
        let registry = PluginRegistry::empty();
        let result = process_body("{{ env(name=\"CALEPIN_TEST_VAR\") }}", "html", &meta, &registry);
        assert_eq!(result.text, "hello_tera");
        std::env::remove_var("CALEPIN_TEST_VAR");
    }

    #[test]
    fn test_pagebreak_html() {
        let meta = Metadata::default();
        let registry = PluginRegistry::empty();
        let result = process_body("{{ pagebreak() }}", "html", &meta, &registry);
        assert!(result.text.contains("page-break-after"));
    }

    #[test]
    fn test_pagebreak_latex_marker() {
        let meta = Metadata::default();
        let registry = PluginRegistry::empty();
        let result = process_body("{{ pagebreak() }}", "latex", &meta, &registry);
        // LaTeX output should be wrapped in markers
        assert!(!result.sc_fragments.is_empty());
        assert_eq!(result.sc_fragments[0], "\\newpage{}");
    }

    #[test]
    fn test_code_blocks_preserved() {
        let text = "before {{ meta.title }}\n```\n{{ not_a_var }}\n```\nafter";
        let mut meta = Metadata::default();
        meta.title = Some("T".to_string());
        let registry = PluginRegistry::empty();
        let result = process_body(text, "html", &meta, &registry);
        assert!(result.text.contains("before T"));
        assert!(result.text.contains("{{ not_a_var }}"));
    }
}
