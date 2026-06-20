use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

use crate::utils::html::escape as html_escape;
use crate::utils::http::timeout_agent;

use super::paths::normalize_path;

const DEFAULT_ICON_PREFIX: &str = "lucide";
const ICON_DOWNLOAD_TIMEOUT_SECS: u64 = 5;

pub(super) struct IconCache {
    src_dir: PathBuf,
    cache_dir: PathBuf,
    agent: ureq::Agent,
    /// Icons that already failed this build; avoids repeating slow download
    /// attempts (and duplicate warnings) for every nav item that uses them.
    unavailable: BTreeSet<String>,
}

impl IconCache {
    pub(super) fn new(src_dir: &Path, cache_subdir: &str) -> Self {
        let agent = timeout_agent(Duration::from_secs(ICON_DOWNLOAD_TIMEOUT_SECS));
        Self {
            src_dir: src_dir.to_path_buf(),
            cache_dir: src_dir.join(cache_subdir),
            agent,
            unavailable: BTreeSet::new(),
        }
    }

    /// Returns the icon's inline SVG, or `None` (after a warning) when it
    /// cannot be fetched or is unsafe to inline. Only a malformed icon spec is
    /// an error: a missing network must not fail the whole site build.
    fn resolve(&mut self, spec: Option<&str>) -> Result<Option<String>> {
        let Some(spec) = spec else {
            return Ok(None);
        };
        if self.unavailable.contains(spec) {
            return Ok(None);
        }
        if icon_spec_is_local_path(spec) {
            return self.resolve_local(spec);
        }
        let icon = parse_icon_spec(spec)?;
        let path = self
            .cache_dir
            .join(&icon.prefix)
            .join(format!("{}.svg", icon.name));
        if path.is_file() {
            let svg = fs::read_to_string(&path)
                .with_context(|| format!("failed to read cached icon {}", path.display()))?;
            return Ok(self.sanitized_or_warn(&svg, spec));
        }

        fs::create_dir_all(path.parent().unwrap())
            .with_context(|| format!("failed to create icon cache {}", self.cache_dir.display()))?;
        let url = format!(
            "https://api.iconify.design/{}/{}.svg",
            icon.prefix, icon.name
        );
        let svg = match self.agent.get(&url).call() {
            Ok(response) => match response.into_string() {
                Ok(svg) => svg,
                Err(error) => {
                    cwarn!("failed to read downloaded icon `{spec}`: {error}");
                    self.unavailable.insert(spec.to_string());
                    return Ok(None);
                }
            },
            Err(error) => {
                cwarn!("failed to download icon `{spec}` from {url}: {error}");
                self.unavailable.insert(spec.to_string());
                return Ok(None);
            }
        };
        let Some(svg) = self.sanitized_or_warn(&svg, spec) else {
            return Ok(None);
        };
        fs::write(&path, &svg)
            .with_context(|| format!("failed to cache icon `{spec}` at {}", path.display()))?;
        Ok(Some(svg))
    }

    fn resolve_local(&mut self, spec: &str) -> Result<Option<String>> {
        let requested = Path::new(spec.trim());
        let absolute = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.src_dir.join(requested)
        };
        let normalized = normalize_path(&absolute);
        let src_dir = normalize_path(&self.src_dir);
        normalized.strip_prefix(&src_dir).with_context(|| {
            format!("local icon `{spec}` must stay inside the source directory")
        })?;
        let svg = fs::read_to_string(&normalized)
            .with_context(|| format!("failed to read local icon {}", normalized.display()))?;
        Ok(self.sanitized_or_warn(&svg, spec))
    }

    fn sanitized_or_warn(&mut self, svg: &str, spec: &str) -> Option<String> {
        match sanitize_icon_svg(svg, spec) {
            Ok(svg) => Some(svg),
            Err(error) => {
                cwarn!("{error}");
                self.unavailable.insert(spec.to_string());
                None
            }
        }
    }
}

