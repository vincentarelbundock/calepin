//! Rd AST to markdown converter.
//!
//! Parses the JSON AST produced by extract_rdocs.R and emits clean
//! markdown-style .qmd files with TOML front matter.  Cross-references
//! (`\link[pkg]{fun}`) are resolved to hyperlinks using pkgdown site
//! URLs discovered from DESCRIPTION files, with rdrr.io as fallback.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct RdTopic {
    pub topic: String,
    pub nodes: Vec<RdNode>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum RdNode {
    Text {
        tag: String,
        text: String,
    },
    Parent {
        tag: String,
        children: Vec<RdNode>,
        #[serde(default)]
        option: Option<String>,
    },
}

impl RdNode {
    fn tag(&self) -> &str {
        match self {
            RdNode::Text { tag, .. } | RdNode::Parent { tag, .. } => tag,
        }
    }

    fn children(&self) -> &[RdNode] {
        match self {
            RdNode::Parent { children, .. } => children,
            RdNode::Text { .. } => &[],
        }
    }

    fn text(&self) -> &str {
        match self {
            RdNode::Text { text, .. } => text,
            RdNode::Parent { .. } => "",
        }
    }

    fn option(&self) -> Option<&str> {
        match self {
            RdNode::Parent { option, .. } => option.as_deref(),
            RdNode::Text { .. } => None,
        }
    }
}

/// Collect plain text from a node, stripping all markup.
fn collect_text(node: &RdNode) -> String {
    match node {
        RdNode::Text { text, .. } => text.clone(),
        RdNode::Parent { children, .. } => {
            children.iter().map(collect_text).collect::<Vec<_>>().join("")
        }
    }
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

pub struct RdRenderer<'a> {
    pub package: &'a str,
    pub urls: &'a HashMap<String, String>,
}

impl<'a> RdRenderer<'a> {
    /// Convert a full Rd topic to a .qmd string.
    pub fn render_topic(&self, topic: &RdTopic) -> String {
        let mut title = String::new();
        let mut sections: Vec<(String, String)> = Vec::new();

        for node in &topic.nodes {
            match node.tag() {
                "\\title" => title = collect_text(node),
                "\\description" => {
                    sections.push(("Description".into(), self.render_section_body(node)))
                }
                "\\usage" => sections.push(("Usage".into(), render_usage(node))),
                "\\arguments" => {
                    sections.push(("Arguments".into(), self.render_arguments(node)))
                }
                "\\details" => {
                    sections.push(("Details".into(), self.render_section_body(node)))
                }
                "\\value" => sections.push(("Value".into(), self.render_section_body(node))),
                "\\note" => sections.push(("Note".into(), self.render_section_body(node))),
                "\\references" => {
                    sections.push(("References".into(), self.render_section_body(node)))
                }
                "\\seealso" => {
                    sections.push(("See Also".into(), self.render_section_body(node)))
                }
                "\\author" => {
                    sections.push(("Author".into(), self.render_section_body(node)))
                }
                "\\examples" => sections.push(("Examples".into(), render_examples(node))),
                "\\section" => {
                    let children = node.children();
                    if !children.is_empty() {
                        let sec_title = collect_text(&children[0]).trim().to_string();
                        let body = if children.len() > 1 {
                            self.render_children(&children[1..], 0)
                        } else {
                            String::new()
                        };
                        sections.push((sec_title, body.trim().to_string()));
                    }
                }
                _ => {} // \name, \alias, \keyword, \concept -- skip
            }
        }

        let mut out = format!("---\ntitle = \"`{}`\"\n---\n", topic.topic);
        if !title.is_empty() {
            out.push_str(&format!("\n*{}*\n", title.trim()));
        }
        for (heading, body) in &sections {
            out.push_str(&format!("\n## {}\n\n{}\n", heading, body));
        }
        out
    }

    // -----------------------------------------------------------------------
    // Link resolution
    // -----------------------------------------------------------------------

