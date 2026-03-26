//! Module system: registry, manifests, and built-in modules.
//!
//! Each module lives in its own directory under `modules/`.
//! Module kinds:
//!   - `TransformElement` -- pre-render element list mutation
//!   - `TransformElementChildren` -- per-div structural rewriting
//!   - `TransformSpan` -- span-level rendering
//!   - `TransformDocument` -- post-assembly document transformation
//!
//! External code should import from `crate::modules::` (this file),
//! never from individual submodules like `crate::modules::highlight::`.

pub mod manifest;
pub mod registry;

// Transform trait
pub mod transform_document;

// Built-in modules (private -- external access via re-exports below)
mod append_footnotes;
mod callout;
mod convert_math;
mod convert_svg_pdf;
mod embed_images;
mod figure;
mod highlight;
mod layout;
mod listing;
mod lorem;
mod pagebreak;
mod placeholder;
mod split_slides;
mod table;
mod tabset;
mod theorem;
mod video;

// ---------------------------------------------------------------------------
// Public API -- re-exports for external code
// ---------------------------------------------------------------------------

// Syntax highlighting engine (config parsing is internal to the module)
pub use highlight::Highlighter;

// Figure utilities: image variant selection and element var building
pub use figure::{select_image_variant, BuildFigureVars};

// Listing wrapper for lst- labeled code blocks
pub use listing::wrap_listing;

// Footnote state and rendering (owned by ElementRenderer, logic in append_footnotes module)
pub use append_footnotes::{FootnoteState, render_footnote_section};

// Typst math conversion
pub use convert_math::{convert_math_for_typst, strip_math_for_typst};

/// Return the cross-reference prefix for a class name, if any.
/// Combines theorem and callout prefix lookups.
pub fn prefix_for_class(class: &str) -> Option<&'static str> {
    theorem::theorem_prefix(class)
        .or_else(|| callout::callout_prefix(class))
}

/// List all built-in syntax highlighting theme names.
pub fn list_builtin_themes() -> Vec<&'static str> {
    let mut names: Vec<&str> = highlight::BUILTIN_THEMES.files()
        .filter_map(|f| {
            if f.path().extension()?.to_str()? == "tmTheme" {
                f.path().file_stem()?.to_str()
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names
}