fn icon_spec_is_local_path(spec: &str) -> bool {
    let spec = spec.trim();
    spec.ends_with(".svg") || spec.contains('/') || spec.contains('\\')
}

struct IconSpec {
    prefix: String,
    name: String,
}

fn parse_icon_spec(value: &str) -> Result<IconSpec> {
    let value = value.trim();
    let (prefix, name) = value
        .split_once(':')
        .map(|(prefix, name)| (prefix.trim(), name.trim()))
        .unwrap_or((DEFAULT_ICON_PREFIX, value));
    if !valid_icon_component(prefix) || !valid_icon_component(name) {
        return Err(anyhow!(
            "invalid icon `{value}`; use `name` or `prefix:name` with lowercase letters, digits, and hyphens"
        ));
    }
    Ok(IconSpec {
        prefix: prefix.to_string(),
        name: name.to_string(),
    })
}

fn valid_icon_component(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && !value.contains("--")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

pub(super) fn sanitize_icon_svg(svg: &str, spec: &str) -> Result<String> {
    let svg = svg.trim();
    let lower = svg.to_ascii_lowercase();
    if !lower.starts_with("<svg")
        || !lower.contains("</svg>")
        || lower.contains("<script")
        || lower.contains("<foreignobject")
        || lower.contains("javascript:")
        || contains_event_handler_attribute(&lower)
    {
        return Err(anyhow!("downloaded icon `{spec}` is not a safe inline SVG"));
    }
    Ok(svg.to_string())
}

/// Detects `on...=` event-handler attributes (`onload=`, `onclick=`, ...) in
/// lowercased SVG source. May reject rare benign content (e.g. text mentioning
/// `on x=`); for icons that trade-off is fine.
fn contains_event_handler_attribute(lower: &str) -> bool {
    lower.match_indices("on").any(|(index, _)| {
        let preceded_by_whitespace = lower[..index]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_whitespace());
        if !preceded_by_whitespace {
            return false;
        }
        let rest = &lower[index + "on".len()..];
        let name_len = rest.chars().take_while(char::is_ascii_alphanumeric).count();
        name_len > 0 && rest[name_len..].trim_start().starts_with('=')
    })
}

pub(super) fn nav_label_html(label: &str, icon_cache: &mut IconCache) -> Result<String> {
    let mut html = String::new();
    let mut rest = label;
    while let Some(start) = rest.find("{icon:") {
        html.push_str(&html_escape(&rest[..start]));
        let after_start = &rest[start + "{icon:".len()..];
        let Some(end) = after_start.find('}') else {
            html.push_str(&html_escape(&rest[start..]));
            return Ok(html);
        };
        let icon = after_start[..end].trim();
        push_nav_icon(&mut html, icon, icon_cache)?;
        rest = &after_start[end + 1..];
    }
    html.push_str(&html_escape(rest));
    Ok(html)
}

fn push_nav_icon(html: &mut String, icon: &str, icon_cache: &mut IconCache) -> Result<()> {
    if let Some(svg) = icon_cache.resolve(Some(icon))? {
        html.push_str(r#"<span class="calepin-nav-icon">"#);
        html.push_str(&svg);
        html.push_str("</span>");
    }
    Ok(())
}

pub(super) fn accessible_nav_label(label: &str, fallback: &str) -> String {
    let stripped = strip_icon_tokens(label).trim().to_string();
    if stripped.is_empty() {
        fallback.to_string()
    } else {
        stripped
    }
}

fn strip_icon_tokens(label: &str) -> String {
    let mut out = String::new();
    let mut rest = label;
    while let Some(start) = rest.find("{icon:") {
        out.push_str(&rest[..start]);
        let after_start = &rest[start + "{icon:".len()..];
        let Some(end) = after_start.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        rest = &after_start[end + 1..];
    }
    out.push_str(rest);
    out
}
