// Shortcode processing: `{{< name args >}}` directives in text.
//
// - expand_includes()      — Pre-parse expansion of `{{< include >}}` directives.
// - process_shortcodes()   — Main processor: built-in → WASM plugin → external.
// - parse_shortcode()      — Tokenize into name, positional args, keyword args.
// - is_inside_code()       — Detect if position is in backtick/fenced code.
//
// Built-in: pagebreak, meta, env, include, var, video.
// External shortcodes use util::run_json_process() and util::resolve_executable().

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::plugins::PluginHandle;
use crate::render::markers;
use crate::types::Metadata;

/// Expand `{{< include file >}}` shortcodes before block parsing.
/// This must run on the raw body text so that included content gets
/// parsed as blocks (code chunks, divs, etc.) rather than inline text.
pub fn expand_includes(text: &str) -> String {
    static ESC_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{\{\{<\s*include\s+(.+?)\s*>\}\}\}").unwrap()
    });
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{\{<\s*include\s+(.+?)\s*>\}\}").unwrap()
    });

    // Protect escaped includes {{{< include >}}} with placeholders
    let mut escaped = Vec::new();
    let text = ESC_RE.replace_all(text, |caps: &regex::Captures| {
        markers::wrap_escaped_shortcode(&mut escaped, caps[0].to_string())
    }).to_string();

    // Expand real includes
    let text = RE.replace_all(&text, |caps: &regex::Captures| {
        let path = caps[1].trim();
        shortcode_include(path)
    }).to_string();

    // Restore escaped includes
    markers::resolve_escaped_shortcodes(&text, &escaped)
}

/// Result of shortcode processing, containing the processed text and
/// fragment vecs needed for later marker resolution.
pub struct ShortcodeResult {
    pub text: String,
    pub sc_fragments: Vec<String>,
    pub escaped_fragments: Vec<String>,
}

/// Process all shortcodes in a text string.
/// Returns the text with shortcodes replaced by their output, plus fragment
/// vecs for deferred marker resolution.
///
/// Escaped shortcodes use triple braces: `{{{< name >}}}` renders as
/// the literal text `{{< name >}}` without processing.
/// Shortcodes inside backtick-delimited code spans are not processed.
pub fn process_shortcodes(text: &str, format: &str, metadata: &Metadata, plugins: &[PluginHandle]) -> ShortcodeResult {
    // First pass: replace escaped shortcodes {{{< ... >}}} with placeholders
    static ESCAPE_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{\{\{<\s*(.+?)\s*>\}\}\}").unwrap()
    });
    let escape_re = &*ESCAPE_RE;
    let mut escaped: Vec<String> = Vec::new();
    let text = escape_re.replace_all(text, |caps: &regex::Captures| {
        let literal = format!("{{{{< {} >}}}}", caps[1].trim());
        markers::wrap_escaped_shortcode(&mut escaped, literal)
    }).to_string();

    // Second pass: process real shortcodes
    static SC_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"\{\{<\s*(.+?)\s*>\}\}").unwrap()
    });
    let re = &*SC_RE;
    let mut sc_fragments: Vec<String> = Vec::new();
    let text = re.replace_all(&text, |caps: &regex::Captures| {
        let full_match = caps.get(0).unwrap();
        let start = full_match.start();

        // Skip if inside a code span or fenced code block
        let before = &text[..start];
        if is_inside_code(before) {
            return caps[0].to_string();
        }

        let inner = caps[1].trim();
        let (name, args, kwargs) = parse_shortcode(inner);

        // Try built-in shortcodes first
        if let Some(output) = builtin_shortcode(&name, &args, &kwargs, format, metadata) {
            // `include` returns raw markdown that still needs parsing — don't wrap in markers
            if name == "include" || name == "meta" || name == "env" || name == "var" {
                return output;
            }
            return wrap_raw_output(&output, format, &mut sc_fragments);
        }

        // Try WASM plugins
        for plugin in plugins {
            let ctx = crate::plugins::ShortcodeContext {
                name: name.clone(),
                args: args.clone(),
                kwargs: kwargs.clone(),
                format: format.to_string(),
            };
            if let Some(output) = plugin.call_shortcode(&ctx) {
                return wrap_raw_output(&output, format, &mut sc_fragments);
            }
        }

        // Try external shortcode
        if let Some(output) = external_shortcode(&name, &args, &kwargs, format, metadata) {
            return wrap_raw_output(&output, format, &mut sc_fragments);
        }

        // Unknown shortcode: warn and keep as-is
        cwarn!("unknown shortcode '{}'", name);
        caps[0].to_string()
    })
    .to_string();

    ShortcodeResult {
        text,
        sc_fragments,
        escaped_fragments: escaped,
    }
}

