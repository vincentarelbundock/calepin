use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::typst::io::write_if_changed;

fn runtime_source() -> Result<String> {
    const SHARED_CHUNK_STYLING: &str =
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/assets/themes/shared/typst/code-block.typ");
    let mut source = fs::read_to_string(SHARED_CHUNK_STYLING)?;
    for path in runtime_files()? {
        source.push_str(&fs::read_to_string(path.as_path())?);
    }

    Ok(source)
}

fn runtime_files() -> Result<Vec<PathBuf>> {
    const RUNTIME_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/assets/typst-runtime");

    let mut files: Vec<PathBuf> = fs::read_dir(RUNTIME_DIR)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension() == Some(OsStr::new("typ")))
        .collect();

    files.sort_unstable_by_key(|path| {
        path.file_name()
            .unwrap_or_else(|| OsStr::new(""))
            .to_owned()
    });

    Ok(files)
}

pub fn write_runtime(root: &Path) -> Result<PathBuf> {
    let path = root.join(".calepin").join("calepin.typ");
    let source = runtime_source()?;
    write_if_changed(&path, &source)?;
    Ok(path)
}

#[cfg(test)]
mod runtime_tests;
