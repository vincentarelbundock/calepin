//! Rendering pipeline and shared machinery.
//!
//! Orchestrators:
//!   - `elements` — ElementRenderer: dispatches each Element to filters/templates
//!   - `div` — div rendering pipeline
//!   - `span` — span rendering pipeline
//!
//! Shared machinery:
//!   - `template` — {{variable}} substitution + page templates
//!   - `convert` — comrak options, image attrs, render entry points
//!   - `html_emit` / `latex_emit` / `typst_emit` / `markdown_emit` — format emitters
//!   - `markers` — math/raw output protection

pub mod ast;
pub mod div;
pub mod elements;
pub mod html_emit;
pub mod latex_emit;
pub mod convert;
pub mod markdown_emit;
pub mod markers;
pub mod metadata;
pub mod typst_emit;

pub mod span;
pub mod template;
pub mod formats;
pub mod typst_compile;
