// Evaluate orchestrator and engine dispatch.
//
// - evaluate()          — Walk parsed Blocks, execute code chunks, process Jinja functions and
//                         inline expressions, filter conditional content, and produce Elements.
// - execute_chunk()     — Dispatch a code chunk to the R or Python engine and capture output.
// - evaluate_inline()   — Dispatch an inline expression (`{r}`/`{python}`) to its engine.
// - make_sentinel()     — Generate a unique sentinel string for the capture protocol.
// - process_results()   — Parse sentinel-delimited engine output into ChunkResults.
// - format_matches()    — Check if a format name matches the current output format (with aliases).
// - content_is_visible() — Evaluate .content-visible/.content-hidden conditions.

pub mod block;
pub mod cache;
pub mod diagram;
pub mod inline;
pub mod python;
pub mod r;
pub mod sh;
pub mod subprocess;
pub mod util;

use anyhow::Result;
use std::path::Path;

use std::collections::HashMap;

use crate::types::{Block, ChunkOptions, ChunkResult, CodeChunk, Element, OptionValue};
use crate::config::Metadata;

/// Holds mutable references to the active engine sessions.
/// Threaded through the evaluate pipeline so block/inline code can dispatch.
pub struct EngineContext<'a> {
    pub r: Option<&'a mut r::RSession>,
    pub python: Option<&'a mut python::PythonSession>,
    pub sh: Option<&'a mut sh::ShSession>,
}

/// Owns engine sessions. Initialized lazily based on what the document needs.
pub struct EnginePool {
    pub r: Option<r::RSession>,
    pub python: Option<python::PythonSession>,
    pub sh: Option<sh::ShSession>,
}

impl EnginePool {
    /// Initialize engine sessions needed by the document.
    /// Only starts interpreters for languages actually used in code chunks.
    pub fn init(
        blocks: &[Block],
        body: &str,
        metadata: &Metadata,
        engine: &str,
        working_dir: Option<&Path>,
    ) -> anyhow::Result<Self> {
        let timeout = subprocess::resolve_timeout(metadata);
        let r = if util::needs_engine(blocks, body, metadata, "r") {
            Some(r::RSession::init(engine, working_dir, timeout)?)
        } else {
            None
        };
        let python = if util::needs_engine(blocks, body, metadata, "python") {
            Some(python::PythonSession::init(working_dir, timeout)?)
        } else {
            None
        };
        let sh = if util::needs_engine(blocks, body, metadata, "sh") {
            Some(sh::ShSession::init(working_dir, timeout)?)
        } else {
            None
        };
        Ok(Self { r, python, sh })
    }

    /// Borrow as an EngineContext for threading through evaluate.
    pub fn context(&mut self) -> EngineContext<'_> {
        EngineContext {
            r: self.r.as_mut(),
            python: self.python.as_mut(),
            sh: self.sh.as_mut(),
        }
    }
}

/// Result of evaluating all blocks.
pub struct EvalResult {
    pub elements: Vec<Element>,
    pub sc_fragments: Vec<String>,
    /// Preamble content collected from code chunks (e.g. \usepackage lines).
    pub preamble: Vec<String>,
}

