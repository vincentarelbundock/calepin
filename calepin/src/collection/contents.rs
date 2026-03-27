//! Document tree expansion: resolving `[[contents]]` sections and glob patterns.

use crate::config::{ContentSection, DocumentEntry};

/// A node in the expanded document tree.
#[derive(Debug, Clone)]
pub enum DocumentNode {
    Document { path: String, title: Option<String> },
    Section { title: String, index: Option<String>, documents: Vec<DocumentNode> },
}

/// Expand `[[contents]]` into a flat `DocumentNode` tree, resolving globs.
/// When `lang` is Some, only sections matching that language (or with no lang) are included.
pub fn expand_contents(contents: &[ContentSection], base_dir: &std::path::Path) -> Vec<DocumentNode> {
    expand_contents_for_lang(contents, base_dir, None)
}

/// Expand `[[contents]]` filtered by language.
pub fn expand_contents_for_lang(
    contents: &[ContentSection],
    base_dir: &std::path::Path,
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
        let expanded = expand_section_documents(&section.pages, base_dir);
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
fn expand_section_documents(entries: &[DocumentEntry], base_dir: &std::path::Path) -> Vec<DocumentNode> {
    let mut result = Vec::new();
    for entry in entries {
        match entry {
            DocumentEntry::Path(pattern) => {
                for path in expand_glob(pattern, base_dir) {
                    result.push(DocumentNode::Document { path, title: None });
                }
            }
            DocumentEntry::Named { title, page } => {
                for path in expand_glob(page, base_dir) {
                    result.push(DocumentNode::Document { path, title: Some(title.clone()) });
                }
            }
        }
    }
    result
}

/// Expand a string as a glob pattern if it contains `*`, otherwise return as-is.
pub fn expand_glob(pattern: &str, base_dir: &std::path::Path) -> Vec<String> {
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
