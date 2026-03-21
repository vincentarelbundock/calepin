// Evaluate orchestrator and engine dispatch.
//
// - evaluate()          — Walk parsed Blocks, execute code chunks, process shortcodes and
//                         inline expressions, filter conditional content, and produce Elements.
// - execute_chunk()     — Dispatch a code chunk to the R or Python engine and capture output.
// - evaluate_inline()   — Dispatch an inline expression (`{r}`/`{python}`) to its engine.
// - make_sentinel()     — Generate a unique sentinel string for the capture protocol.
// - process_results()   — Parse sentinel-delimited engine output into ChunkResults.
// - format_matches()    — Check if a format name matches the current output format (with aliases).
// - content_is_visible() — Evaluate .content-visible/.content-hidden conditions.

pub mod block;
pub mod cache;
pub mod inline;
pub mod python;
pub mod r;
pub mod sh;
pub mod subprocess;
pub mod util;

use anyhow::Result;
use std::path::Path;

use std::collections::HashMap;

use crate::types::{Block, ChunkOptions, ChunkResult, Element, Metadata};

/// Holds mutable references to the active engine sessions.
/// Threaded through the evaluate pipeline so block/inline code can dispatch.
pub struct EngineContext<'a> {
    pub r: Option<&'a mut r::RSession>,
    pub python: Option<&'a mut python::PythonSession>,
    pub sh: Option<&'a mut sh::ShSession>,
}

/// Result of evaluating all blocks.
pub struct EvalResult {
    pub elements: Vec<Element>,
    pub sc_fragments: Vec<String>,
}

