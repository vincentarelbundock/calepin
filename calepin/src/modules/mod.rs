//! Module system: registry, manifests, themes, and built-in modules.
//!
//! Each module lives in its own directory under `modules/`.
//! Transform traits: `TransformElementRaw`, `TransformElementRendered`, `TransformBody`.

pub mod highlight;
pub mod manifest;
pub mod registry;
pub mod theme;
pub mod transform_body;

// Built-in element transform modules
pub mod tabset;
pub mod layout;
pub mod figure_div;
pub mod table_div;

// Built-in body transform modules
pub mod append_footnotes_html;
pub mod embed_images_html;
pub mod inject_color_defs_latex;
pub mod inject_syntax_css_html;
pub mod split_slides_html;
