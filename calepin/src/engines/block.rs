// Block-level code chunk evaluation.
//
// - evaluate_block() — Execute a code chunk (with caching), then map ChunkResults to Elements
//                      based on chunk options (echo, eval, include, results, warning, message).
// - apply_comment()  — Prepend a comment prefix to each line of output text.

use anyhow::Result;
use std::path::Path;

use crate::engines::{self, EngineContext};
use crate::engines::cache::CacheState;
use crate::types::{ChunkResult, CodeChunk, Element, ResultsMode};

/// Evaluate a code chunk and return Elements (interleaved source + output).
/// This is the block-level counterpart to `inline::evaluate_inline()`.
pub fn evaluate_block(
    chunk: &CodeChunk,
    fig_dir: &Path,
    fig_ext: &str,
    ctx: &mut EngineContext,
    cache: &mut CacheState,
) -> Result<Vec<Element>> {
    let opts = &chunk.options;

    // Always go through execute_chunk_cached so the upstream digest advances,
    // even for include=false or eval=false chunks.
    let results = engines::cache::execute_chunk_cached(&chunk.source, opts, &chunk.label, fig_dir, fig_ext, ctx, cache)?;

    if !opts.include() {
        return Ok(vec![]);
    }
    let comment = opts.comment();
    let lang = opts.engine();
    let mut elements = Vec::new();

    for result in results {
        match result {
            ChunkResult::Source(lines) => {
                if opts.echo() {
                    elements.push(Element::CodeSource {
                        code: lines.join("\n"),
                        lang: lang.clone(),
                        label: chunk.label.clone(),
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
        }
    }

    Ok(elements)
}

/// Apply comment prefix to output lines.
fn apply_comment(text: &str, comment: &str) -> String {
    text.lines()
        .map(|line| format!("{}{}", comment, line))
        .collect::<Vec<_>>()
        .join("\n")
}