/// Wrap shortcode output in protection markers for formats that convert markdown.
/// HTML and markdown pass through; LaTeX and Typst need protection so format-specific
/// output (e.g. \newpage, #pagebreak()) survives the markdown-to-format conversion.
fn wrap_raw_output(output: &str, format: &str, fragments: &mut Vec<String>) -> String {
    match format {
        "html" | "markdown" | "md" => output.to_string(),
        _ => markers::wrap_shortcode_raw(fragments, output.to_string()),
    }
}

/// Resolve shortcode raw markers in rendered text, restoring the protected content.
pub fn resolve_shortcode_raw(text: &str, fragments: &[String]) -> String {
    markers::resolve_shortcode_raw(text, fragments)
}

/// Resolve escaped shortcode markers in rendered text, restoring the literal text.
pub fn resolve_escaped_shortcodes(text: &str, escaped: &[String]) -> String {
    markers::resolve_escaped_shortcodes(text, escaped)
}

/// Check if a position in text is inside a code span or fenced code block.
/// Counts unpaired backticks for inline spans, and tracks triple-backtick fences.
fn is_inside_code(before: &str) -> bool {
    let mut in_fence = false;
    let mut backtick_count = 0u32;

    for line in before.split('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence {
            // Count backticks on this line for inline code span detection
            backtick_count += line.chars().filter(|&c| c == '`').count() as u32;
        }
    }

    in_fence || backtick_count % 2 != 0
}

/// Parse a shortcode invocation into name, positional args, and keyword args.
/// Input: "name arg1 arg2 key=value key2=\"quoted value\""
fn parse_shortcode(s: &str) -> (String, Vec<String>, HashMap<String, String>) {
    let tokens = tokenize_shortcode(s);
    let mut args = Vec::new();
    let mut kwargs = HashMap::new();
    let name = tokens.first().cloned().unwrap_or_default();

    for token in tokens.iter().skip(1) {
        if let Some((key, value)) = token.split_once('=') {
            // Don't treat URLs (containing :// or /) as key=value pairs
            if !key.contains("://") && !key.contains('/') {
                let value = value.trim_matches('"').trim_matches('\'');
                kwargs.insert(key.to_string(), value.to_string());
            } else {
                args.push(token.clone());
            }
        } else {
            args.push(token.clone());
        }
    }

    (name, args, kwargs)
}

