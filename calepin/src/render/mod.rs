//! Rendering: Element → format-specific body string.
//!
//! Orchestrators:
//!   - `elements` — ElementRenderer: dispatches each Element via render_text/render_div/render_templated
//!   - `div` — div rendering pipeline
//!   - `span` — span rendering pipeline
//!
//! Per-element transforms:
//!   - `transform_element/` — callout, theorem, code, figure enrichment
//!
//! AST emitters:
//!   - `emit/` — shared walker + html/latex/typst/markdown emitters
//!
//! Shared machinery:
//!   - `template` — {{variable}} substitution + page templates
//!   - `convert` — comrak options, image attrs, render entry points
//!   - `markers` — math/raw output protection

pub mod filter;
pub mod div;
pub mod elements;
pub mod convert;
pub mod markers;
pub mod metadata;

pub mod span;
pub mod template;

