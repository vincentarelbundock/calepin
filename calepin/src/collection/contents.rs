//! Document tree expansion: resolving `[[contents]]` sections, glob patterns,
//! and directory scanning with recursive nesting.

use std::collections::HashSet;
use std::path::Path;

use crate::config::{ContentSection, IncludeEntry};

/// A node in the expanded document tree.
#[derive(Debug, Clone)]
pub enum DocumentNode {
    Document { path: String, title: Option<String> },
    Section { title: String, index: Option<String>, documents: Vec<DocumentNode> },
}

/// Expand `[[contents]]` into a `DocumentNode` tree, resolving globs and
/// directory scans. When `lang` is Some, only sections matching that
/// language (or with no lang) are included.
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
        // Skip standalone sections (rendered but not in navigation)
        if section.standalone {
            continue;
        }
        // Language filter
        if let Some(filter_lang) = lang {
            if let Some(ref section_lang) = section.lang {
                if section_lang != filter_lang {
                    continue;
                }
            }
        }

        let includes = section.resolved_include();
        let expanded = expand_includes(&includes, &section.exclude, base_dir);

        if let Some(text) = section.display_text() {
            result.push(DocumentNode::Section {
                title: text.to_string(),
                index: section.display_href().map(String::from),
                documents: expanded,
            });
        } else {
            // Untitled section: items appear at the top level
            result.extend(expanded);
        }
    }
    result
}

/// Expand a list of include entries into document nodes.
pub fn expand_includes(
    includes: &[IncludeEntry],
    exclude: &[String],
    base_dir: &Path,
) -> Vec<DocumentNode> {
    let mut result = Vec::new();

    for entry in includes {
        match entry {
            IncludeEntry::Path(pattern) => {
                let path = base_dir.join(pattern);
                // Check if it's a directory -> recursive nested scan
                if path.is_dir() {
                    let nodes = expand_directory(&path, base_dir, exclude);
                    result.extend(nodes);
                } else {
                    // Glob or literal path
                    for p in expand_glob(pattern, base_dir) {
                        if !is_excluded(&p, exclude, base_dir) {
                            result.push(DocumentNode::Document { path: p, title: None });
                        }
                    }
                }
            }
            IncludeEntry::Item { text, href, .. } => {
                if let Some(href) = href {
                    for p in expand_glob(href, base_dir) {
                        if !is_excluded(&p, exclude, base_dir) {
                            result.push(DocumentNode::Document {
                                path: p,
                                title: text.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    result
}

/// Check if a relative path matches any exclude glob pattern.
fn is_excluded(rel_path: &str, exclude: &[String], base_dir: &Path) -> bool {
    if exclude.is_empty() {
        return false;
    }
    let abs_path = base_dir.join(rel_path);
    let abs_str = abs_path.display().to_string();
    for pattern in exclude {
        let abs_base = std::fs::canonicalize(base_dir).unwrap_or_else(|_| base_dir.to_path_buf());
        let full = abs_base.join(pattern).display().to_string();
        if let Ok(entries) = glob::glob(&full) {
            for entry in entries.flatten() {
                if entry.display().to_string() == abs_str {
                    return true;
                }
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

// ---------------------------------------------------------------------------
// Directory scanning
// ---------------------------------------------------------------------------

/// Scan a directory recursively, building a nested `DocumentNode` tree.
///
/// Each subdirectory becomes a `Section` node whose title is derived from the
/// directory name. Files become `Document` nodes. Exclusion patterns are applied.
fn expand_directory(
    dir: &Path,
    base_dir: &Path,
    exclude: &[String],
) -> Vec<DocumentNode> {
    // Canonicalize to avoid `./ ` prefix issues with the glob crate
    let abs_dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let abs_base = std::fs::canonicalize(base_dir).unwrap_or_else(|_| base_dir.to_path_buf());

    // Collect all .qmd files under this directory
    let include = vec!["**/*.qmd".to_string()];
    let mut matched_files: HashSet<String> = HashSet::new();
    for pattern in &include {
        let full = abs_dir.join(pattern).display().to_string();
        if let Ok(entries) = glob::glob(&full) {
            for entry in entries.flatten() {
                if let Ok(rel) = entry.strip_prefix(&abs_base) {
                    matched_files.insert(rel.display().to_string());
                }
            }
        }
    }

    // Remove excluded files
    for pattern in exclude {
        let full = abs_dir.join(pattern).display().to_string();
        if let Ok(entries) = glob::glob(&full) {
            for entry in entries.flatten() {
                if let Ok(rel) = entry.strip_prefix(&abs_base) {
                    matched_files.remove(&rel.display().to_string());
                }
            }
        }
    }

    build_dir_tree(&abs_dir, &abs_base, &matched_files)
}

/// Recursively build a document tree from a directory, only including files
/// that are in the `allowed` set.
fn build_dir_tree(
    dir: &Path,
    base_dir: &Path,
    allowed: &HashSet<String>,
) -> Vec<DocumentNode> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut files: Vec<DocumentNode> = Vec::new();
    let mut subdirs: Vec<(String, std::path::PathBuf)> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if path.is_dir() {
            if name.starts_with('.') || name.starts_with("__") {
                continue;
            }
            subdirs.push((name.to_string(), path));
        } else if path.extension().and_then(|e| e.to_str()) == Some("qmd") {
            if let Ok(rel) = path.strip_prefix(base_dir) {
                let rel_str = rel.display().to_string();
                if allowed.contains(&rel_str) {
                    if name == "index.qmd" || name == "_index.qmd" {
                        continue; // Used as section index
                    }
                    files.push(DocumentNode::Document {
                        path: rel_str,
                        title: None,
                    });
                }
            }
        }
    }

    files.sort_by(|a, b| {
        let pa = match a { DocumentNode::Document { path, .. } => path.as_str(), _ => "" };
        let pb = match b { DocumentNode::Document { path, .. } => path.as_str(), _ => "" };
        pa.cmp(pb)
    });

    subdirs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut result = Vec::new();
    for (dirname, subdir_path) in &subdirs {
        let children = build_dir_tree(subdir_path, base_dir, allowed);
        if children.is_empty() {
            continue;
        }

        let index = find_index_file(subdir_path, base_dir, allowed);
        let title = prettify_dirname(dirname);
        result.push(DocumentNode::Section {
            title,
            index,
            documents: children,
        });
    }

    result.extend(files);
    result
}

/// Look for an index.qmd or _index.qmd in a directory.
fn find_index_file(dir: &Path, base_dir: &Path, allowed: &HashSet<String>) -> Option<String> {
    for name in &["index.qmd", "_index.qmd"] {
        let path = dir.join(name);
        if path.exists() {
            if let Ok(rel) = path.strip_prefix(base_dir) {
                let rel_str = rel.display().to_string();
                if allowed.contains(&rel_str) {
                    return Some(rel_str);
                }
            }
        }
    }
    None
}

/// Convert a directory name to a pretty title.
fn prettify_dirname(name: &str) -> String {
    name.replace('-', " ")
        .replace('_', " ")
        .split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prettify_dirname() {
        assert_eq!(prettify_dirname("getting-started"), "Getting Started");
        assert_eq!(prettify_dirname("api_reference"), "Api Reference");
        assert_eq!(prettify_dirname("FAQ"), "FAQ");
    }
}
