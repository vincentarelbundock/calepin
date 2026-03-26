//! Document metadata: types, parsing, targets, and config loading.

mod types;
mod parse;
mod merge;
mod targets;
mod load;

pub use types::*;
pub use parse::{split_frontmatter, parse_metadata};
pub use targets::{Target, resolve_target, resolve_target_output_path, target_vars_to_jinja_from_meta};
pub use load::{
    LanguageConfig, ContentSection, DocumentEntry, NavbarConfig, PostCommand,
    load_project_metadata, builtin_metadata,
    SHARED_TOML, DOCUMENT_TOML, COLLECTION_TOML,
};