    /// Resolve a `\link` option + display text to a URL.
    fn resolve_link_url(&self, option: Option<&str>, text: &str) -> String {
        let text = text.trim();
        match option {
            // \link{fun} -- same package
            None => format!("{}.qmd", text),
            // \link[=topic]{text} -- same package, explicit topic
            Some(opt) if opt.starts_with('=') => format!("{}.qmd", &opt[1..]),
            Some(opt) => {
                let (pkg, topic) = match opt.split_once(':') {
                    Some((p, t)) => (p, t),
                    None => (opt, text),
                };
                if pkg == self.package {
                    format!("{}.qmd", topic)
                } else if let Some(base) = self.urls.get(pkg) {
                    format!("{}/reference/{}.html", base, topic)
                } else {
                    format!("https://rdrr.io/cran/{}/man/{}.html", pkg, topic)
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Node rendering
    // -----------------------------------------------------------------------

    /// Render a node to markdown.
    fn render_node(&self, node: &RdNode, depth: usize) -> String {
        match node.tag() {
            "TEXT" => node.text().to_string(),
            "RCODE" | "VERB" => node.text().to_string(),
            "COMMENT" => String::new(),
            "GROUP" => self.render_children(node.children(), depth),
            "\\code" => self.render_code(node),
            "\\bold" | "\\strong" => {
                format!("**{}**", self.render_children(node.children(), depth))
            }
            "\\emph" | "\\var" => {
                format!("*{}*", self.render_children(node.children(), depth))
            }
            "\\pkg" => format!("**{}**", self.render_children(node.children(), depth)),
            "\\link" | "\\linkS4class" => self.render_link(node),
            "\\file" | "\\env" | "\\option" | "\\command" => {
                format!("`{}`", collect_text(node))
            }
            "\\url" => format!("<{}>", collect_text(node)),
            "\\href" => {
                let children = node.children();
                let url =
                    if !children.is_empty() { collect_text(&children[0]) } else { String::new() };
                let text = if children.len() >= 2 {
                    self.render_children(&children[1..2], depth)
                } else {
                    url.clone()
                };
                format!("[{}]({})", text, url)
            }
            "\\email" => format!("<{}>", collect_text(node)),
            "\\doi" => {
                let doi = collect_text(node);
                format!("[doi:{}](https://doi.org/{})", doi, doi)
            }
            "\\eqn" => {
                let children = node.children();
                let latex =
                    if !children.is_empty() { collect_text(&children[0]) } else { String::new() };
                format!("${}$", latex)
            }
            "\\deqn" => {
                let children = node.children();
                let latex =
                    if !children.is_empty() { collect_text(&children[0]) } else { String::new() };
                format!("\n$$\n{}\n$$\n", latex)
            }
            "\\cr" => "\n".to_string(),
            "\\dots" | "\\ldots" => "...".to_string(),
            "\\R" => "R".to_string(),
            "\\sQuote" => format!("'{}'", self.render_children(node.children(), depth)),
            "\\dQuote" => format!("\"{}\"", self.render_children(node.children(), depth)),
            "\\acronym" => self.render_children(node.children(), depth),
            "\\preformatted" => format!("\n```\n{}\n```\n", collect_text(node)),
            "\\tabular" => self.render_tabular(node),
            "\\itemize" => self.render_itemize(node, depth),
            "\\enumerate" => self.render_enumerate(node, depth),
            "\\describe" => self.render_describe(node, depth),
            "\\item" => String::new(), // handled by parent list functions
            _ => self.render_children(node.children(), depth),
        }
    }

    /// Render all children and concatenate.
    fn render_children(&self, nodes: &[RdNode], depth: usize) -> String {
        let mut out = String::new();
        for node in nodes {
            out.push_str(&self.render_node(node, depth));
        }
        out
    }

    /// Render `\link` or `\linkS4class` as a markdown link.
    fn render_link(&self, node: &RdNode) -> String {
        let text = collect_text(node).trim().to_string();
        let url = self.resolve_link_url(node.option(), &text);
        format!("[`{}`]({})", text, url)
    }

    /// Render `\code`, detecting `\link` children to produce code-styled links.
    fn render_code(&self, node: &RdNode) -> String {
        let children = node.children();
        if let Some(link) = children.iter().find(|c| c.tag() == "\\link") {
            let display = collect_text(node).trim().to_string();
            let fun = collect_text(link).trim().to_string();
            let url = self.resolve_link_url(link.option(), &fun);
            format!("[`{}`]({})", display, url)
        } else {
            format!("`{}`", collect_text(node))
        }
    }

    // -----------------------------------------------------------------------
    // Section renderers
    // -----------------------------------------------------------------------

    fn render_section_body(&self, node: &RdNode) -> String {
        self.render_children(node.children(), 0).trim().to_string()
    }

    fn render_arguments(&self, node: &RdNode) -> String {
        let mut out = String::new();
        for child in node.children() {
            if child.tag() != "\\item" {
                continue;
            }
            let children = child.children();
            if children.is_empty() {
                continue;
            }
            let name = collect_text(&children[0]).trim().to_string();
            let desc = if children.len() >= 2 {
                self.render_children(&children[1..], 0).trim().to_string()
            } else {
                String::new()
            };
            out.push_str(&format!("**`{}`**\n: {}\n\n", name, desc));
        }
        out.trim_end().to_string()
    }

    // -----------------------------------------------------------------------
    // List renderers
    // -----------------------------------------------------------------------

    /// \itemize: \item is a marker, content follows as siblings until next \item.
    fn render_itemize(&self, node: &RdNode, depth: usize) -> String {
        let indent = "  ".repeat(depth);
        let mut lines: Vec<String> = Vec::new();
        let mut current = String::new();

        for child in node.children() {
            if child.tag() == "\\item" {
                let text = current.trim().to_string();
                if !text.is_empty() {
                    lines.push(format!("{}- {}", indent, text));
                }
                current.clear();
            } else {
                current.push_str(&self.render_node(child, depth + 1));
            }
        }
        let text = current.trim().to_string();
        if !text.is_empty() {
            lines.push(format!("{}- {}", indent, text));
        }

        format!("\n{}\n", lines.join("\n"))
    }

    fn render_enumerate(&self, node: &RdNode, depth: usize) -> String {
        let indent = "  ".repeat(depth);
        let mut lines: Vec<String> = Vec::new();
        let mut current = String::new();
        let mut counter = 0usize;

        for child in node.children() {
            if child.tag() == "\\item" {
                let text = current.trim().to_string();
                if !text.is_empty() {
                    lines.push(format!("{}{}. {}", indent, counter, text));
                }
                current.clear();
                counter += 1;
            } else {
                current.push_str(&self.render_node(child, depth + 1));
            }
        }
        let text = current.trim().to_string();
        if !text.is_empty() {
            lines.push(format!("{}{}. {}", indent, counter, text));
        }

        format!("\n{}\n", lines.join("\n"))
    }

    /// \describe: \item has 2 children (term, description).
    fn render_describe(&self, node: &RdNode, depth: usize) -> String {
        let indent = "  ".repeat(depth);
        let mut lines: Vec<String> = Vec::new();

        for child in node.children() {
            if child.tag() != "\\item" {
                continue;
            }
            let children = child.children();
            if children.is_empty() {
                continue;
            }
            let term = self.render_children(&children[0..1], depth + 1).trim().to_string();
            let desc = if children.len() >= 2 {
                self.render_children(&children[1..], depth + 1).trim().to_string()
            } else {
                String::new()
            };
            lines.push(format!("{}- **{}**: {}", indent, term, desc));
        }

        format!("\n{}\n", lines.join("\n"))
    }

    // -----------------------------------------------------------------------
    // Table renderer
    // -----------------------------------------------------------------------

    fn render_tabular(&self, node: &RdNode) -> String {
        let children = node.children();
        // children[0] is column spec (TEXT), children[1] is content
        if children.len() < 2 {
            return String::new();
        }
        let content = &children[1];

        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut current_row: Vec<String> = Vec::new();
        let mut current_cell = String::new();

        for child in content.children() {
            match child.tag() {
                "\\tab" => {
                    current_row.push(current_cell.trim().to_string());
                    current_cell.clear();
                }
                "\\cr" => {
                    current_row.push(current_cell.trim().to_string());
                    rows.push(current_row);
                    current_row = Vec::new();
                    current_cell.clear();
                }
                _ => current_cell.push_str(&self.render_node(child, 0)),
            }
        }
        if !current_row.is_empty() || !current_cell.is_empty() {
            current_row.push(current_cell.trim().to_string());
            rows.push(current_row);
        }

        if rows.is_empty() {
            return String::new();
        }
        let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0);

        let mut lines: Vec<String> = Vec::new();
        for (i, row) in rows.iter().enumerate() {
            let mut padded = row.clone();
            padded.resize(ncols, String::new());
            lines.push(format!("| {} |", padded.join(" | ")));
            if i == 0 {
                lines.push(format!("| {} |", vec!["---"; ncols].join(" | ")));
            }
        }

        format!("\n{}\n", lines.join("\n"))
    }
}

// ---------------------------------------------------------------------------
// Standalone helpers (no link context needed)
// ---------------------------------------------------------------------------

fn render_usage(node: &RdNode) -> String {
    let code = collect_text(node);
    let lines: Vec<&str> = code.lines().map(|l| l.trim_end()).collect();
    let start = lines.iter().position(|l| !l.trim().is_empty()).unwrap_or(0);
    let end =
        lines.iter().rposition(|l| !l.trim().is_empty()).map(|i| i + 1).unwrap_or(lines.len());
    format!("```r\n{}\n```", lines[start..end].join("\n"))
}

/// Render \examples by walking the AST to distinguish executable code from
/// display-only (\dontrun) and hidden (\dontshow) blocks.
///
/// - Regular RCODE/TEXT and \donttest -> ```{r} (executed by Calepin)
/// - \dontrun -> ```r (display only)
/// - \dontshow -> omitted
fn render_examples(node: &RdNode) -> String {
    let mut spans: Vec<(String, bool)> = Vec::new();
    collect_example_spans(node.children(), true, &mut spans);

    let spans: Vec<_> = spans.into_iter().filter(|(t, _)| !t.trim().is_empty()).collect();
    let mut merged: Vec<(String, bool)> = Vec::new();
    for (text, exec) in spans {
        if let Some(last) = merged.last_mut() {
            if last.1 == exec {
                last.0.push_str(&text);
                continue;
            }
        }
        merged.push((text, exec));
    }

    let mut out = String::new();
    for (code, exec) in &merged {
        let trimmed = code.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lines: Vec<&str> = trimmed.lines().map(|l| l.trim_end()).collect();
        let body = lines.join("\n");
        if *exec {
            out.push_str(&format!("```{{r}}\n{}\n```\n\n", body));
        } else {
            out.push_str(&format!("```r\n{}\n```\n\n", body));
        }
    }
    out.trim_end().to_string()
}

fn collect_example_spans(nodes: &[RdNode], executable: bool, out: &mut Vec<(String, bool)>) {
    for node in nodes {
        match node.tag() {
            "TEXT" | "RCODE" | "VERB" => {
                out.push((node.text().to_string(), executable));
            }
            "\\dontrun" => {
                collect_example_spans(node.children(), false, out);
            }
            "\\donttest" => {
                collect_example_spans(node.children(), executable, out);
            }
            "\\dontshow" => {}
            _ => {
                out.push((collect_text(node), executable));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// External package URL discovery (pure Rust, replaces R-side logic)
// ---------------------------------------------------------------------------

/// Walk the Rd AST to collect external package names from `\link[pkg]{...}`.
pub fn collect_linked_packages(topics: &[RdTopic], this_package: &str) -> HashSet<String> {
    let mut pkgs = HashSet::new();
    for topic in topics {
        for node in &topic.nodes {
            collect_links_from_node(node, this_package, &mut pkgs);
        }
    }
    pkgs
}

fn collect_links_from_node(node: &RdNode, this_package: &str, pkgs: &mut HashSet<String>) {
    match node {
        RdNode::Parent { tag, children, option, .. } => {
            if tag == "\\link" {
                if let Some(opt) = option {
                    if !opt.starts_with('=') {
                        let ext_pkg = opt.split(':').next().unwrap_or(opt);
                        if ext_pkg != this_package && !ext_pkg.is_empty() {
                            pkgs.insert(ext_pkg.to_string());
                        }
                    }
                }
            }
            for child in children {
                collect_links_from_node(child, this_package, pkgs);
            }
        }
        RdNode::Text { .. } => {}
    }
}

/// Regex pattern matching pkgdown-style documentation sites.
static PKGDOWN_RE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(
        r"github\.io|gitlab\.io|r-lib\.org|tidyverse\.org|tidymodels\.org|bioconductor\.org"
    ).unwrap()
});

/// Discover pkgdown URLs for a set of packages by reading their DESCRIPTION files.
///
/// Locates each package's DESCRIPTION via `R_LIBS` / `R_HOME` library paths,
/// parses the `URL` field (DCF format), and returns the first URL that looks
/// like a pkgdown site.
pub fn discover_pkgdown_urls(packages: &HashSet<String>) -> HashMap<String, String> {
    let lib_paths = r_library_paths();
    let mut urls = HashMap::new();
    for pkg in packages {
        if let Some(url) = find_pkgdown_url(pkg, &lib_paths) {
            urls.insert(pkg.clone(), url);
        }
    }
    urls
}

/// Parse the `URL` field from an R package DESCRIPTION file and return the
/// first URL that matches a pkgdown site pattern.
fn find_pkgdown_url(package: &str, lib_paths: &[std::path::PathBuf]) -> Option<String> {
    for lib in lib_paths {
        let desc_path = lib.join(package).join("DESCRIPTION");
        if let Ok(contents) = std::fs::read_to_string(&desc_path) {
            if let Some(url) = extract_pkgdown_url_from_dcf(&contents) {
                return Some(url);
            }
        }
    }
    None
}

/// Extract the first pkgdown-style URL from a DCF (Debian Control File) string.
fn extract_pkgdown_url_from_dcf(dcf: &str) -> Option<String> {
    // DCF format: "Key: value" with continuation lines starting with whitespace.
    // Find the URL field and collect its full value.
    let mut in_url = false;
    let mut url_value = String::new();

    for line in dcf.lines() {
        if line.starts_with("URL:") {
            in_url = true;
            url_value = line.strip_prefix("URL:").unwrap().trim().to_string();
        } else if in_url {
            if line.starts_with(char::is_whitespace) {
                // Continuation line
                url_value.push(' ');
                url_value.push_str(line.trim());
            } else {
                break;
            }
        }
    }

    if url_value.is_empty() {
        return None;
    }

    // Split on commas and whitespace, find the first pkgdown-looking URL.
    for url in url_value.split(|c: char| c == ',' || c.is_whitespace()) {
        let url = url.trim();
        if !url.is_empty() && PKGDOWN_RE.is_match(url) {
            return Some(url.trim_end_matches('/').to_string());
        }
    }

    None
}

/// Collect R library paths from the environment, similar to `.libPaths()`.
fn r_library_paths() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();

    // R_LIBS_USER, R_LIBS_SITE, R_LIBS (colon-separated)
    for var in &["R_LIBS_USER", "R_LIBS_SITE", "R_LIBS"] {
        if let Ok(val) = std::env::var(var) {
            for p in val.split(':') {
                let expanded = expand_r_path(p);
                let path = Path::new(&expanded);
                if path.is_dir() {
                    paths.push(path.to_path_buf());
                }
            }
        }
    }

    // Common default paths
    if let Ok(home) = std::env::var("HOME") {
        // macOS default user library
        let user_lib = Path::new(&home).join("Library/R");
        if user_lib.is_dir() {
            // Find version-specific subdirectory (e.g., Library/R/arm64/4.4/library)
            if let Ok(entries) = std::fs::read_dir(&user_lib) {
                for entry in entries.flatten() {
                    let lib_dir = find_library_subdir(&entry.path());
                    if let Some(d) = lib_dir {
                        if !paths.contains(&d) {
                            paths.push(d);
                        }
                    }
                }
            }
        }
    }

    // R_HOME/library
    if let Ok(r_home) = std::env::var("R_HOME") {
        let lib = Path::new(&r_home).join("library");
        if lib.is_dir() && !paths.contains(&lib) {
            paths.push(lib);
        }
    }

    // Homebrew R on macOS
    for prefix in &["/opt/homebrew/lib/R", "/usr/local/lib/R", "/usr/lib/R"] {
        let lib = Path::new(prefix).join("library");
        if lib.is_dir() && !paths.contains(&lib) {
            paths.push(lib);
        }
    }

    paths
}

/// Expand `%V` and `%p` placeholders in R library paths.
fn expand_r_path(path: &str) -> String {
    // These placeholders are common in R_LIBS_USER but we just strip them
    // if we can't resolve the version. The path won't match anything, which
    // is fine -- we have fallbacks.
    path.replace("%V", "*").replace("%p", "*")
        .replace('~', &std::env::var("HOME").unwrap_or_default())
}

/// Find a `library/` subdirectory under an R version directory.
fn find_library_subdir(dir: &Path) -> Option<std::path::PathBuf> {
    // Handles structures like: arm64/4.4/library or x86_64/4.3/library
    if !dir.is_dir() {
        return None;
    }
    // Direct library/ child
    let lib = dir.join("library");
    if lib.is_dir() {
        return Some(lib);
    }
    // One level deeper (version dir)
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let lib = entry.path().join("library");
            if lib.is_dir() {
                return Some(lib);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_pkgdown_url_from_dcf() {
        let dcf = "\
Package: ggplot2
Title: Create Elegant Data Visualisations Using the Grammar of Graphics
URL: https://ggplot2.tidyverse.org,
    https://github.com/tidyverse/ggplot2
BugReports: https://github.com/tidyverse/ggplot2/issues
";
        assert_eq!(
            extract_pkgdown_url_from_dcf(dcf),
            Some("https://ggplot2.tidyverse.org".to_string())
        );
    }

    #[test]
    fn test_extract_pkgdown_url_github_io() {
        let dcf = "\
Package: data.table
URL: https://r-datatable.com,
    https://Rdatatable.gitlab.io/data.table,
    https://github.com/Rdatatable/data.table
";
        assert_eq!(
            extract_pkgdown_url_from_dcf(dcf),
            Some("https://Rdatatable.gitlab.io/data.table".to_string())
        );
    }

    #[test]
    fn test_extract_pkgdown_url_none() {
        let dcf = "\
Package: base
Title: The R Base Package
";
        assert_eq!(extract_pkgdown_url_from_dcf(dcf), None);
    }

    #[test]
    fn test_collect_linked_packages() {
        let topics = vec![RdTopic {
            topic: "test".to_string(),
            nodes: vec![
                RdNode::Parent {
                    tag: "\\description".to_string(),
                    children: vec![RdNode::Parent {
                        tag: "\\link".to_string(),
                        children: vec![RdNode::Text {
                            tag: "TEXT".to_string(),
                            text: "tibble".to_string(),
                        }],
                        option: Some("tibble".to_string()),
                    }],
                    option: None,
                },
                RdNode::Parent {
                    tag: "\\link".to_string(),
                    children: vec![RdNode::Text {
                        tag: "TEXT".to_string(),
                        text: "fun".to_string(),
                    }],
                    option: Some("=topic".to_string()),
                },
            ],
        }];
        let linked = collect_linked_packages(&topics, "mypkg");
        assert!(linked.contains("tibble"));
        assert!(!linked.contains("mypkg"));
        assert_eq!(linked.len(), 1); // =topic links are same-package
    }
}