/// Evaluate a document: set up cache and figure paths, then evaluate all blocks.
pub fn evaluate_document(
    input: &Path,
    blocks: &[Block],
    _body: &str,
    output_ext: &str,
    metadata: &Metadata,
    registry: &crate::registry::ModuleRegistry,
    ctx: &mut EngineContext,
    path_ctx: &crate::paths::PathContext,
    default_fig_ext: &str,
) -> Result<EvalResult> {
    let rel_stem = path_ctx.relative_stem(input);
    let fig_dir = path_ctx.figures_dir(&rel_stem);
    let cache_dir = path_ctx.cache_root(&rel_stem);
    let cache_enabled = metadata.var.get("execute")
        .and_then(|v| v.get("cache"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let mut cache = cache::CacheState::new(input, &cache_dir, cache_enabled);

    let result = evaluate(blocks, &fig_dir, default_fig_ext, output_ext, metadata, registry, ctx, &mut cache)?;

    // Clean up empty fig_dir
    if fig_dir.is_dir() && std::fs::read_dir(&fig_dir).map_or(false, |mut d| d.next().is_none()) {
        std::fs::remove_dir(&fig_dir).ok();
    }

    Ok(result)
}

/// Evaluate all blocks and produce a flat list of Elements.
/// Executes code chunks, processes Jinja functions, filters conditional content.
#[inline(never)]
pub fn evaluate(
    blocks: &[Block],
    fig_dir: &Path,
    fig_ext: &str,
    output_ext: &str,
    metadata: &Metadata,
    registry: &crate::registry::ModuleRegistry,
    ctx: &mut EngineContext,
    cache: &mut cache::CacheState,
) -> Result<EvalResult> {
    let mut elements: Vec<Element> = Vec::new();
    let mut sc_fragments: Vec<String> = Vec::new();
    let mut preamble: Vec<String> = Vec::new();

    for block in blocks {
        match block {
            Block::Text(text) => {
                let jinja_result = crate::jinja::process_body(
                    &text.content, output_ext, metadata, registry,
                );
                // Only hash inline code expressions into the upstream digest,
                // not the full text. This way prose edits don't invalidate chunk caches.
                for (_start, _end, ic) in crate::parse::blocks::collect_inline_code(&jinja_result.text) {
                    cache.advance_digest_inline(&format!("{}:{}", ic.engine, ic.expr));
                }
                let processed = inline::evaluate_inline(&jinja_result.text, ctx)?;
                sc_fragments.extend(jinja_result.sc_fragments);
                elements.push(Element::Text { content: processed });
            }
            Block::Code(chunk) => {
                // Merge document-level defaults from front matter var into chunk options.
                // Resolution order: chunk #| options > front matter var > _calepin.toml defaults.
                // Only merge keys that look like chunk options (contain a dot or match known names).
                static CHUNK_OPT_PREFIXES: &[&str] = &[
                    "echo", "eval", "include", "warning", "message", "results", "cache",
                    "fig_", "out_", "comment", "dev", "dpi", "label",
                ];
                let mut merged_chunk = chunk.clone();
                merged_chunk.options.metadata = metadata.clone();
                for (key, val) in &metadata.var {
                    let opt_key = crate::util::normalize_key(key);
                    let is_chunk_opt = CHUNK_OPT_PREFIXES.iter().any(|p| opt_key.starts_with(p));
                    if is_chunk_opt && !merged_chunk.options.inner.contains_key(&opt_key) {
                        let inserted = if let Some(s) = val.as_str() {
                            merged_chunk.options.inner.insert(opt_key.clone(), OptionValue::String(s.to_string()));
                            true
                        } else if let Some(b) = val.as_bool() {
                            merged_chunk.options.inner.insert(opt_key.clone(), OptionValue::Bool(b));
                            true
                        } else if let Some(n) = val.as_floating_point() {
                            merged_chunk.options.inner.insert(opt_key.clone(), OptionValue::String(n.to_string()));
                            true
                        } else if let Some(n) = val.as_integer() {
                            merged_chunk.options.inner.insert(opt_key.clone(), OptionValue::String(n.to_string()));
                            true
                        } else {
                            false
                        };
                        if inserted {
                            merged_chunk.options.defaults_keys.insert(opt_key);
                        }
                    }
                }

                // If #| jinja: true, process chunk source through Jinja before execution
                let jinja_chunk;
                let chunk_ref = if merged_chunk.options.get_bool("jinja", false)
                {
                    let joined = merged_chunk.source.join("\n");
                    let jinja_result = crate::jinja::process_body(
                        &joined, output_ext, metadata, registry,
                    );
                    sc_fragments.extend(jinja_result.sc_fragments);
                    jinja_chunk = CodeChunk {
                        source: jinja_result.text.lines().map(|l| l.to_string()).collect(),
                        ..merged_chunk.clone()
                    };
                    &jinja_chunk
                } else {
                    &merged_chunk
                };
                let (mut chunk_elements, chunk_preamble) = block::evaluate_block(chunk_ref, fig_dir, fig_ext, ctx, cache)?;
                elements.append(&mut chunk_elements);
                preamble.extend(chunk_preamble);
            }
            Block::CodeBlock(cb) => {
                elements.push(Element::CodeSource {
                    code: cb.code.clone(),
                    lang: cb.lang.clone(),
                    label: String::new(),
                    filename: cb.filename.clone(),
                    lst_cap: None,
                });
            }
            Block::Div(div) => {
                if !div_is_visible(&div.classes, &div.attrs, output_ext, &metadata.var) {
                    continue;
                }
                if div.classes.iter().any(|c| c == "hidden") {
                    let child_result = evaluate(&div.children, fig_dir, fig_ext, output_ext, metadata, registry, ctx, cache)?;
                    preamble.extend(child_result.preamble);
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
                        lst_cap: None,
                    });
                    continue;
                }
                let child_result = evaluate(&div.children, fig_dir, fig_ext, output_ext, metadata, registry, ctx, cache)?;
                sc_fragments.extend(child_result.sc_fragments);
                preamble.extend(child_result.preamble);
                let mut child_elements = child_result.elements;
                let mut div_attrs = div.attrs.clone();
                if let Some(ref id) = div.id {
                    if id.starts_with("fig-") && !div_attrs.contains_key("fig_cap") {
                        let (remaining, caption) = crate::modules::figure::separate_figure_caption(&child_elements);
                        if !caption.is_empty() {
                            div_attrs.insert("fig_cap".to_string(), caption);
                            child_elements = remaining;
                        }
                    } else if id.starts_with("tbl-") && !div_attrs.contains_key("tbl_cap") {
                        let (remaining, caption) = crate::modules::table::separate_table_caption(&child_elements);
                        if !caption.is_empty() {
                            div_attrs.insert("tbl_cap".to_string(), caption);
                            child_elements = remaining;
                        }
                    }
                }
                elements.push(Element::Div {
                    classes: div.classes.clone(),
                    id: div.id.clone(),
                    attrs: div_attrs,
                    children: child_elements,
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

    Ok(EvalResult { elements, sc_fragments, preamble })
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
    if let Err(e) = std::fs::create_dir_all(fig_dir) {
        eprintln!("Warning: failed to create figure directory {}: {}", fig_dir.display(), e);
    }
    let fig_width = options.fig_width();
    let fig_height = options.fig_height();
    let fig_full_path = fig_dir.join(format!("{}-1.{}", label, fig_ext));
    // Use absolute path so the subprocess can write figures regardless of its cwd
    let fig_abs = if fig_full_path.is_relative() {
        std::env::current_dir().unwrap_or_default().join(&fig_full_path)
    } else {
        fig_full_path.clone()
    };
    let fig_full_str = if is_table_chunk {
        String::new()
    } else {
        fig_abs.to_string_lossy().replace('\\', "/")
    };

    // R and Python engines emit per-expression _SOURCE: tags for interleaved output.
    // Other engines get a single upfront Source entry.
    let engine_name = options.engine();
    let interleaved = !matches!(engine_name.as_str(), "sh")
        && !diagram::is_diagram_engine(&engine_name);
    if !interleaved {
        results.push(ChunkResult::Source(source.to_vec()));
    }

    // Dispatch to engine-specific capture
    let captured = match engine_name.as_str() {
        eng if diagram::is_diagram_engine(eng) => {
            // Diagram engines always produce SVG
            let svg_path = fig_dir.join(format!("{}-1.svg", label));
            return diagram::execute_diagram(
                &code,
                eng,
                &svg_path,
                source,
                options,
            );
        }
        "sh" => {
            let session = ctx.sh.as_mut()
                .ok_or_else(|| anyhow::anyhow!("{}", crate::tools::not_found_message(&crate::tools::SH)))?;
            session.capture(&code)?
        }
        "python" => {
            let session = ctx.python.as_mut()
                .ok_or_else(|| anyhow::anyhow!("{}", crate::tools::not_found_message(&crate::tools::PYTHON)))?;
            let dpi: f64 = options
                .get_opt_string("dpi")
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| options.metadata.dpi.unwrap_or(150.0));
            session.capture(&code, &fig_full_str, fig_width, fig_height, dpi)?
        }
        _ => {
            let session = ctx.r.as_mut()
                .ok_or_else(|| anyhow::anyhow!("{}", crate::tools::not_found_message(&crate::tools::RSCRIPT)))?;
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

    // Fallback: if engine emitted no SOURCE tags (e.g. parse error), add upfront source.
    if interleaved && !results.iter().any(|r| matches!(r, ChunkResult::Source(_))) {
        results.insert(0, ChunkResult::Source(source.to_vec()));
    }

    Ok(results)
}

/// Evaluate an inline code expression for the given engine.
pub fn evaluate_inline(engine: &str, expr: &str, ctx: &mut EngineContext) -> Result<String> {
    match engine {
        "sh" => {
            let session = ctx.sh.as_mut()
                .ok_or_else(|| anyhow::anyhow!("{}", crate::tools::not_found_message(&crate::tools::SH)))?;
            session.evaluate_inline(expr)
        }
        "python" => {
            let session = ctx.python.as_mut()
                .ok_or_else(|| anyhow::anyhow!("{}", crate::tools::not_found_message(&crate::tools::PYTHON)))?;
            session.evaluate_inline(expr)
        }
        "r" => {
            let session = ctx.r.as_mut()
                .ok_or_else(|| anyhow::anyhow!("{}", crate::tools::not_found_message(&crate::tools::RSCRIPT)))?;
            session.evaluate_inline(expr)
        }
        _ => Err(anyhow::anyhow!("Unknown inline engine: {}", engine)),
    }
}

/// Generate a unique sentinel that cannot appear in user output.
pub fn make_sentinel() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("__CALEPIN_{:x}_{:x}__", std::process::id(), seq)
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

    let source_prefix = format!("{}_SOURCE:", sentinel);
    let output_prefix = format!("{}_OUTPUT:", sentinel);
    let asis_prefix = format!("{}_ASIS:", sentinel);
    let error_prefix = format!("{}_ERROR:", sentinel);
    let warning_prefix = format!("{}_WARNING:", sentinel);
    let message_prefix = format!("{}_MESSAGE:", sentinel);
    let plot_prefix = format!("{}_PLOT:", sentinel);
    let preamble_prefix = format!("{}_PREAMBLE:", sentinel);

    for part in rest.split(&sep) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(text) = part.strip_prefix(&source_prefix) {
            if !text.is_empty() {
                let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
                results.push(ChunkResult::Source(lines));
            }
        } else if let Some(text) = part.strip_prefix(&error_prefix) {
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
        } else if let Some(text) = part.strip_prefix(&preamble_prefix) {
            if !text.is_empty() {
                results.push(ChunkResult::Preamble(text.to_string()));
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Visibility logic (used by div rendering and span rendering)
// ---------------------------------------------------------------------------

pub fn format_matches(format_name: &str, output_format: &str) -> bool {
    format_name == output_format
}

fn div_is_visible(
    classes: &[String],
    attrs: &HashMap<String, String>,
    output_format: &str,
    meta_extra: &HashMap<String, crate::value::Value>,
) -> bool {
    content_is_visible(classes, attrs, output_format, Some(meta_extra))
}

pub fn content_is_visible(
    classes: &[String],
    attrs: &HashMap<String, String>,
    output_format: &str,
    meta_extra: Option<&HashMap<String, crate::value::Value>>,
) -> bool {
    let is_content_visible = classes.iter().any(|c| c == "content-visible");
    let is_content_hidden = classes.iter().any(|c| c == "content-hidden");

    if !is_content_visible && !is_content_hidden {
        return true;
    }

    let when_format = attrs.get("when_format").map(|s| s.as_str());
    let unless_format = attrs.get("unless_format").map(|s| s.as_str());
    let when_meta = attrs.get("when_meta").map(|s| s.as_str());
    let unless_meta = attrs.get("unless_meta").map(|s| s.as_str());

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

fn meta_is_truthy(extra: Option<&HashMap<String, crate::value::Value>>, key: &str) -> bool {
    use crate::value::Value;
    let extra = match extra {
        Some(e) => e,
        None => return false,
    };
    let parts: Vec<&str> = key.split('.').collect();
    let mut current: &Value = match extra.get(parts[0]) {
        Some(v) => v,
        None => return false,
    };
    for part in &parts[1..] {
        match current.get(part) {
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
    fn test_content_visible_when_format() {
        let classes = vec!["content-visible".to_string()];
        let mut attrs = HashMap::new();
        attrs.insert("when_format".to_string(), "html".to_string());
        assert!(content_is_visible(&classes, &attrs, "html", None));
        assert!(!content_is_visible(&classes, &attrs, "latex", None));
    }

    #[test]
    fn test_content_visible_unless_format() {
        let classes = vec!["content-visible".to_string()];
        let mut attrs = HashMap::new();
        attrs.insert("unless_format".to_string(), "latex".to_string());
        assert!(content_is_visible(&classes, &attrs, "html", None));
        assert!(!content_is_visible(&classes, &attrs, "latex", None));
    }

    #[test]
    fn test_content_hidden_when_format() {
        let classes = vec!["content-hidden".to_string()];
        let mut attrs = HashMap::new();
        attrs.insert("when_format".to_string(), "html".to_string());
        assert!(!content_is_visible(&classes, &attrs, "html", None));
        assert!(content_is_visible(&classes, &attrs, "latex", None));
    }

    #[test]
    fn test_content_hidden_unless_format() {
        let classes = vec!["content-hidden".to_string()];
        let mut attrs = HashMap::new();
        attrs.insert("unless_format".to_string(), "latex".to_string());
        assert!(!content_is_visible(&classes, &attrs, "html", None));
        assert!(content_is_visible(&classes, &attrs, "latex", None));
    }

    #[test]
    fn test_content_visible_combined() {
        let classes = vec!["content-visible".to_string()];
        let mut attrs = HashMap::new();
        attrs.insert("when_format".to_string(), "html".to_string());
        attrs.insert("unless_format".to_string(), "html".to_string());
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
        use crate::value::Value;
        let classes = vec!["content-visible".to_string()];
        let mut attrs = HashMap::new();
        attrs.insert("when_meta".to_string(), "draft".to_string());
        let mut extra = HashMap::new();
        extra.insert("draft".to_string(), Value::Bool(true));
        assert!(content_is_visible(&classes, &attrs, "html", Some(&extra)));
        extra.insert("draft".to_string(), Value::Bool(false));
        assert!(!content_is_visible(&classes, &attrs, "html", Some(&extra)));
    }

    #[test]
    fn test_unless_meta() {
        use crate::value::Value;
        let classes = vec!["content-hidden".to_string()];
        let mut attrs = HashMap::new();
        attrs.insert("unless_meta".to_string(), "published".to_string());
        let mut extra = HashMap::new();
        extra.insert("published".to_string(), Value::Bool(true));
        assert!(content_is_visible(&classes, &attrs, "html", Some(&extra)));
        extra.insert("published".to_string(), Value::Bool(false));
        assert!(!content_is_visible(&classes, &attrs, "html", Some(&extra)));
    }

    #[test]
    fn test_when_meta_dot_notation() {
        use crate::value::Value;
        let classes = vec!["content-visible".to_string()];
        let mut attrs = HashMap::new();
        attrs.insert("when_meta".to_string(), "options.show-code".to_string());
        let mut extra = HashMap::new();
        let mut opts = crate::value::Table::new();
        opts.insert("show-code".to_string(), Value::Bool(true));
        extra.insert("options".to_string(), Value::Table(opts));
        assert!(content_is_visible(&classes, &attrs, "html", Some(&extra)));
    }
}
