// Block-level code chunk evaluation.
//
// - evaluate_block() — Execute a code chunk (with caching), then map ChunkResults to Elements
//                      based on chunk options (echo, eval, include, results, warning, message).
// - apply_comment()  — Prepend a comment prefix to each line of output text.

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use crate::engines::{self, EngineContext};
use crate::engines::cache::CacheState;
use crate::types::{ChunkResult, CodeChunk, Element, ResultsMode};

/// Evaluate a code chunk and return Elements (interleaved source + output).
/// This is the block-level counterpart to `inline::evaluate_inline()`.
#[inline(never)]
pub fn evaluate_block(
    chunk: &CodeChunk,
    fig_dir: &Path,
    fig_ext: &str,
    ctx: &mut EngineContext,
    cache: &mut CacheState,
) -> Result<(Vec<Element>, Vec<String>)> {
    let opts = &chunk.options;

    // Always go through execute_chunk_cached so the upstream digest advances,
    // even for include=false or eval=false chunks.
    let results = engines::cache::execute_chunk_cached(&chunk.source, opts, &chunk.label, fig_dir, fig_ext, ctx, cache)?;

    // Extract preamble before any early returns — preamble is always collected.
    let preamble: Vec<String> = results.iter().filter_map(|r| match r {
        ChunkResult::Preamble(text) => Some(text.clone()),
        _ => None,
    }).collect();

    if !opts.include() {
        return Ok((vec![], preamble));
    }
    let comment = opts.comment();
    let lang = opts.engine();
    let mut elements = Vec::new();

    // For fenced echo, merge interleaved per-expression sources back into a single block
    // so the fenced header/pipe-comments wrap all source lines together.
    let echo_val = opts.get_string("echo", "true");
    let results = if echo_val == "fenced" {
        let mut merged_lines: Vec<String> = Vec::new();
        let mut non_source: Vec<ChunkResult> = Vec::new();
        for r in results {
            match r {
                ChunkResult::Source(lines) => merged_lines.extend(lines),
                other => non_source.push(other),
            }
        }
        let mut merged = Vec::new();
        if !merged_lines.is_empty() {
            merged.push(ChunkResult::Source(merged_lines));
        }
        merged.extend(non_source);
        merged
    } else {
        results
    };

    for result in results {
        match result {
            ChunkResult::Source(lines) => {
                let echo_val = opts.get_string("echo", "true");
                if echo_val == "true" || echo_val == "fenced" {
                    let code = if echo_val == "fenced" {
                        let header = if chunk.label.is_empty() {
                            format!("```{{{}}}", lang)
                        } else {
                            format!("```{{{}, {}}}", lang, chunk.label)
                        };
                        let mut body = String::new();
                        for pc in &chunk.pipe_comments {
                            body.push_str("#| ");
                            body.push_str(pc);
                            body.push('\n');
                        }
                        body.push_str(&lines.join("\n"));
                        format!("{}\n{}\n```", header, body)
                    } else {
                        lines.join("\n")
                    };
                    elements.push(Element::CodeSource {
                        code,
                        lang: lang.clone(),
                        label: chunk.label.clone(),
                        filename: opts.get_string("filename", ""),
                        lst_cap: opts.lst_cap(),
                    });
                }
            }
            ChunkResult::Output(text) => match opts.results() {
                ResultsMode::Hide => {}
                ResultsMode::Asis => {
                    elements.push(Element::CodeAsis { text });
                }
                ResultsMode::Markup => {
                    let commented = apply_comment(&text, &comment);
                    elements.push(Element::CodeOutput { text: commented });
                }
            },
            ChunkResult::Asis(text) => {
                // knit_print methods often wrap output in Pandoc raw blocks:
                //   ```{=html}\n...\n```
                // Strip the wrapper and emit the inner content directly as CodeAsis.
                // If there is no raw block wrapper, the content is markdown (e.g.,
                // knitr::kable pipe tables) and should be rendered as Text.
                if let Some(inner) = strip_raw_block_wrapper(&text) {
                    elements.push(Element::CodeAsis { text: inner });
                } else {
                    elements.push(Element::Text { content: text });
                }
            }
            ChunkResult::Warning(text) => {
                if opts.warning() {
                    elements.push(Element::CodeWarning { text });
                }
            }
            ChunkResult::Message(text) => {
                if opts.message() {
                    elements.push(Element::CodeMessage { text });
                }
            }
            ChunkResult::Error(text) => {
                elements.push(Element::CodeError { text });
            }
            ChunkResult::Plot(path) => {
                elements.push(Element::Figure {
                    path,
                    alt: opts.fig_alt().unwrap_or_default(),
                    caption: opts.fig_cap(),
                    label: chunk.label.clone(),
                    number: None,
                    attrs: opts.to_figure_attrs(),
                });
            }
            ChunkResult::Preamble(_) => {} // already extracted above
        }
    }

    // If the chunk has a tbl- label, wrap asis output in a Div so the
    // table structural handler and cross-ref system can process it.
    if chunk.label.starts_with("tbl-") {
        let caption = opts.tbl_cap().unwrap_or_default();
        let mut div_children = Vec::new();
        let mut other = Vec::new();
        for el in elements {
            match &el {
                Element::CodeAsis { .. } | Element::Text { .. } => div_children.push(el),
                _ => other.push(el),
            }
        }
        if !caption.is_empty() {
            div_children.push(Element::Text { content: format!("\n\n{}", caption) });
        }
        if !div_children.is_empty() {
            other.push(Element::Div {
                classes: vec![],
                id: Some(chunk.label.clone()),
                attrs: HashMap::new(),
                children: div_children,
            });
        }
        return Ok((other, preamble));
    }

    Ok((elements, preamble))
}

/// Strip Pandoc raw block wrappers (` ```{=html}\n...\n``` `) from knit_asis output.
/// Returns `Some(inner)` if the text was wrapped in a raw block fence,
/// or `None` if there was no wrapper (i.e., the content is plain markdown).
fn strip_raw_block_wrapper(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("```{=") {
        if let Some(after_lang) = rest.strip_suffix("```") {
            // Find the end of the opening fence line (e.g., "html}\n")
            if let Some(newline_pos) = after_lang.find('\n') {
                let lang_close = &after_lang[..newline_pos];
                if lang_close.ends_with('}') {
                    return Some(after_lang[newline_pos + 1..].trim().to_string());
                }
            }
        }
    }
    None
}

/// Apply comment prefix to output lines.
fn apply_comment(text: &str, comment: &str) -> String {
    text.lines()
        .map(|line| format!("{}{}", comment, line))
        .collect::<Vec<_>>()
        .join("\n")
}
