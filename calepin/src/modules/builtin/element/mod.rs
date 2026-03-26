//! Built-in element plugins: tabset, layout, figure, table.
//!
//! These operate on div elements during rendering. Tabset and layout
//! are registered as element plugins (input=raw). Figure and table
//! provide helper functions used by div.rs for figure/table div enrichment.

pub mod figure;
pub mod layout;
pub mod table;
pub mod tabset;
