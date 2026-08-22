//! Generate Typst API reference pages from Python source.
//!
//! The pipeline is: resolve a package's public surface ([`resolve`]), extract
//! signatures and docstrings from the AST ([`extract`]), parse the docstrings
//! with `pydocstring`, and render Typst ([`emit`]).
//!
//! Signatures come from static analysis of the source rather than runtime
//! introspection, so annotations render exactly as the author wrote them —
//! `int | None`, not `typing.Optional[int]`.

pub mod emit;
pub mod extract;
pub mod markdown;
pub mod model;
pub mod resolve;
pub mod typst_escape;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

pub use resolve::{Package, Resolution};

/// What a generation run produced, for reporting back to the caller.
pub struct Report {
    pub written: Vec<String>,
    pub unresolved: Vec<resolve::Unresolved>,
    pub template_written: bool,
}

/// Generate one `.typ` per exported definition, plus an index and (on first
/// run) the styling template.
pub fn generate(package: &Package, out_dir: &Path, website: bool) -> Result<Report> {
    let resolution = package.resolve()?;

    fs::create_dir_all(out_dir).with_context(|| format!("cannot create {}", out_dir.display()))?;

    // The template is user-editable, so never clobber an existing one.
    let template_path = out_dir.join("api.typ");
    let template_written = !template_path.exists();
    if template_written {
        fs::write(&template_path, emit::TEMPLATE)
            .with_context(|| format!("cannot write {}", template_path.display()))?;
    }

    let mut written = Vec::new();
    for item in &resolution.items {
        let stem = emit::file_stem(item.qualname());
        let path = out_dir.join(format!("{stem}.typ"));
        fs::write(&path, emit::render_page(item, "api.typ", website))
            .with_context(|| format!("cannot write {}", path.display()))?;
        written.push(format!("{stem}.typ"));
    }

    let index_path = out_dir.join("index.typ");
    fs::write(
        &index_path,
        emit::render_index(&resolution.items, "api.typ", &package.name, website),
    )
    .with_context(|| format!("cannot write {}", index_path.display()))?;
    written.push("index.typ".to_string());

    Ok(Report {
        written,
        unresolved: resolution.unresolved,
        template_written,
    })
}
