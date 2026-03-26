//! Module system: registry, manifests, themes, and built-in modules.
//!
//! Each module lives in its own directory under `modules/`.
//! Transform traits at 5 pipeline stages:
//!   - `TransformAsset` -- pre-render asset preparation
//!   - `TransformElementRaw` / `TransformElementRendered` -- per div/span
//!   - `TransformBody` -- body string mutation
//!   - `TransformPage` -- page template variable injection
//!   - `TransformDocument` -- post-assembly document mutation

pub mod manifest;
pub mod registry;
pub mod theme;

// Transform traits
pub mod transform_body;
pub mod transform_document;
pub mod transform_page;

// Built-in element transform modules
pub mod tabset;
pub mod layout;
pub mod figure_div;
pub mod table_div;

// Built-in body transform modules
pub mod append_footnotes_html;
pub mod convert_svg_pdf;
pub mod embed_images_html;
pub mod highlight;
pub mod inject_color_defs_latex;
pub mod split_slides_html;
