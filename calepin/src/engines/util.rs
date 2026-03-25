// Engine detection utilities.
//
// - needs_engine() — Scan parsed blocks, inline expressions, and metadata to determine
//                    whether a specific engine (R or Python) needs to be initialized.

use crate::types::Block;
use crate::metadata::Metadata;

/// Check whether any parsed blocks or metadata fields require a specific engine.
/// For "r", also returns true if there are code chunks with no explicit engine
/// (R is the default). For other engines, only matches explicit engine names.
pub fn needs_engine(blocks: &[Block], body: &str, metadata: &Metadata, engine_name: &str) -> bool {
    check_blocks_for_engine(blocks, body, engine_name) || metadata_has_inline_code(metadata, engine_name)
}

/// Check whether any metadata fields contain inline code for the given engine.
fn metadata_has_inline_code(meta: &Metadata, engine: &str) -> bool {
    let pattern = format!("`{{{}", engine);
    let check = |s: &Option<String>| s.as_ref().map_or(false, |v| v.contains(&pattern));
    check(&meta.title) || check(&meta.subtitle) || check(&meta.date) || check(&meta.abstract_text)
        || meta.authors.iter().any(|a| a.name.literal.contains(&pattern))
}

fn check_blocks_for_engine(blocks: &[Block], body: &str, engine_name: &str) -> bool {
    for block in blocks {
        match block {
            Block::Code(chunk) => {
                // Skip chunks that won't execute
                if !chunk.options.eval() {
                    continue;
                }
                if chunk.options.engine() == engine_name {
                    return true;
                }
            }
            Block::Div(div) => {
                if check_blocks_for_engine(&div.children, body, engine_name) {
                    return true;
                }
            }
            _ => {}
        }
    }
    // Check for inline code: `{engine} ...`
    body.contains(&format!("`{{{}", engine_name))
}
