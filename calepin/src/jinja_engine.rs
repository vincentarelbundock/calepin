// MiniJinja-based body processing.
//
// - expand_includes()      — Pre-parse `{% include "file" %}` expansion.
// - process_body()         — Main entry: Jinja-render a text block (code-block-safe).
// - protect_code_blocks()  — Extract fenced code blocks before Jinja evaluation.
// - restore_code_blocks()  — Re-insert protected code after Jinja evaluation.
//
// Built-in Jinja functions:
//   pagebreak(), video(url, ...), brand(type, name, mode?), kbd(keys),
//   lipsum(paragraphs|sentences|words), placeholder(width, height, text, color)
//
// Context variables:
//   meta.title, meta.author, meta.date, ...  — from Metadata
//   var.key.subkey                            — from front matter `variables:` block
//   env.HOME, env.USER, ...                   — system environment variables
//   format                                   — current output format
//
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use regex::Regex;
use serde_json;
use minijinja::{self, Value, Error, ErrorKind};

use crate::registry::PluginRegistry;
use crate::render::markers;
use crate::types::Metadata;




// ---------------------------------------------------------------------------
// Pre-parse include expansion
// ---------------------------------------------------------------------------

/// Expand `{% include "file" %}` directives before block parsing.
/// This must run on the raw body text so that included content gets
/// parsed as blocks (code chunks, divs, etc.) rather than inline text.
/// Paths are resolved relative to `document_dir`.
#[inline(never)]
pub fn expand_includes(text: &str, document_dir: &std::path::Path) -> String {
    // {% include "file" %} or {% include 'file' %}
    static INCLUDE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"\{%[-\s]\s*include\s+["'](.+?)["']\s*[-\s]?%\}"#).unwrap()
    });
    // {% raw %} ... {% endraw %} blocks (protect from include expansion)
    static RAW_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{%-?\s*raw\s*-?%\}[\s\S]*?\{%-?\s*endraw\s*-?%\}").unwrap()
    });

    // Protect fenced code blocks from include expansion
    let (text, code_blocks) = protect_code_blocks(text);

    // Protect {% raw %} blocks from include expansion
    let mut raw_blocks = Vec::new();
    let text = RAW_RE.replace_all(&text, |caps: &regex::Captures| {
        let idx = raw_blocks.len();
        raw_blocks.push(caps[0].to_string());
        format!("\u{FDD2}RAW{}\u{FDD3}", idx)
    }).to_string();

    // Expand includes (resolve relative to document directory)
    let text = INCLUDE_RE.replace_all(&text, |caps: &regex::Captures| {
        let path = caps[1].trim();
        let resolved = document_dir.join(path);
        include_file(&resolved.to_string_lossy())
    }).to_string();

    // Restore {% raw %} blocks
    let mut result = text;
    for (idx, block) in raw_blocks.iter().enumerate() {
        result = result.replace(&format!("\u{FDD2}RAW{}\u{FDD3}", idx), block);
    }

    // Restore fenced code blocks
    restore_code_blocks(&result, &code_blocks)
}

/// Read and include a file, stripping YAML front matter if present.
fn include_file(path: &str) -> String {
    if path.is_empty() {
        // Return error marker that will surface later
        return "\n\n**Error: `{% include %}` requires a file path**\n\n".to_string();
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
            // Surface as visible error in the rendered output
            format!("\n\n**Error: `{{%% include \"{}\" %}}`: {}**\n\n", path, e)
        }
    }
}

// ---------------------------------------------------------------------------
// Jinja body processing
// ---------------------------------------------------------------------------

/// Result of Jinja body processing.
pub struct BodyTeraResult {
    pub text: String,
    pub sc_fragments: Vec<String>,
}

