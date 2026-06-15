use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::typst::io::write_if_changed;

pub const RUNTIME_SOURCE: &str = concat!(
    include_str!("../assets/typst-runtime/template.typ"),
    include_str!("../assets/typst-runtime/themes.typ"),
    include_str!("../assets/typst-runtime/state.typ"),
    include_str!("../assets/typst-runtime/render.typ"),
    include_str!("../assets/typst-runtime/options.typ"),
    include_str!("../assets/typst-runtime/chunk.typ"),
);

pub fn write_runtime(root: &Path) -> Result<PathBuf> {
    let path = root.join(".calepin").join("calepin.typ");
    write_if_changed(&path, RUNTIME_SOURCE)?;
    Ok(path)
}

#[cfg(test)]
mod tests;
