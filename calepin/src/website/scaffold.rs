use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use crate::utils::path::{absolutize_from, normalize_path};

pub(crate) fn scaffold_website(dir: &Path, theme: &str, force: bool) -> Result<()> {
    let (files, binary_files) = website_scaffold(theme)?;
    let docs = normalize_path(&absolutize_from(&std::env::current_dir()?, dir));
    fs::create_dir_all(&docs).with_context(|| format!("failed to create {}", docs.display()))?;

    for (path, contents) in files {
        write_scaffold_file(&docs.join(path), contents, force)?;
    }
    for (path, contents) in binary_files {
        write_scaffold_bytes(&docs.join(path), contents, force)?;
    }
    Ok(())
}

type WebsiteScaffoldFiles = (
    &'static [(&'static str, &'static str)],
    &'static [(&'static str, &'static [u8])],
);

fn website_scaffold(theme: &str) -> Result<WebsiteScaffoldFiles> {
    match theme {
        "calepin" => Ok((
            CALEPIN_WEBSITE_SCAFFOLD_FILES,
            CALEPIN_WEBSITE_SCAFFOLD_BINARY_FILES,
        )),
        "academic" => Ok((
            ACADEMIC_WEBSITE_SCAFFOLD_FILES,
            ACADEMIC_WEBSITE_SCAFFOLD_BINARY_FILES,
        )),
        _ => Err(anyhow!(
            "unknown website scaffold theme `{theme}`; use one of calepin, academic"
        )),
    }
}

fn write_scaffold_bytes(path: &Path, contents: &[u8], force: bool) -> Result<()> {
    if path.exists() && !force {
        return Err(anyhow!(
            "{} already exists; pass --force to overwrite it",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

fn write_scaffold_file(path: &Path, contents: &str, force: bool) -> Result<()> {
    write_scaffold_bytes(path, contents.as_bytes(), force)
}

/// Shared by every website scaffold: keeps Calepin's regenerable artifacts and
/// the generated entry files staged beside each document out of the user's
/// repository. The entry files are hidden, so without this a `git add -A` after
/// an interrupted build quietly commits them.
const SCAFFOLD_GITIGNORE: (&str, &str) = (
    ".gitignore",
    include_str!("../assets/scaffolds/website/gitignore"),
);

const CALEPIN_WEBSITE_SCAFFOLD_FILES: &[(&str, &str)] = &[
    SCAFFOLD_GITIGNORE,
    (
        "calepin.toml",
        include_str!("../assets/scaffolds/website/calepin/calepin.toml"),
    ),
    (
        "README.md",
        include_str!("../assets/scaffolds/website/calepin/docs/README.md"),
    ),
    (
        "index.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/index.typ"),
    ),
    (
        "404.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/404.typ"),
    ),
    (
        "about.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/about.typ"),
    ),
    (
        "guide/features.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/guide/features.typ"),
    ),
    (
        "guide/writing.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/guide/writing.typ"),
    ),
    (
        "blog.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/blog.typ"),
    ),
    (
        "posts/first-post.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/posts/first-post.typ"),
    ),
    (
        "posts/theme-tour.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/posts/theme-tour.typ"),
    ),
    (
        "posts/writing-with-footnotes.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/posts/writing-with-footnotes.typ"),
    ),
    (
        "posts/code-and-results.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/posts/code-and-results.typ"),
    ),
    (
        "posts/multilingual-notes.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/posts/multilingual-notes.typ"),
    ),
    (
        "fr/index.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/fr/index.typ"),
    ),
    (
        "fr/about.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/fr/about.typ"),
    ),
    (
        "fr/guide/features.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/fr/guide/features.typ"),
    ),
    (
        "fr/guide/writing.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/fr/guide/writing.typ"),
    ),
    (
        "fr/blog.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/fr/blog.typ"),
    ),
    (
        "fr/posts/first-post.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/fr/posts/first-post.typ"),
    ),
    (
        "fr/posts/theme-tour.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/fr/posts/theme-tour.typ"),
    ),
    (
        "fr/posts/writing-with-footnotes.typ",
        include_str!(
            "../assets/scaffolds/website/calepin/docs/fr/posts/writing-with-footnotes.typ"
        ),
    ),
    (
        "fr/posts/code-and-results.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/fr/posts/code-and-results.typ"),
    ),
    (
        "fr/posts/multilingual-notes.typ",
        include_str!("../assets/scaffolds/website/calepin/docs/fr/posts/multilingual-notes.typ"),
    ),
];