/// Process a text block through MiniJinja, evaluating functions and variable references.
#[inline(never)]
pub fn process_body(
    text: &str,
    format: &str,
    metadata: &Metadata,
    registry: &PluginRegistry,
) -> BodyTeraResult {
    let fragments = Arc::new(Mutex::new(Vec::new()));

    // 1. Protect fenced code blocks and inline code from Jinja
    let (protected, mut code_blocks) = protect_code_blocks(text);
    let protected = protect_inline_code(&protected, &mut code_blocks);

    // 1b. Escape heading attribute syntax {#id .class} which Jinja
    //     interprets as comment openers ({# ... #}).
    static RE_HEADING_ATTR: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{(#[a-zA-Z][a-zA-Z0-9_-]*(?:\s+\.[a-zA-Z][a-zA-Z0-9_-]*)*)\}").unwrap()
    });
    let protected = RE_HEADING_ATTR.replace_all(&protected, "\u{FDD2}$1\u{FDD3}").to_string();

    // Quick exit: if no Jinja syntax found, skip processing
    if !protected.contains("{{") && !protected.contains("{%") {
        return BodyTeraResult {
            text: text.to_string(),
            sc_fragments: Vec::new(),
        };
    }

    // 2. Build MiniJinja environment with custom functions
    let mut env = minijinja::Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Lenient);

    // Register built-in functions
    {
        let fmt = format.to_string();
        let frags = Arc::clone(&fragments);
        env.add_function("pagebreak", move |_args: &[Value]| -> Result<Value, Error> {
            let output = match fmt.as_str() {
                "html" => "<div style=\"page-break-after: always;\"></div>",
                "latex" | "tex" => "\\newpage{}",
                "typst" | "typ" => "#pagebreak()",
                "markdown" | "md" => "\n---\n",
                _ => "\u{0C}",
            };
            Ok(Value::from_safe_string(wrap_if_needed(output, &fmt, &frags)))
        });
    }

    {
        let fmt = format.to_string();
        let frags = Arc::clone(&fragments);
        env.add_function("video", move |kwargs: minijinja::value::Kwargs| -> Result<Value, Error> {
            let url: &str = kwargs.get("url")
                .map_err(|_| Error::new(ErrorKind::MissingArgument, "video() requires a `url` argument"))?;
            let width: &str = kwargs.get("width").unwrap_or("100%");
            let height: &str = kwargs.get("height").unwrap_or("400");
            let title: &str = kwargs.get("title").unwrap_or("Video");
            kwargs.assert_all_used()?;

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

            let output = match fmt.as_str() {
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
            Ok(Value::from_safe_string(wrap_if_needed(&output, &fmt, &frags)))
        });
    }

    {
        let fmt = format.to_string();
        let frags = Arc::clone(&fragments);
        // Safety: brand_addr is valid for the duration of process_body() where the
        // metadata reference is valid. MiniJinja doesn't use threads for evaluation.
        // We cast to usize to satisfy Send+Sync requirements on closures.
        let brand_addr: usize = metadata.brand.as_ref()
            .map_or(0, |b| b as *const _ as usize);
        env.add_function("brand", move |kwargs: minijinja::value::Kwargs| -> Result<Value, Error> {
            if brand_addr == 0 {
                return Ok(Value::from(""));
            }
            // Safety: brand_addr is valid for the duration of process_body()
            let brand_ref = unsafe { &*(brand_addr as *const crate::brand::Brand) };

            let typ: &str = kwargs.get("type")
                .map_err(|_| Error::new(ErrorKind::MissingArgument, "brand() requires a `type` argument (\"color\" or \"logo\")"))?;
            let name: &str = kwargs.get("name")
                .map_err(|_| Error::new(ErrorKind::MissingArgument, "brand() requires a `name` argument"))?;
            let mode: Option<&str> = kwargs.get("mode").ok();
            kwargs.assert_all_used()?;

            let output = match typ {
                "color" => crate::brand::brand_color(brand_ref, name, mode).unwrap_or_default(),
                "logo" => {
                    let m = mode.unwrap_or("both");
                    crate::brand::brand_logo_tag(brand_ref, name, m, &fmt).unwrap_or_default()
                }
                _ => {
                    cwarn!("brand(): unknown type '{}'", typ);
                    String::new()
                }
            };

            // brand color returns a plain string (CSS color), doesn't need wrapping
            if typ == "color" {
                Ok(Value::from(output))
            } else {
                Ok(Value::from_safe_string(wrap_if_needed(&output, &fmt, &frags)))
            }
        });
    }

    {
        let fmt = format.to_string();
        let frags = Arc::clone(&fragments);
        env.add_function("kbd", move |kwargs: minijinja::value::Kwargs| -> Result<Value, Error> {
            let keys_val: Value = kwargs.get("keys").unwrap_or(Value::from(Vec::<String>::new()));
            kwargs.assert_all_used()?;
            let keys: Vec<String> = keys_val.try_iter()
                .map(|iter| iter.filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();

            if keys.is_empty() {
                return Ok(Value::from(""));
            }

            let output = match fmt.as_str() {
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
            Ok(Value::from_safe_string(wrap_if_needed(&output, &fmt, &frags)))
        });
    }

    env.add_function("lipsum", |kwargs: minijinja::value::Kwargs| -> Result<Value, Error> {
        if let Ok(n) = kwargs.get::<u64>("words") {
            kwargs.assert_all_used()?;
            return Ok(Value::from(lipsum_words(n as usize)));
        }
        if let Ok(n) = kwargs.get::<u64>("sentences") {
            kwargs.assert_all_used()?;
            return Ok(Value::from(lipsum_sentences(n as usize)));
        }
        let n: u64 = kwargs.get("paragraphs").unwrap_or(1);
        kwargs.assert_all_used()?;
        Ok(Value::from(lipsum_paragraphs(n as usize)))
    });

    {
        let fmt = format.to_string();
        let frags = Arc::clone(&fragments);
        env.add_function("placeholder", move |kwargs: minijinja::value::Kwargs| -> Result<Value, Error> {
            let width: u32 = kwargs.get("width").unwrap_or(600);
            let height: u32 = kwargs.get("height").unwrap_or(400);
            let color: &str = kwargs.get("color").unwrap_or("#cccccc");
            let text: Option<&str> = kwargs.get("text").ok();
            let text = text.map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}×{}", width, height));
            kwargs.assert_all_used()?;

            let output = match fmt.as_str() {
                "html" => {
                    format!(
                        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\">\
                         <rect width=\"100%\" height=\"100%\" fill=\"{}\"/>\
                         <text x=\"50%\" y=\"50%\" dominant-baseline=\"middle\" text-anchor=\"middle\" \
                         font-family=\"sans-serif\" font-size=\"20\" fill=\"#666\">{}</text>\
                         </svg>",
                        width, height, crate::util::escape_html(color),
                        crate::util::escape_html(&text)
                    )
                }
                "latex" | "tex" => {
                    format!(
                        "\\fbox{{\\parbox[c][{}pt]{{{}pt}}{{\\centering {}}}}}",
                        height, width, text
                    )
                }
                "typst" | "typ" => {
                    format!(
                        "#rect(width: {}pt, height: {}pt, fill: luma(200))[#align(center + horizon)[{}]]",
                        width, height, text
                    )
                }
                _ => format!("[{} ({}×{})]", text, width, height),
            };
            Ok(Value::from_safe_string(wrap_if_needed(&output, &fmt, &frags)))
        });
    }

    // Register plugin shortcodes as Jinja functions
    for (name, plugin_idx) in registry.shortcode_names() {
        let meta_json = serde_json::json!({
            "title": metadata.title,
            "author": metadata.author,
            "date": metadata.date,
        });
        let fmt = format.to_string();
        let frags = Arc::clone(&fragments);
        let sc_name = name.clone();
        // Safety: registry_addr is valid for the duration of process_body()
        // where the registry reference is valid. Cast to usize for Send+Sync.
        let registry_addr = registry as *const PluginRegistry as usize;
        let func_name: &'static str = Box::leak(name.clone().into_boxed_str());
        env.add_function(func_name, move |kwargs: minijinja::value::Kwargs| -> Result<Value, Error> {
            // Safety: registry_addr is valid for the duration of process_body()
            let registry = unsafe { &*(registry_addr as *const PluginRegistry) };

            let plugin = match registry.plugin_by_index(plugin_idx) {
                Some(p) => p,
                None => return Ok(Value::from("")),
            };

            // Convert kwargs to the format expected by call_subprocess_shortcode
            let mut kw = HashMap::new();
            for key in kwargs.args() {
                if let Ok(val) = kwargs.get::<String>(key) {
                    kw.insert(key.to_string(), val);
                }
            }

            let positional: Vec<String> = Vec::new();

            if let Some(output) = registry.call_subprocess_shortcode(
                plugin, &sc_name, &positional, &kw, &fmt, &meta_json,
            ) {
                let trimmed = output.trim().to_string();
                Ok(Value::from_safe_string(wrap_if_needed(&trimmed, &fmt, &frags)))
            } else {
                Ok(Value::from(""))
            }
        });
    }

    // 3. Build context with metadata, variables, and environment
    let context = minijinja::context! {
        format => format,
        meta => build_meta_map(metadata),
        var => build_variables_map(metadata),
        env => std::env::vars().collect::<HashMap<String, String>>(),
    };

    // 4. Render through MiniJinja (on error, fall back to protected text so that
    //    restore_code_blocks can still recover code block placeholders)
    let rendered = match env.render_str(&protected, &context) {
        Ok(r) => r,
        Err(e) => {
            cwarn!("body template error: {}", e);
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
/// placeholders that Jinja won't try to evaluate.
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

    (result, blocks)
}

/// Replace inline code spans (`` `...` ``) with placeholders.
/// Only protects spans that contain Jinja-like syntax.
fn protect_inline_code(text: &str, blocks: &mut Vec<String>) -> String {
    let mut result = String::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '`' {
            let start = i;
            let mut tick_count = 0;
            while i < len && chars[i] == '`' {
                tick_count += 1;
                i += 1;
            }
            let mut found_end = false;
            while i <= len - tick_count {
                if chars[i] == '`' {
                    let mut closing = 0;
                    while i < len && chars[i] == '`' {
                        closing += 1;
                        i += 1;
                    }
                    if closing == tick_count {
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
                } else {
                    i += 1;
                }
            }
            if !found_end {
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
// Metadata → Jinja context helpers
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
    serde_json::Value::Object(map)
}

/// Build the `var` context from extra front matter fields.
fn build_variables_map(metadata: &Metadata) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (key, value) in &metadata.var {
        map.insert(key.clone(), crate::value::to_json(value));
    }
    serde_json::Value::Object(map)
}

// ---------------------------------------------------------------------------
// Lorem ipsum
// ---------------------------------------------------------------------------

const LIPSUM_WORDS: &[&str] = &[
    "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing",
    "elit", "sed", "do", "eiusmod", "tempor", "incididunt", "ut", "labore",
    "et", "dolore", "magna", "aliqua", "enim", "ad", "minim", "veniam",
    "quis", "nostrud", "exercitation", "ullamco", "laboris", "nisi",
    "aliquip", "ex", "ea", "commodo", "consequat", "duis", "aute", "irure",
    "in", "reprehenderit", "voluptate", "velit", "esse", "cillum",
    "fugiat", "nulla", "pariatur", "excepteur", "sint", "occaecat",
    "cupidatat", "non", "proident", "sunt", "culpa", "qui", "officia",
    "deserunt", "mollit", "anim", "id", "est", "laborum", "at", "vero",
    "eos", "accusamus", "iusto", "odio", "dignissimos", "ducimus",
    "blanditiis", "praesentium", "voluptatum", "deleniti", "atque",
    "corrupti", "quos", "dolores", "quas", "molestias", "excepturi",
    "obcaecati", "cupiditate", "provident", "similique", "optio",
    "cumque", "nihil", "impedit", "quo", "minus", "quod", "maxime",
    "placeat", "facere", "possimus", "omnis", "voluptas", "assumenda",
    "repellendus", "temporibus", "autem", "quibusdam", "officiis",
    "debitis", "aut", "rerum", "necessitatibus", "saepe", "eveniet",
    "voluptates", "repudiandae", "recusandae", "itaque", "earum",
    "hic", "tenetur", "sapiente", "delectus", "reiciendis", "voluptatibus",
    "maiores", "alias", "perferendis", "doloribus", "asperiores",
    "repellat",
];

fn lipsum_words(n: usize) -> String {
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        out.push(LIPSUM_WORDS[i % LIPSUM_WORDS.len()]);
    }
    let mut s = out.join(" ");
    if let Some(first) = s.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    s
}

fn lipsum_sentence(word_count: usize, offset: usize) -> String {
    let mut out = Vec::with_capacity(word_count);
    for i in 0..word_count {
        out.push(LIPSUM_WORDS[(i + offset) % LIPSUM_WORDS.len()]);
    }
    let mut s = out.join(" ");
    if let Some(first) = s.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    s.push('.');
    s
}

fn lipsum_sentences(n: usize) -> String {
    let mut sentences = Vec::with_capacity(n);
    for i in 0..n {
        let len = 8 + (i * 3) % 8;
        sentences.push(lipsum_sentence(len, i * 7));
    }
    sentences.join(" ")
}

fn lipsum_paragraphs(n: usize) -> String {
    let mut paragraphs = Vec::with_capacity(n);
    for i in 0..n {
        let count = 3 + (i * 2) % 3;
        let mut sentences = Vec::with_capacity(count);
        for j in 0..count {
            let len = 8 + ((i * 5 + j * 3) % 8);
            sentences.push(lipsum_sentence(len, i * 17 + j * 11));
        }
        paragraphs.push(sentences.join(" "));
    }
    paragraphs.join("\n\n")
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
    fn test_inline_code_protected() {
        let mut meta = Metadata::default();
        meta.title = Some("T".to_string());
        let registry = PluginRegistry::empty();
        let result = process_body("version: `{{ meta.title }}`", "html", &meta, &registry);
        assert!(result.text.contains("`{{ meta.title }}`"));
    }

    #[test]
    fn test_no_jinja_syntax_passthrough() {
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
    fn test_env_context_variable() {
        std::env::set_var("CALEPIN_TEST_VAR", "hello_jinja");
        let meta = Metadata::default();
        let registry = PluginRegistry::empty();
        let result = process_body("{{ env.CALEPIN_TEST_VAR }}", "html", &meta, &registry);
        assert_eq!(result.text, "hello_jinja");
        std::env::remove_var("CALEPIN_TEST_VAR");
    }

    #[test]
    fn test_lipsum_default() {
        let meta = Metadata::default();
        let registry = PluginRegistry::empty();
        let result = process_body("{{ lipsum() }}", "html", &meta, &registry);
        assert!(result.text.contains("Lorem"));
        assert!(result.text.contains('.'));
    }

    #[test]
    fn test_lipsum_words() {
        let meta = Metadata::default();
        let registry = PluginRegistry::empty();
        let result = process_body("{{ lipsum(words=5) }}", "html", &meta, &registry);
        assert_eq!(result.text.split_whitespace().count(), 5);
    }

    #[test]
    fn test_lipsum_paragraphs() {
        let meta = Metadata::default();
        let registry = PluginRegistry::empty();
        let result = process_body("{{ lipsum(paragraphs=3) }}", "html", &meta, &registry);
        // 3 paragraphs separated by double newlines
        let paras: Vec<&str> = result.text.split("\n\n").collect();
        assert_eq!(paras.len(), 3);
    }

    #[test]
    fn test_placeholder_html() {
        let meta = Metadata::default();
        let registry = PluginRegistry::empty();
        let result = process_body("{{ placeholder(width=200, height=100) }}", "html", &meta, &registry);
        assert!(result.text.contains("<svg"));
        assert!(result.text.contains("200"));
        assert!(result.text.contains("100"));
    }

    #[test]
    fn test_placeholder_latex() {
        let meta = Metadata::default();
        let registry = PluginRegistry::empty();
        let result = process_body("{{ placeholder(width=200, height=100) }}", "latex", &meta, &registry);
        assert!(!result.sc_fragments.is_empty());
        assert!(result.sc_fragments[0].contains("fbox"));
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
