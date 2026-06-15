use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::typst::io::write_if_changed;

fn runtime_source() -> Result<String> {
    const SHARED_CHUNK_STYLING: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/assets/themes/shared/typst/code-block.typ");
    let runtime_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/assets/typst-runtime");
    let mut files: Vec<PathBuf> = fs::read_dir(runtime_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension() == Some(OsStr::new("typ")))
        .collect();

    files.sort_unstable_by(|a, b| a.file_name().cmp(&b.file_name()));

    let mut source = String::new();
    source.push_str(&fs::read_to_string(SHARED_CHUNK_STYLING)?);
    for path in files {
        source.push_str(&fs::read_to_string(&path)?);
    }

    Ok(source)
}

pub fn write_runtime(root: &Path) -> Result<PathBuf> {
    let path = root.join(".calepin").join("calepin.typ");
    let source = runtime_source()?;
    write_if_changed(&path, &source)?;
    Ok(path)
}

#[cfg(test)]
mod runtime_tests;
