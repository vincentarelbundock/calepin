//! Document tree expansion: resolving `[[contents]]` sections and glob patterns.

use std::path::Path;

use crate::config::{ContentSection, DocumentEntry};

/// A node in the expanded document tree.
#[derive(Debug, Clone)]
pub enum DocumentNode {
    Document { path: String, title: Option<String> },
    Section { title: String, index: Option<String>, documents: Vec<DocumentNode> },
}

/// Expand `[[contents]]` into a flat `DocumentNode` tree, resolving globs.
/// When `lang` is Some, only sections matching that language (or with no lang) are included.
pub fn expand_contents(contents: &[ContentSection], base_dir: &Path) -> Vec<DocumentNode> {
    expand_contents_for_lang(contents, base_dir, None)
}

/// Expand `[[contents]]` filtered by language.
pub fn expand_contents_for_lang(
    contents: &[ContentSection],
    base_dir: &Path,
    lang: Option<&str>,
) -> Vec<DocumentNode> {
    let mut result = Vec::new();
    for section in contents {
        // Language filter: if lang is requested, skip sections that don't match
        if let Some(filter_lang) = lang {
            if let Some(ref section_lang) = section.lang {
                if section_lang != filter_lang {
                    continue;
                }
            }
            // Sections without lang are included for all languages
        }

        // Auto-discovery mode: recursively build sections from directory structure
        if let Some(ref auto_dir) = section.auto {
            let nodes = expand_auto_directory(auto_dir, base_dir, &section.exclude);
            if let Some(ref title) = section.title {
                result.push(DocumentNode::Section {
                    title: title.clone(),
                    index: section.index.clone(),
                    documents: nodes,
                });
            } else {
                result.extend(nodes);
            }
            continue;
        }

        let expanded = expand_section_documents(&section.pages, base_dir, &section.exclude);
        if let Some(ref title) = section.title {
            result.push(DocumentNode::Section {
                title: title.clone(),
                index: section.index.clone(),
                documents: expanded,
            });
        } else {
            // Untitled section: pages appear at the top level
            result.extend(expanded);
        }
    }
    result
}

/// Expand document entries within a single section, resolving globs.
fn expand_section_documents(
    entries: &[DocumentEntry],
    base_dir: &Path,
    exclude: &[String],
) -> Vec<DocumentNode> {
    let mut result = Vec::new();
    for entry in entries {
        match entry {
            DocumentEntry::Path(pattern) => {
                for path in expand_glob(pattern, base_dir) {
                    if !is_excluded(&path, exclude) {
                        result.push(DocumentNode::Document { path, title: None });
                    }
                }
            }
            DocumentEntry::Named { title, page } => {
                for path in expand_glob(page, base_dir) {
                    if !is_excluded(&path, exclude) {
                        result.push(DocumentNode::Document { path, title: Some(title.clone()) });
                    }
                }
            }
        }
    }
    result
}

/// Recursively build a `DocumentNode` tree from a directory.
///
/// For each subdirectory, creates a `Section` node with the directory name as
/// title (titlecased, dashes/underscores replaced with spaces). If an `index.qmd`
/// exists in the subdirectory, it becomes the section's index page. `.qmd` files
/// at each level become `Document` leaf nodes. Entries are sorted alphabetically.
fn expand_auto_directory(dir: &str, base_dir: &Path, exclude: &[String]) -> Vec<DocumentNode> {
    let abs_dir = base_dir.join(dir);
    if !abs_dir.is_dir() {
        return Vec::new();
    }

    let mut documents = Vec::new();
    let mut subdirs = Vec::new();

    // Read directory entries, sorted by name
    let mut entries: Vec<_> = match std::fs::read_dir(&abs_dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return Vec::new(),
    };
    entries.sort_by_key(|e| e.file_name());

    for entry in &entries {
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden files/dirs
        if name_str.starts_with('.') || name_str.starts_with('_') {
            continue;
        }

        let rel_path = format!("{}/{}", dir, name_str);

        if file_type.is_dir() {
            subdirs.push(rel_path);
        } else if file_type.is_file()
            && name_str.ends_with(".qmd")
            && name_str != "index.qmd"
        {
            if !is_excluded(&rel_path, exclude) {
                documents.push(DocumentNode::Document {
                    path: rel_path,
                    title: None,
                });
            }
        }
    }

    // Process subdirectories into sections
    let mut result = Vec::new();

    // Loose documents at this level come first
    result.append(&mut documents);

    for subdir in subdirs {
        let subdir_name = Path::new(&subdir)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();

        // Check for index.qmd in the subdirectory
        let index_path = format!("{}/index.qmd", subdir);
        let index = if base_dir.join(&index_path).is_file() && !is_excluded(&index_path, exclude) {
            Some(index_path)
        } else {
            None
        };

        let children = expand_auto_directory(&subdir, base_dir, exclude);
        if children.is_empty() && index.is_none() {
            continue;
        }

        result.push(DocumentNode::Section {
            title: titlecase_dirname(&subdir_name),
            index,
            documents: children,
        });
    }

    result
}

/// Convert a directory name to a display title.
/// Replaces dashes and underscores with spaces, then titlecases each word.
fn titlecase_dirname(name: &str) -> String {
    name.replace('-', " ")
        .replace('_', " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().to_string() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Check if a relative path matches any exclude glob pattern.
fn is_excluded(rel_path: &str, exclude: &[String]) -> bool {
    for pattern in exclude {
        if let Ok(glob) = glob::Pattern::new(pattern) {
            if glob.matches(rel_path) {
                return true;
            }
        }
    }
    false
}

/// Expand a string as a glob pattern if it contains `*`, otherwise return as-is.
pub fn expand_glob(pattern: &str, base_dir: &Path) -> Vec<String> {
    if !pattern.contains('*') {
        return vec![pattern.to_string()];
    }
    let full_pattern = base_dir.join(pattern).display().to_string();
    let mut paths: Vec<String> = glob::glob(&full_pattern)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter_map(|path| {
            path.strip_prefix(base_dir).ok()
                .map(|rel| rel.display().to_string())
        })
        .collect();
    paths.sort();
    paths
}
