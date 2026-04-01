//! Module system: registry, manifests, and built-in modules.
//!
//! Each module lives in its own directory under `modules/`.
//! Module kinds:
//!   - `TransformElement` -- pre-render element list mutation
//!   - `TransformElementChildren` -- per-div structural rewriting
//!   - `TransformSpan` -- span-level rendering
//!   - `TransformDocument` -- post-assembly document transformation
//!   - `TransformProject` -- cross-page coordination (navigation, cross-refs)
//!
//! External code should import from `crate::modules::` (this file),
//! never from individual submodules like `crate::modules::highlight::`.

pub mod registry;

// Transform traits
pub mod transform_document;
pub mod project_modules;
#[allow(dead_code)]
pub mod external;

// Built-in modules (private -- external access via re-exports below)
mod append_footnotes;
mod convert_math;
mod convert_svg_pdf;
mod document_listing;
mod embed_images;
mod figure;
mod highlight;
mod layout;
mod listing;
mod lorem;
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
pub use highlight::{Highlighter, HighlightConfig};

// Figure utilities: image variant selection and element var building
pub use figure::{select_image_variant, BuildFigureVars};

// Listing wrapper for lst- labeled code blocks
pub use listing::wrap_listing;

// Footnote state and rendering (owned by ElementRenderer, logic in append_footnotes module)
pub use append_footnotes::{FootnoteState, render_footnote_section};

// Typst math conversion
pub use convert_math::{convert_math_for_typst, strip_math_for_typst};

/// Return the cross-reference prefix for a class name, if any.
pub fn prefix_for_class(class: &str) -> Option<&'static str> {
    theorem::theorem_prefix(class)
}

/// Render child elements, joining non-empty results with double newlines.
pub fn render_children(children: &[crate::types::Element], render_element: &dyn Fn(&crate::types::Element) -> String) -> String {
    children.iter()
        .map(render_element)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// List all built-in color scheme names.
pub fn list_builtin_color_schemes() -> Vec<&'static str> {
    crate::config::extension::builtin_color_names()
}
