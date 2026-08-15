use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::Serialize;

use crate::utils::static_files::collect_files_by;
use crate::utils::template::no_autoescape_env;

use super::paths::{relative_or_self, slash_path};
use super::site::SiteMetadata;
use super::url::{absolute_site_url, absolute_site_url_without_index};
use super::util::{clean_optional_string, xml_escape};
use super::{
    WebsiteConfig, DEFAULT_ROBOTS_TEMPLATE, LLMS_FILE, ROBOTS_FILE, ROBOTS_TEMPLATE_DIR,
    ROBOTS_TEMPLATE_FILE,
};

/// Writes the sitemap from every built page except the 404 page.
pub(super) fn write_sitemap(
    out_dir: &Path,
    base_url: Option<&str>,
    hrefs: &BTreeSet<String>,
) -> Result<()> {
    let path = out_dir.join("sitemap.xml");
    let Some(base_url) = base_url else {
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove stale sitemap {}", path.display()))?;
        }
        return Ok(());
    };

    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for href in hrefs {
        xml.push_str("  <url><loc>");
        xml.push_str(&xml_escape(&absolute_site_url_without_index(
            base_url, href,
        )));
        xml.push_str("</loc></url>\n");
    }
    xml.push_str("</urlset>\n");

    fs::write(&path, xml).with_context(|| format!("failed to write {}", path.display()))
}

/// Writes `llms.txt`: a Markdown index of the site — title, description, and
/// one linked, described entry per page — for LLM consumers that cannot walk
/// the rendered HTML. Links are absolute when `base-url` is configured and
/// root-relative otherwise.
pub(super) fn write_llms_txt(
    out_dir: &Path,
    enabled: bool,
    base_url: Option<&str>,
    metadata: &SiteMetadata,
    pages_index: &serde_json::Value,
) -> Result<()> {
    let path = out_dir.join(LLMS_FILE);
    if !enabled {
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove stale {}", path.display()))?;
        }
        return Ok(());
    }

    let mut out = format!(
        "# {}\n",
        metadata.title.as_deref().unwrap_or("Documentation")
    );
    if let Some(description) = metadata.description.as_deref() {
        out.push_str(&format!("\n> {description}\n"));
    }
    out.push_str("\n## Pages\n\n");

    let mut entries = pages_index
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| llms_entry(entry, base_url))
        .collect::<Vec<_>>();
    entries.sort();
    for (_, line) in &entries {
        out.push_str(line);
    }

    fs::write(&path, out).with_context(|| format!("failed to write {}", path.display()))
}

/// Returns the sort key and rendered list line for one page.
fn llms_entry(entry: &serde_json::Value, base_url: Option<&str>) -> Option<(String, String)> {
    let href = clean_optional_string(entry.get("href")?.as_str())?;
    let url = match base_url {
        Some(base_url) => absolute_site_url_without_index(base_url, &href),
        None => format!("/{}", href.trim_start_matches('/')),
    };
    let title = clean_optional_string(entry.get("title").and_then(|title| title.as_str()))
        .unwrap_or_else(|| href.clone());
    let excerpt = entry
        .get("excerpt")
        .and_then(|excerpt| excerpt.as_str())
        .and_then(|excerpt| clean_optional_string(Some(excerpt)));
    let line = match excerpt {
        Some(excerpt) => format!("- [{title}]({url}): {excerpt}\n"),
        None => format!("- [{title}]({url})\n"),
    };
    Some((href, line))
}

#[derive(Serialize)]
struct RobotsTemplateContext<'a> {
    config: &'a WebsiteConfig,
    sitemap_url: Option<String>,
}

pub(super) fn write_robots(
    out_dir: &Path,
    src_dir: &Path,
    config: &WebsiteConfig,
    base_url: Option<&str>,
) -> Result<()> {
    let path = out_dir.join(ROBOTS_FILE);
    if !config.robots_enabled() {
        return Ok(());
    }

    let template_dir = src_dir.join(ROBOTS_TEMPLATE_DIR);
    let mut env = no_autoescape_env();
    let mut has_robots_template = false;

    if template_dir.is_dir() {
        for (name, source) in read_template_files(&template_dir)? {
            if name == ROBOTS_TEMPLATE_FILE {
                has_robots_template = true;
            }
            env.add_template_owned(name, source)
                .map_err(|error| anyhow!("robots template: {error}"))?;
        }
    }
    if !has_robots_template {
        env.add_template(ROBOTS_TEMPLATE_FILE, DEFAULT_ROBOTS_TEMPLATE)
            .map_err(|error| anyhow!("robots template: {error}"))?;
    }

    let template = env
        .get_template(ROBOTS_TEMPLATE_FILE)
        .map_err(|error| anyhow!("robots template: {error}"))?;
    let contents = template
        .render(RobotsTemplateContext {
            config,
            sitemap_url: base_url.map(|url| absolute_site_url(url, "sitemap.xml")),
        })
        .map_err(|error| anyhow!("robots template: {error}"))?;
    fs::write(&path, contents).with_context(|| format!("failed to write {}", path.display()))
}

pub(super) fn read_template_files(dir: &Path) -> Result<Vec<(String, String)>> {
    let mut paths = Vec::new();
    collect_template_files(dir, dir, &mut paths)?;
    paths.sort();

    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let rel = relative_or_self(dir, &path);
        let name = slash_path(rel);
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        files.push((name, contents));
    }
    Ok(files)
}

fn collect_template_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    collect_files_by(
        root,
        dir,
        out,
        |rel, _| !has_calepin_component(rel),
        |rel, _| !has_calepin_component(rel),
    )
}

fn has_calepin_component(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str().to_str() == Some(".calepin"))
}
