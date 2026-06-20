use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::Serialize;

use crate::utils::template::no_autoescape_env;

use super::paths::slash_path;
use super::util::{absolute_site_url, xml_escape};
use super::{
    WebsiteConfig, DEFAULT_ROBOTS_TEMPLATE, ROBOTS_FILE, ROBOTS_TEMPLATE_DIR, ROBOTS_TEMPLATE_FILE,
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
        xml.push_str(&xml_escape(&absolute_site_url(base_url, href)));
        xml.push_str("</loc></url>\n");
    }
    xml.push_str("</urlset>\n");

    fs::write(&path, xml).with_context(|| format!("failed to write {}", path.display()))
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
        let rel = path.strip_prefix(dir).unwrap_or(&path);
        let name = slash_path(rel);
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        files.push((name, contents));
    }
    Ok(files)
}

fn collect_template_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            collect_template_files(root, &path, out)?;
        } else if path.is_file() {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            if rel
                .components()
                .any(|component| component.as_os_str().to_str() == Some(".calepin"))
            {
                continue;
            }
            out.push(path);
        }
    }
    Ok(())
}
