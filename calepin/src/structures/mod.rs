//! Structures: compound elements that require per-child rendering control.
//!
//! These are called directly by the orchestrator (div.rs) before
//! the filter pipeline, since they need to render children individually
//! rather than working from a pre-rendered string.

pub mod figure;
pub mod layout;
pub mod table;
pub mod tabset;
