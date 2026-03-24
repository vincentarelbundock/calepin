// MiniJinja-based body processing.
//
// - expand_includes()      -- Pre-parse `{% include "file" %}` expansion.
// - process_body()         -- Main entry: Jinja-render a text block (code-block-safe).
// - protect_code_blocks()  -- Extract fenced code blocks before Jinja evaluation.
// - restore_code_blocks()  -- Re-insert protected code after Jinja evaluation.
//
// Built-in Jinja functions:
//   pagebreak(), video(url, ...), kbd(keys),
//   lipsum(paragraphs|sentences|words), placeholder(width, height, text, color)
//
// Context variables:
//   meta.title, meta.author, meta.date, ...  -- from Metadata
//   var.key.subkey                            -- from front matter `variables:` block
//   env.HOME, env.USER, ...                   -- system environment variables
//   base, target                              -- current output format
//   snip.snippet_name                          -- lazy snippet inclusion from snippets/

mod includes;
pub mod lipsum;
mod protection;

pub use includes::expand_includes;
pub(crate) use lipsum::{lipsum_words, lipsum_sentence, lipsum_paragraphs};

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use regex::Regex;
use serde_json;
use minijinja::{self, Value, Error, ErrorKind};

use crate::registry::PluginRegistry;
use crate::render::markers;
use crate::types::Metadata;

use protection::{protect_code_blocks, protect_inline_code, restore_code_blocks};

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
            let vdefs = crate::project::get_defaults().video;
            let default_width = vdefs.as_ref().and_then(|v| v.width.clone()).unwrap_or_else(|| "100%".to_string());
            let default_height = vdefs.as_ref().and_then(|v| v.height.clone()).unwrap_or_else(|| "400".to_string());
            let default_title = vdefs.as_ref().and_then(|v| v.title.clone()).unwrap_or_else(|| "Video".to_string());
            let width: &str = kwargs.get("width").unwrap_or(&default_width);
            let height: &str = kwargs.get("height").unwrap_or(&default_height);
            let title: &str = kwargs.get("title").unwrap_or(&default_title);
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
            return Ok(Value::from(lipsum::lipsum_words(n as usize)));
        }
        if let Ok(n) = kwargs.get::<u64>("sentences") {
            kwargs.assert_all_used()?;
            return Ok(Value::from(lipsum::lipsum_sentences(n as usize)));
        }
        let default_paragraphs = crate::project::get_defaults().lipsum.as_ref().and_then(|l| l.paragraphs).unwrap_or(1);
        let n: u64 = kwargs.get("paragraphs").unwrap_or(default_paragraphs);
        kwargs.assert_all_used()?;
        Ok(Value::from(lipsum::lipsum_paragraphs(n as usize)))
    });

    {
        let fmt = format.to_string();
        let frags = Arc::clone(&fragments);
        env.add_function("placeholder", move |kwargs: minijinja::value::Kwargs| -> Result<Value, Error> {
            let pdefs = crate::project::get_defaults().placeholder;
            let default_pw = pdefs.as_ref().and_then(|p| p.width).unwrap_or(600);
            let default_ph = pdefs.as_ref().and_then(|p| p.height).unwrap_or(400);
            let default_color = pdefs.as_ref().and_then(|p| p.color.clone()).unwrap_or_else(|| "#cccccc".to_string());
            let width: u32 = kwargs.get("width").unwrap_or(default_pw);
            let height: u32 = kwargs.get("height").unwrap_or(default_ph);
            let color: &str = kwargs.get("color").unwrap_or(&default_color);
            let text: Option<&str> = kwargs.get("text").ok();
            let text = text.map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}\u{00d7}{}", width, height));
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
                _ => format!("[{} ({}x{})]", text, width, height),
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
    let meta_val = build_meta_map(metadata);
    let var_val = build_variables_map(metadata);
    let env_val: HashMap<String, String> = std::env::vars().collect();

    let context = minijinja::context! {
        base => format,     // rendering engine (html, latex, typst, markdown)
        target => format,   // target name (defaults to base when no target specified)
        meta => meta_val,
        var => var_val,
        env => env_val,
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
// Metadata -> Jinja context helpers
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