/// Quote-aware tokenizer for shortcode arguments.
fn tokenize_shortcode(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;

    for ch in s.chars() {
        match in_quote {
            Some(q) if ch == q => {
                current.push(ch);
                in_quote = None;
            }
            Some(_) => {
                current.push(ch);
            }
            None if ch == '"' || ch == '\'' => {
                current.push(ch);
                in_quote = Some(ch);
            }
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            None => {
                current.push(ch);
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

// ---------------------------------------------------------------------------
// Built-in shortcodes
// ---------------------------------------------------------------------------

fn builtin_shortcode(
    name: &str,
    args: &[String],
    kwargs: &HashMap<String, String>,
    format: &str,
    metadata: &Metadata,
) -> Option<String> {
    match name {
        "pagebreak" => Some(shortcode_pagebreak(format)),
        "meta" => {
            let key = args.first().map(|s| s.as_str()).unwrap_or("");
            Some(shortcode_meta(key, metadata))
        }
        "env" => {
            let var = args.first().map(|s| s.as_str()).unwrap_or("");
            Some(shortcode_env(var))
        }
        "include" => {
            let path = args.first().map(|s| s.as_str()).unwrap_or("");
            Some(shortcode_include(path))
        }
        "var" => {
            let key = args.first().map(|s| s.as_str()).unwrap_or("");
            Some(shortcode_var(key))
        }
        "video" => {
            let url = args.first().map(|s| s.as_str()).unwrap_or("");
            Some(shortcode_video(url, kwargs, format))
        }
        _ => None,
    }
}

fn shortcode_pagebreak(format: &str) -> String {
    match format {
        "html" => "<div style=\"page-break-after: always;\"></div>".to_string(),
        "latex" | "tex" => "\\newpage{}".to_string(),
        "typst" | "typ" => "#pagebreak()".to_string(),
        "markdown" | "md" => "\n---\n".to_string(),
        _ => "\u{0C}".to_string(), // form feed
    }
}

fn shortcode_meta(key: &str, metadata: &Metadata) -> String {
    match key {
        "title" => metadata.title.clone().unwrap_or_default(),
        "subtitle" => metadata.subtitle.clone().unwrap_or_default(),
        "author" => metadata
            .author
            .as_ref()
            .map(|a| a.join(", "))
            .unwrap_or_default(),
        "date" => metadata.date.clone().unwrap_or_default(),
        "abstract" => metadata.abstract_text.clone().unwrap_or_default(),
        "keywords" => metadata.keywords.join(", "),
        _ => {
            // Check the extra metadata map
            metadata
                .extra
                .get(key)
                .map(|v| {
                    if let Some(s) = v.as_str() { s.to_string() }
                    else if let Some(b) = v.as_bool() { b.to_string() }
                    else if let Some(n) = v.as_integer() { n.to_string() }
                    else if let Some(f) = v.as_floating_point() { f.to_string() }
                    else { format!("{:?}", v) }
                })
                .unwrap_or_default()
        }
    }
}

fn shortcode_env(var: &str) -> String {
    std::env::var(var).unwrap_or_default()
}

fn shortcode_video(url: &str, kwargs: &HashMap<String, String>, format: &str) -> String {
    if url.is_empty() {
        cwarn!("{{{{< video >}}}} requires a URL");
        return String::new();
    }
    let width = kwargs.get("width").map(|s| s.as_str()).unwrap_or("100%");
    let height = kwargs.get("height").map(|s| s.as_str()).unwrap_or("400");
    let title = kwargs.get("title").map(|s| s.as_str()).unwrap_or("Video");

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

    match format {
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
    }
}

fn shortcode_include(path: &str) -> String {
    if path.is_empty() {
        cwarn!("{{{{< include >}}}} requires a file path");
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
            cwarn!("{{{{< include {} >}}}}: {}", path, e);
            String::new()
        }
    }
}

/// Cached _variables.yml content, loaded once per process.
static VARIABLES: LazyLock<Option<saphyr::YamlOwned>> = LazyLock::new(|| {
    use saphyr::LoadableYamlNode;
    let content = std::fs::read_to_string("_variables.yml")
        .or_else(|_| std::fs::read_to_string("_variables.yaml"))
        .unwrap_or_default();
    if content.is_empty() {
        return None;
    }
    match saphyr::YamlOwned::load_from_str(&content) {
        Ok(docs) => docs.into_iter().next(),
        Err(e) => {
            cwarn!("failed to parse _variables.yml: {}", e);
            None
        }
    }
});

fn shortcode_var(key: &str) -> String {
    if key.is_empty() {
        return String::new();
    }
    let value = match VARIABLES.as_ref() {
        Some(v) => v,
        None => {
            cwarn!("{{{{< var {} >}}}}: no _variables.yml found", key);
            return String::new();
        }
    };
    // Support dot-notation: "a.b.c" traverses nested mappings
    let parts: Vec<&str> = key.split('.').collect();
    let mut current = value;
    for part in &parts {
        match current.as_mapping_get(part) {
            Some(v) => current = v,
            None => return String::new(),
        }
    }
    if let Some(s) = current.as_str() { return s.to_string(); }
    if let Some(b) = current.as_bool() { return b.to_string(); }
    if let Some(n) = current.as_integer() { return n.to_string(); }
    if let Some(f) = current.as_floating_point() { return f.to_string(); }
    match current {
        _ => format!("{:?}", current),
    }
}

// ---------------------------------------------------------------------------
// External shortcodes
// ---------------------------------------------------------------------------

fn external_shortcode(
    name: &str,
    args: &[String],
    kwargs: &HashMap<String, String>,
    format: &str,
    metadata: &Metadata,
) -> Option<String> {
    let path = crate::util::resolve_executable("shortcodes", name, None)?;

    let input = serde_json::json!({
        "name": name,
        "args": args,
        "kwargs": kwargs,
        "format": format,
        "meta": {
            "title": metadata.title,
            "author": metadata.author,
            "date": metadata.date,
        },
    });

    crate::util::run_json_process(&path, &input)
        .map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_shortcode_simple() {
        let (name, args, kwargs) = parse_shortcode("env HOME");
        assert_eq!(name, "env");
        assert_eq!(args, vec!["HOME"]);
        assert!(kwargs.is_empty());
    }

    #[test]
    fn test_parse_shortcode_kwargs() {
        let (name, args, kwargs) = parse_shortcode("git-rev short=true");
        assert_eq!(name, "git-rev");
        assert!(args.is_empty());
        assert_eq!(kwargs.get("short").unwrap(), "true");
    }

    #[test]
    fn test_parse_shortcode_no_args() {
        let (name, args, kwargs) = parse_shortcode("pagebreak");
        assert_eq!(name, "pagebreak");
        assert!(args.is_empty());
        assert!(kwargs.is_empty());
    }

    #[test]
    fn test_shortcode_env() {
        std::env::set_var("SNB_TEST_VAR", "hello");
        let result = shortcode_env("SNB_TEST_VAR");
        assert_eq!(result, "hello");
        std::env::remove_var("SNB_TEST_VAR");
    }

    #[test]
    fn test_shortcode_meta() {
        let mut meta = Metadata::default();
        meta.title = Some("My Title".to_string());
        assert_eq!(shortcode_meta("title", &meta), "My Title");
        assert_eq!(shortcode_meta("missing", &meta), "");
    }

    #[test]
    fn test_process_shortcodes() {
        let meta = Metadata::default();
        std::env::set_var("SNB_TEST_SC", "world");
        let text = "Hello {{< env SNB_TEST_SC >}}!";
        let result = process_shortcodes(text, "html", &meta, &[]);
        assert_eq!(result.text, "Hello world!");
        std::env::remove_var("SNB_TEST_SC");
    }

    #[test]
    fn test_pagebreak_html() {
        let result = shortcode_pagebreak("html");
        assert!(result.contains("page-break-after"));
    }

    #[test]
    fn test_escaped_shortcode() {
        let meta = Metadata::default();
        let text = "Literal: {{{< meta title >}}}";
        let result = process_shortcodes(text, "html", &meta, &[]);
        let resolved = resolve_escaped_shortcodes(&result.text, &result.escaped_fragments);
        assert_eq!(resolved, "Literal: {{< meta title >}}");
    }

    #[test]
    fn test_pagebreak_latex() {
        let result = shortcode_pagebreak("tex");
        assert_eq!(result, "\\newpage{}");
    }
}
