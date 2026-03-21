//! Rendering pipeline and shared machinery.
//!
//! Orchestrators:
//!   - `elements` — ElementRenderer: dispatches each Element to filters/templates
//!   - `div` — div rendering pipeline
//!   - `span` — span rendering pipeline
//!
//! Shared machinery:
//!   - `template` — {{variable}} substitution + page templates
//!   - `markdown` — comrak markdown-to-HTML/Typst conversion
//!   - `latex` — markdown-to-LaTeX AST conversion
//!   - `markers` — math/raw output protection

pub mod ast;
pub mod div;
pub mod elements;
pub mod html_ast;
pub mod latex;
pub mod latex_emit;
pub mod markdown;
pub mod markers;
pub mod typst_ast;

pub mod span;
pub mod template;