/// Evaluate all blocks and produce a flat list of Elements.
/// Executes code chunks, processes shortcodes, filters conditional content.
pub fn evaluate(
    blocks: &[Block],
    fig_dir: &Path,
    fig_ext: &str,
    output_ext: &str,
    metadata: &Metadata,
    registry: &crate::registry::PluginRegistry,
    ctx: &mut EngineContext,
    cache: &mut cache::CacheState,
) -> Result<EvalResult> {
    let mut elements: Vec<Element> = Vec::new();
    let mut sc_fragments: Vec<String> = Vec::new();

    for block in blocks {
        match block {
            Block::Text(text) => {
                let tera_result = crate::tera_engine::process_body(
                    &text.content, output_ext, metadata, registry,
                );
                // Only hash inline code expressions into the upstream digest,
                // not the full text. This way prose edits don't invalidate chunk caches.
                for (_start, _end, ic) in crate::parse::blocks::collect_inline_code(&tera_result.text) {
                    cache.advance_digest_inline(&format!("{}:{}", ic.engine, ic.expr));
                }
                let processed = inline::evaluate_inline(&tera_result.text, ctx)?;
                sc_fragments.extend(tera_result.sc_fragments);
                elements.push(Element::Text { content: processed });
            }
            Block::Code(chunk) => {
                let mut chunk_elements = block::evaluate_block(chunk, fig_dir, fig_ext, ctx, cache)?;
                elements.append(&mut chunk_elements);
            }
            Block::CodeBlock(cb) => {
                elements.push(Element::CodeSource {
                    code: cb.code.clone(),
                    lang: cb.lang.clone(),
                    label: String::new(),
                    filename: cb.filename.clone(),
                });
            }
            Block::Div(div) => {
                if !div_is_visible(&div.classes, &div.attrs, output_ext, &metadata.extra) {
                    continue;
                }
                if div.classes.iter().any(|c| c == "hidden") {
                    let _children = evaluate(&div.children, fig_dir, fig_ext, output_ext, metadata, registry, ctx, cache)?;
                    continue;
                }
                if div.classes.iter().any(|c| c == "verbatim") {
                    let raw = div.children.iter()
                        .filter_map(|b| match b {
                            Block::Text(t) => Some(t.content.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    elements.push(Element::CodeSource {
                        code: raw,
                        lang: String::new(),
                        label: String::new(),
                        filename: String::new(),
                    });
                    continue;
                }
                let child_result = evaluate(&div.children, fig_dir, fig_ext, output_ext, metadata, registry, ctx, cache)?;
                sc_fragments.extend(child_result.sc_fragments);
                elements.push(Element::Div {
                    classes: div.classes.clone(),
                    id: div.id.clone(),
                    attrs: div.attrs.clone(),
                    children: child_result.elements,
                });
            }
            Block::Raw(raw) => {
                if format_matches(&raw.format, output_ext) {
                    elements.push(Element::CodeAsis {
                        text: raw.content.clone(),
                    });
                }
            }
        }
    }

    Ok(EvalResult { elements, sc_fragments })
}

// ---------------------------------------------------------------------------
// Engine dispatch: shared machinery called by block.rs and inline.rs
// ---------------------------------------------------------------------------

/// Execute a code chunk and capture all output.
/// Dispatches to the appropriate engine based on chunk options.
pub fn execute_chunk(
    source: &[String],
    options: &ChunkOptions,
    label: &str,
    fig_dir: &Path,
    fig_ext: &str,
    ctx: &mut EngineContext,
) -> Result<Vec<ChunkResult>> {
    let code = source.join("\n");
    let mut results = Vec::new();

    if !options.eval() {
        results.push(ChunkResult::Source(source.to_vec()));
        return Ok(results);
    }

    // Set up figure paths (skip for tbl- chunks which don't produce plots)
    let is_table_chunk = label.starts_with("tbl-");
    std::fs::create_dir_all(fig_dir).ok();
    let fig_width = options.fig_width();
    let fig_height = options.fig_height();
    let fig_full_path = fig_dir.join(format!("{}-1.{}", label, fig_ext));
    let fig_full_str = if is_table_chunk {
        String::new()
    } else {
        fig_full_path.to_string_lossy().replace('\\', "/")
    };

    results.push(ChunkResult::Source(source.to_vec()));

    // Dispatch to engine-specific capture
    let captured = match options.engine().as_str() {
        "sh" => {
            let session = ctx.sh.as_mut()
                .ok_or_else(|| anyhow::anyhow!("sh is not available. Is /bin/sh on PATH?"))?;
            session.capture(&code)?
        }
        "python" => {
            let session = ctx.python.as_mut()
                .ok_or_else(|| anyhow::anyhow!("Python is not available. Is python3 installed and on PATH?"))?;
            let dpi: f64 = options
                .get_opt_string("dpi")
                .and_then(|s| s.parse().ok())
                .unwrap_or(150.0);
            session.capture(&code, &fig_full_str, fig_width, fig_height, dpi)?
        }
        _ => {
            let session = ctx.r.as_mut()
                .ok_or_else(|| anyhow::anyhow!("R is not available. Is Rscript installed and on PATH?"))?;
            let dev = if options.get_opt_string("dev").is_some() {
                options.dev()
            } else {
                match fig_ext {
                    "pdf" => "cairo_pdf".to_string(),
                    "svg" => "svg".to_string(),
                    _ => "png".to_string(),
                }
            };
            session.capture(&code, &fig_full_str, &dev, fig_width, fig_height)?
        }
    };

    process_results(&captured, &fig_full_path, options, &mut results)?;
    Ok(results)
}

/// Evaluate an inline code expression for the given engine.
pub fn evaluate_inline(engine: &str, expr: &str, ctx: &mut EngineContext) -> Result<String> {
    match engine {
        "sh" => {
            let session = ctx.sh.as_mut()
                .ok_or_else(|| anyhow::anyhow!("sh is not available"))?;
            session.evaluate_inline(expr)
        }
        "python" => {
            let session = ctx.python.as_mut()
                .ok_or_else(|| anyhow::anyhow!("Python is not available"))?;
            session.evaluate_inline(expr)
        }
        "r" => {
            let session = ctx.r.as_mut()
                .ok_or_else(|| anyhow::anyhow!("R is not available"))?;
            session.evaluate_inline(expr)
        }
        _ => Err(anyhow::anyhow!("Unknown inline engine: {}", engine)),
    }
}

/// Generate a unique sentinel that cannot appear in user output.
pub fn make_sentinel() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    format!("__CALEPIN_{:x}_{:x}__", pid, nanos)
}

/// Parse sentinel-delimited capture output into ChunkResults.
fn process_results(
    raw: &str,
    fig_path: &Path,
    options: &ChunkOptions,
    results: &mut Vec<ChunkResult>,
) -> Result<()> {
    let (sentinel, rest) = raw.split_once('\n').unwrap_or(("", raw));
    let sep = format!("\n{}_SEP\n", sentinel);

    let output_prefix = format!("{}_OUTPUT:", sentinel);
    let asis_prefix = format!("{}_ASIS:", sentinel);
    let error_prefix = format!("{}_ERROR:", sentinel);
    let warning_prefix = format!("{}_WARNING:", sentinel);
    let message_prefix = format!("{}_MESSAGE:", sentinel);
    let plot_prefix = format!("{}_PLOT:", sentinel);

    for part in rest.split(&sep) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(text) = part.strip_prefix(&error_prefix) {
            if !text.is_empty() {
                results.push(ChunkResult::Error(text.to_string()));
            }
        } else if let Some(text) = part.strip_prefix(&asis_prefix) {
            if !text.is_empty() {
                results.push(ChunkResult::Asis(text.to_string()));
            }
        } else if let Some(text) = part.strip_prefix(&output_prefix) {
            if let Some(err_msg) = text.strip_prefix(&error_prefix) {
                results.push(ChunkResult::Error(err_msg.to_string()));
            } else if !text.is_empty() {
                results.push(ChunkResult::Output(text.to_string()));
            }
        } else if let Some(text) = part.strip_prefix(&warning_prefix) {
            if options.warning() && !text.is_empty() {
                results.push(ChunkResult::Warning(text.to_string()));
            }
        } else if let Some(text) = part.strip_prefix(&message_prefix) {
            if options.message() && !text.is_empty() {
                results.push(ChunkResult::Message(text.to_string()));
            }
        } else if part.starts_with(&plot_prefix) {
            if fig_path.exists() {
                results.push(ChunkResult::Plot(fig_path.to_path_buf()));
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Visibility logic (used by div rendering and span rendering)
// ---------------------------------------------------------------------------

pub fn format_matches(format_name: &str, output_format: &str) -> bool {
    let normalized = match format_name {
        "tex" | "pdf" => "latex",
        "typ" => "typst",
        "md" => "markdown",
        other => other,
    };
    normalized == output_format
}

fn div_is_visible(
    classes: &[String],
    attrs: &HashMap<String, String>,
    output_format: &str,
    meta_extra: &HashMap<String, saphyr::YamlOwned>,
) -> bool {
    content_is_visible(classes, attrs, output_format, Some(meta_extra))
}

pub fn content_is_visible(
    classes: &[String],
    attrs: &HashMap<String, String>,
    output_format: &str,
    meta_extra: Option<&HashMap<String, saphyr::YamlOwned>>,
) -> bool {
    let is_content_visible = classes.iter().any(|c| c == "content-visible");
    let is_content_hidden = classes.iter().any(|c| c == "content-hidden");

    if !is_content_visible && !is_content_hidden {
        return true;
    }

    let when_format = attrs.get("when-format").map(|s| s.as_str());
    let unless_format = attrs.get("unless-format").map(|s| s.as_str());
    let when_meta = attrs.get("when-meta").map(|s| s.as_str());
    let unless_meta = attrs.get("unless-meta").map(|s| s.as_str());

    if is_content_visible {
        let when_ok = when_format.map_or(true, |f| format_matches(f, output_format));
        let unless_ok = unless_format.map_or(true, |f| !format_matches(f, output_format));
        let when_meta_ok = when_meta.map_or(true, |key| meta_is_truthy(meta_extra, key));
        let unless_meta_ok = unless_meta.map_or(true, |key| !meta_is_truthy(meta_extra, key));
        when_ok && unless_ok && when_meta_ok && unless_meta_ok
    } else {
        let when_ok = when_format.map_or(true, |f| format_matches(f, output_format));
        let unless_ok = unless_format.map_or(true, |f| !format_matches(f, output_format));
        let when_meta_ok = when_meta.map_or(true, |key| meta_is_truthy(meta_extra, key));
        let unless_meta_ok = unless_meta.map_or(true, |key| !meta_is_truthy(meta_extra, key));
        !(when_ok && unless_ok && when_meta_ok && unless_meta_ok)
    }
}

fn meta_is_truthy(extra: Option<&HashMap<String, saphyr::YamlOwned>>, key: &str) -> bool {
    use saphyr::YamlOwned;
    let extra = match extra {
        Some(e) => e,
        None => return false,
    };
    let parts: Vec<&str> = key.split('.').collect();
    let mut current: &YamlOwned = match extra.get(parts[0]) {
        Some(v) => v,
        None => return false,
    };
    for part in &parts[1..] {
        match current.as_mapping_get(part) {
            Some(v) => current = v,
            None => return false,
        }
    }
    if let Some(b) = current.as_bool() { return b; }
    if let Some(s) = current.as_str() { return !s.is_empty() && s != "false" && s != "no"; }
    if let Some(n) = current.as_floating_point() { return n != 0.0; }
    if let Some(n) = current.as_integer() { return n != 0; }
    if current.is_null() { return false; }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_matches_canonical() {
        assert!(format_matches("html", "html"));
        assert!(format_matches("latex", "latex"));
        assert!(format_matches("typst", "typst"));
        assert!(format_matches("markdown", "markdown"));
        assert!(!format_matches("html", "latex"));
        assert!(!format_matches("latex", "html"));
    }

    #[test]
    fn test_format_matches_aliases() {
        assert!(format_matches("tex", "latex"));
        assert!(format_matches("pdf", "latex"));
        assert!(format_matches("typ", "typst"));
        assert!(format_matches("md", "markdown"));
    }

    #[test]
    fn test_content_visible_when_format() {
        let classes = vec!["content-visible".to_string()];
        let mut attrs = HashMap::new();
        attrs.insert("when-format".to_string(), "html".to_string());
        assert!(content_is_visible(&classes, &attrs, "html", None));
        assert!(!content_is_visible(&classes, &attrs, "latex", None));
    }

    #[test]
    fn test_content_visible_unless_format() {
        let classes = vec!["content-visible".to_string()];
        let mut attrs = HashMap::new();
        attrs.insert("unless-format".to_string(), "pdf".to_string());
        assert!(content_is_visible(&classes, &attrs, "html", None));
        assert!(!content_is_visible(&classes, &attrs, "latex", None));
    }

    #[test]
    fn test_content_hidden_when_format() {
        let classes = vec!["content-hidden".to_string()];
        let mut attrs = HashMap::new();
        attrs.insert("when-format".to_string(), "html".to_string());
        assert!(!content_is_visible(&classes, &attrs, "html", None));
        assert!(content_is_visible(&classes, &attrs, "latex", None));
    }

    #[test]
    fn test_content_hidden_unless_format() {
        let classes = vec!["content-hidden".to_string()];
        let mut attrs = HashMap::new();
        attrs.insert("unless-format".to_string(), "pdf".to_string());
        assert!(!content_is_visible(&classes, &attrs, "html", None));
        assert!(content_is_visible(&classes, &attrs, "latex", None));
    }

    #[test]
    fn test_content_visible_combined() {
        let classes = vec!["content-visible".to_string()];
        let mut attrs = HashMap::new();
        attrs.insert("when-format".to_string(), "html".to_string());
        attrs.insert("unless-format".to_string(), "html".to_string());
        assert!(!content_is_visible(&classes, &attrs, "html", None));
    }

    #[test]
    fn test_no_conditional_class() {
        let classes = vec!["theorem".to_string()];
        let attrs = HashMap::new();
        assert!(content_is_visible(&classes, &attrs, "html", None));
        assert!(content_is_visible(&classes, &attrs, "latex", None));
    }

    #[test]
    fn test_when_meta_truthy() {
        let classes = vec!["content-visible".to_string()];
        let mut attrs = HashMap::new();
        attrs.insert("when-meta".to_string(), "draft".to_string());
        let mut extra = HashMap::new();
        extra.insert("draft".to_string(), saphyr::YamlOwned::Value(saphyr::ScalarOwned::Boolean(true)));
        assert!(content_is_visible(&classes, &attrs, "html", Some(&extra)));
        extra.insert("draft".to_string(), saphyr::YamlOwned::Value(saphyr::ScalarOwned::Boolean(false)));
        assert!(!content_is_visible(&classes, &attrs, "html", Some(&extra)));
    }

    #[test]
    fn test_unless_meta() {
        let classes = vec!["content-hidden".to_string()];
        let mut attrs = HashMap::new();
        attrs.insert("unless-meta".to_string(), "published".to_string());
        let mut extra = HashMap::new();
        extra.insert("published".to_string(), saphyr::YamlOwned::Value(saphyr::ScalarOwned::Boolean(true)));
        assert!(content_is_visible(&classes, &attrs, "html", Some(&extra)));
        extra.insert("published".to_string(), saphyr::YamlOwned::Value(saphyr::ScalarOwned::Boolean(false)));
        assert!(!content_is_visible(&classes, &attrs, "html", Some(&extra)));
    }

    #[test]
    fn test_when_meta_dot_notation() {
        let classes = vec!["content-visible".to_string()];
        let mut attrs = HashMap::new();
        attrs.insert("when-meta".to_string(), "options.show-code".to_string());
        let mut inner = saphyr::MappingOwned::new();
        inner.insert(
            saphyr::YamlOwned::Value(saphyr::ScalarOwned::String("show-code".to_string())),
            saphyr::YamlOwned::Value(saphyr::ScalarOwned::Boolean(true)),
        );
        let mut extra = HashMap::new();
        extra.insert("options".to_string(), saphyr::YamlOwned::Mapping(inner));
        assert!(content_is_visible(&classes, &attrs, "html", Some(&extra)));
    }
}
