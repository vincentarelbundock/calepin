use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::typst::io::write_if_changed;

#[cfg(test)]
fn runtime_source() -> Result<String> {
    runtime_source_with_syntax_theme(&crate::html::HtmlSyntaxTheme::builtin())
}

fn runtime_source_with_syntax_theme(syntax_theme: &crate::html::HtmlSyntaxTheme) -> Result<String> {
    let mut source = syntax_theme.typst_runtime_source();
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

#[cfg(test)]
pub fn write_runtime(root: &Path) -> Result<PathBuf> {
    write_runtime_with_syntax_theme(root, &crate::html::HtmlSyntaxTheme::builtin())
}

pub(crate) fn write_runtime_with_syntax_theme(
    root: &Path,
    syntax_theme: &crate::html::HtmlSyntaxTheme,
) -> Result<PathBuf> {
    let path = root.join(".calepin").join("calepin.typ");
    let source = runtime_source_with_syntax_theme(syntax_theme)?;
    write_if_changed(&path, &source)?;
    Ok(path)
}

#[cfg(test)]
#[path = "runtime_tests.rs"]
mod runtime_tests;