const CALEPIN_WEBSITE_SCAFFOLD_BINARY_FILES: &[(&str, &[u8])] = &[
    (
        "assets/portrait.jpg",
        include_bytes!("../assets/scaffolds/website/calepin/docs/assets/portrait.jpg"),
    ),
    (
        "assets/flowers_01.jpg",
        include_bytes!("../assets/scaffolds/website/calepin/docs/assets/flowers_01.jpg"),
    ),
];

const ACADEMIC_WEBSITE_SCAFFOLD_FILES: &[(&str, &str)] = &[
    SCAFFOLD_GITIGNORE,
    (
        "calepin.toml",
        include_str!("../assets/scaffolds/website/academic/calepin.toml"),
    ),
    (
        "README.md",
        include_str!("../assets/scaffolds/website/academic/docs/README.md"),
    ),
    (
        "index.typ",
        include_str!("../assets/scaffolds/website/academic/docs/index.typ"),
    ),
    (
        "404.typ",
        include_str!("../assets/scaffolds/website/academic/docs/404.typ"),
    ),
    (
        "about.typ",
        include_str!("../assets/scaffolds/website/academic/docs/about.typ"),
    ),
    (
        "guide/features.typ",
        include_str!("../assets/scaffolds/website/academic/docs/guide/features.typ"),
    ),
    (
        "guide/writing.typ",
        include_str!("../assets/scaffolds/website/academic/docs/guide/writing.typ"),
    ),
    (
        "blog.typ",
        include_str!("../assets/scaffolds/website/academic/docs/blog.typ"),
    ),
    (
        "posts/first-post.typ",
        include_str!("../assets/scaffolds/website/academic/docs/posts/first-post.typ"),
    ),
    (
        "posts/theme-tour.typ",
        include_str!("../assets/scaffolds/website/academic/docs/posts/theme-tour.typ"),
    ),
    (
        "posts/writing-with-footnotes.typ",
        include_str!("../assets/scaffolds/website/academic/docs/posts/writing-with-footnotes.typ"),
    ),
    (
        "posts/code-and-results.typ",
        include_str!("../assets/scaffolds/website/academic/docs/posts/code-and-results.typ"),
    ),
    (
        "posts/multilingual-notes.typ",
        include_str!("../assets/scaffolds/website/academic/docs/posts/multilingual-notes.typ"),
    ),
    (
        "fr/index.typ",
        include_str!("../assets/scaffolds/website/academic/docs/fr/index.typ"),
    ),
    (
        "fr/about.typ",
        include_str!("../assets/scaffolds/website/academic/docs/fr/about.typ"),
    ),
    (
        "fr/guide/features.typ",
        include_str!("../assets/scaffolds/website/academic/docs/fr/guide/features.typ"),
    ),
    (
        "fr/guide/writing.typ",
        include_str!("../assets/scaffolds/website/academic/docs/fr/guide/writing.typ"),
    ),
    (
        "fr/blog.typ",
        include_str!("../assets/scaffolds/website/academic/docs/fr/blog.typ"),
    ),
    (
        "fr/posts/first-post.typ",
        include_str!("../assets/scaffolds/website/academic/docs/fr/posts/first-post.typ"),
    ),
    (
        "fr/posts/theme-tour.typ",
        include_str!("../assets/scaffolds/website/academic/docs/fr/posts/theme-tour.typ"),
    ),
    (
        "fr/posts/writing-with-footnotes.typ",
        include_str!(
            "../assets/scaffolds/website/academic/docs/fr/posts/writing-with-footnotes.typ"
        ),
    ),
    (
        "fr/posts/code-and-results.typ",
        include_str!("../assets/scaffolds/website/academic/docs/fr/posts/code-and-results.typ"),
    ),
    (
        "fr/posts/multilingual-notes.typ",
        include_str!("../assets/scaffolds/website/academic/docs/fr/posts/multilingual-notes.typ"),
    ),
];

const ACADEMIC_WEBSITE_SCAFFOLD_BINARY_FILES: &[(&str, &[u8])] = &[
    (
        "assets/portrait.jpg",
        include_bytes!("../assets/scaffolds/website/academic/docs/assets/portrait.jpg"),
    ),
    (
        "assets/flowers_01.jpg",
        include_bytes!("../assets/scaffolds/website/academic/docs/assets/flowers_01.jpg"),
    ),
];
