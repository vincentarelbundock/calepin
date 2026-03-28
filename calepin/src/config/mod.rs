//! Document metadata: types, parsing, targets, and config loading.

pub mod context;
pub mod value;
mod types;
mod parse;
mod merge;
mod targets;
mod load;

pub use types::*;
pub use parse::{split_frontmatter, parse_metadata};
pub use targets::{Target, resolve_target, resolve_target_output_path, build_jinja_vars};
pub use load::{
    LanguageConfig, ContentSection, IncludeEntry, NavbarConfig, PostCommand,
    load_project_metadata, builtin_metadata,
    SHARED_TOML, DOCUMENT_TOML, COLLECTION_TOML,
};
pub use context::{ProjectContext, resolve_context, apply_writer_override};
