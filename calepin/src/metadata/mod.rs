//! Document metadata: types, parsing, and date resolution.

mod types;
mod parse;
mod merge;

pub use types::*;
pub use parse::{split_frontmatter, parse_metadata};
