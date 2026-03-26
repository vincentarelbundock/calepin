//! Module system: registry, manifests, and built-in modules.
//!
//! Each module lives in its own directory under `modules/`.
//! Transform traits at pipeline stages:
//!   - `TransformElementRaw` / `TransformElementRendered` -- per div/span
//!   - `TransformBody` -- body string mutation
//!   - `TransformPage` -- page template variable injection
//!   - `TransformDocument` -- post-assembly document mutation

pub mod manifest;
pub mod registry;

// Transform traits
pub mod transform_body;
pub mod transform_document;
pub mod transform_page;

// Built-in modules
pub mod append_footnotes;
pub mod convert_math;
pub mod convert_svg_pdf;
pub mod embed_images;
pub mod figure;
pub mod highlight;
pub mod layout;
pub mod split_slides;
pub mod table;
pub mod tabset;
