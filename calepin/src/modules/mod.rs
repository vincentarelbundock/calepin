//! Module system: registry, manifests, and built-in modules.
//!
//! Each module lives in its own directory under `modules/`.
//! Three module kinds:
//!   - `TransformElement` -- pre-render element list mutation
//!   - `TransformElementChildren` -- per-div structural rewriting
//!   - `TransformDocument` -- post-assembly document transformation

pub mod manifest;
pub mod registry;

// Transform trait
pub mod transform_document;

// Built-in modules
pub mod append_footnotes;
pub mod callout;
pub mod convert_svg_pdf;
pub mod embed_images;
pub mod figure;
pub mod highlight;
pub mod layout;
pub mod split_slides;
pub mod table;
pub mod tabset;
pub mod theorem;
