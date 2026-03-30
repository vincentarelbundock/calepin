//! Directory tree printing utility for scaffold commands.

use std::path::Path;

/// Print a directory tree to stderr with a maximum depth.
pub fn print_tree(root: &Path, max_depth: usize) {
    eprintln!("  {}/", root.display());
    print_tree_dir(root, "", max_depth, 0);
}

fn print_tree_dir(dir: &Path, prefix: &str, max_depth: usize, depth: usize) {
    let mut entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return,
    };
    entries.sort_by_key(|e| e.file_name());

    let count = entries.len();
    for (i, entry) in entries.iter().enumerate() {
        let is_last = i + 1 == count;
        let connector = if is_last { "`-- " } else { "|-- " };
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if entry.path().is_dir() {
            eprintln!("  {}{}{}/", prefix, connector, name);
            if depth + 1 < max_depth {
                let child_prefix = if is_last {
                    format!("{}    ", prefix)
                } else {
                    format!("{}|   ", prefix)
                };
                print_tree_dir(&entry.path(), &child_prefix, max_depth, depth + 1);
            }
        } else {
            eprintln!("  {}{}{}", prefix, connector, name);
        }
    }
}
