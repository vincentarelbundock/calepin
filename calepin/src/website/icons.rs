use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

use crate::utils::html::escape as html_escape;
use crate::utils::http::timeout_agent;

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
    /// an error: missing local or remote icons must not fail the build.
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
        let path = match absolute.canonicalize() {
            Ok(path) => path,
            Err(error) => {
                cwarn!("failed to read local icon `{spec}`: {error}");
                self.unavailable.insert(spec.to_string());
                return Ok(None);
            }
        };
        let src_dir = self.src_dir.canonicalize().with_context(|| {
            format!(
                "failed to resolve source directory {}",
                self.src_dir.display()
            )
        })?;
        path.strip_prefix(&src_dir).with_context(|| {
            format!("local icon `{spec}` must stay inside the source directory")
        })?;
        let svg = match fs::read_to_string(&path) {
            Ok(svg) => svg,
            Err(error) => {
                cwarn!("failed to read local icon {}: {error}", path.display());
                self.unavailable.insert(spec.to_string());
                return Ok(None);
            }
        };
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
    if !lower.starts_with("<svg") {
        return Err(anyhow!("icon `{spec}` is not a safe inline SVG"));
    }
    validate_svg_markup(svg, spec)?;
    Ok(svg.to_string())
}

fn validate_svg_markup(svg: &str, spec: &str) -> Result<()> {
    let mut index = 0;
    let mut saw_svg_open = false;
    let mut saw_svg_close = false;
    while let Some(offset) = svg[index..].find('<') {
        let start = index + offset;
        let end = find_tag_end(svg, start)
            .ok_or_else(|| anyhow!("icon `{spec}` is not a safe inline SVG"))?;
        let tag = svg[start + 1..end].trim();
        validate_svg_tag(tag, spec, &mut saw_svg_open, &mut saw_svg_close)?;
        index = end + 1;
    }
    if saw_svg_open && saw_svg_close {
        Ok(())
    } else {
        Err(anyhow!("icon `{spec}` is not a safe inline SVG"))
    }
}

fn find_tag_end(svg: &str, start: usize) -> Option<usize> {
    let mut quote = None;
    for (offset, byte) in svg[start + 1..].bytes().enumerate() {
        match (quote, byte) {
            (Some(current), next) if next == current => quote = None,
            (None, b'\'' | b'"') => quote = Some(byte),
            (None, b'>') => return Some(start + 1 + offset),
            _ => {}
        }
    }
    None
}

fn validate_svg_tag(
    tag: &str,
    spec: &str,
    saw_svg_open: &mut bool,
    saw_svg_close: &mut bool,
) -> Result<()> {
    if tag.starts_with("!--") && tag.ends_with("--") {
        return Ok(());
    }
    if tag.starts_with('!') || tag.starts_with('?') {
        return Err(anyhow!("icon `{spec}` is not a safe inline SVG"));
    }
    let closing = tag.strip_prefix('/').map(str::trim_start);
    let tag_body = closing.unwrap_or(tag);
    let (name, index) = parse_svg_name(tag_body, 0)
        .ok_or_else(|| anyhow!("icon `{spec}` is not a safe inline SVG"))?;
    if !allowed_svg_tag(&name) {
        return Err(anyhow!("icon `{spec}` is not a safe inline SVG"));
    }
    if let Some(closing) = closing {
        let rest = closing[index..].trim();
        if !rest.is_empty() {
            return Err(anyhow!("icon `{spec}` is not a safe inline SVG"));
        }
        if name == "svg" {
            *saw_svg_close = true;
        }
        return Ok(());
    }
    if name == "svg" {
        *saw_svg_open = true;
    }
    validate_svg_attributes(tag, index, spec)
}

fn parse_svg_name(source: &str, start: usize) -> Option<(String, usize)> {
    let mut end = start;
    for byte in source[start..].bytes() {
        if is_svg_name_byte(byte) {
            end += 1;
        } else {
            break;
        }
    }
    (end > start).then(|| (source[start..end].to_ascii_lowercase(), end))
}

fn is_svg_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'-' | b'_')
}

fn validate_svg_attributes(tag: &str, mut index: usize, spec: &str) -> Result<()> {
    let bytes = tag.as_bytes();
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() {
            return Ok(());
        }
        if bytes[index] == b'/' {
            if tag[index + 1..].trim().is_empty() {
                return Ok(());
            }
            return Err(anyhow!("icon `{spec}` is not a safe inline SVG"));
        }
        let (name, next) = parse_svg_name(tag, index)
            .ok_or_else(|| anyhow!("icon `{spec}` is not a safe inline SVG"))?;
        if name.starts_with("on") || !allowed_svg_attribute(&name) {
            return Err(anyhow!("icon `{spec}` is not a safe inline SVG"));
        }
        index = next;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] != b'=' {
            return Err(anyhow!("icon `{spec}` is not a safe inline SVG"));
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let (value, next) = parse_svg_attribute_value(tag, index, spec)?;
        validate_svg_attribute_value(&name, value, spec)?;
        index = next;
    }
    Ok(())
}

fn parse_svg_attribute_value<'a>(
    tag: &'a str,
    start: usize,
    spec: &str,
) -> Result<(&'a str, usize)> {
    let bytes = tag.as_bytes();
    if start >= bytes.len() {
        return Err(anyhow!("icon `{spec}` is not a safe inline SVG"));
    }
    match bytes[start] {
        b'\'' | b'"' => {
            let quote = bytes[start];
            let rest = &tag[start + 1..];
            let end = rest
                .bytes()
                .position(|byte| byte == quote)
                .ok_or_else(|| anyhow!("icon `{spec}` is not a safe inline SVG"))?;
            Ok((&rest[..end], start + 1 + end + 1))
        }
        _ => {
            let len = tag[start..]
                .bytes()
                .take_while(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b'/' | b'>'))
                .count();
            if len == 0 {
                return Err(anyhow!("icon `{spec}` is not a safe inline SVG"));
            }
            Ok((&tag[start..start + len], start + len))
        }
    }
}

fn allowed_svg_tag(name: &str) -> bool {
    matches!(
        name,
        "svg"
            | "g"
            | "defs"
            | "path"
            | "circle"
            | "ellipse"
            | "line"
            | "polyline"
            | "polygon"
            | "rect"
            | "clippath"
            | "mask"
            | "lineargradient"
            | "radialgradient"
            | "stop"
            | "title"
            | "desc"
            | "symbol"
            | "use"
    )
}

fn allowed_svg_attribute(name: &str) -> bool {
    name.starts_with("aria-")
        || name.starts_with("data-")
        || matches!(
            name,
            "aria-hidden"
                | "class"
                | "clip-path"
                | "clip-rule"
                | "color"
                | "cx"
                | "cy"
                | "d"
                | "fill"
                | "fill-opacity"
                | "fill-rule"
                | "focusable"
                | "fx"
                | "fy"
                | "gradienttransform"
                | "gradientunits"
                | "height"
                | "href"
                | "id"
                | "mask"
                | "offset"
                | "opacity"
                | "pathlength"
                | "points"
                | "preserveaspectratio"
                | "r"
                | "role"
                | "rx"
                | "ry"
                | "spreadmethod"
                | "stop-color"
                | "stop-opacity"
                | "stroke"
                | "stroke-dasharray"
                | "stroke-dashoffset"
                | "stroke-linecap"
                | "stroke-linejoin"
                | "stroke-miterlimit"
                | "stroke-opacity"
                | "stroke-width"
                | "style"
                | "transform"
                | "version"
                | "viewbox"
                | "width"
                | "x"
                | "x1"
                | "x2"
                | "xlink:href"
                | "xml:space"
                | "xmlns"
                | "xmlns:xlink"
                | "y"
                | "y1"
                | "y2"
        )
}

fn validate_svg_attribute_value(name: &str, value: &str, spec: &str) -> Result<()> {
    let decoded = decode_character_references(value);
    let trimmed = decoded.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("javascript:") || contains_unsafe_url_function(&lower) {
        return Err(anyhow!("icon `{spec}` is not a safe inline SVG"));
    }
    if matches!(name, "href" | "xlink:href") && !trimmed.starts_with('#') {
        return Err(anyhow!("icon `{spec}` is not a safe inline SVG"));
    }
    if name == "style"
        && (lower.contains("@import")
            || lower.contains("expression(")
            || lower.contains("-moz-binding"))
    {
        return Err(anyhow!("icon `{spec}` is not a safe inline SVG"));
    }
    Ok(())
}

fn contains_unsafe_url_function(lower: &str) -> bool {
    let mut rest = lower;
    while let Some(offset) = rest.find("url(") {
        let after_start = &rest[offset + "url(".len()..];
        let Some(end) = after_start.find(')') else {
            return true;
        };
        let target = after_start[..end].trim().trim_matches(['"', '\'']).trim();
        if !target.starts_with('#') {
            return true;
        }
        rest = &after_start[end + 1..];
    }
    false
}

fn decode_character_references(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after_amp = &rest[start + 1..];
        let Some(end) = after_amp.find(';') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let entity = &after_amp[..end];
        if let Some(decoded) = decode_character_reference(entity) {
            out.push(decoded);
        } else {
            out.push('&');
            out.push_str(entity);
            out.push(';');
        }
        rest = &after_amp[end + 1..];
    }
    out.push_str(rest);
    out
}

fn decode_character_reference(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "apos" => Some('\''),
        "gt" => Some('>'),
        "lt" => Some('<'),
        "quot" => Some('"'),
        _ => entity
            .strip_prefix("#x")
            .or_else(|| entity.strip_prefix("#X"))
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .or_else(|| {
                entity
                    .strip_prefix('#')
                    .and_then(|decimal| decimal.parse::<u32>().ok())
            })
            .and_then(char::from_u32),
    }
}

pub(super) fn nav_label_html(label: &str, icon_cache: &mut IconCache) -> Result<String> {
    let mut html = String::new();
    for token in nav_label_tokens(label) {
        match token {
            NavLabelToken::Text(text) => html.push_str(&html_escape(text)),
            NavLabelToken::Icon(icon) => push_nav_icon(&mut html, icon, icon_cache)?,
        }
    }
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
    for token in nav_label_tokens(label) {
        if let NavLabelToken::Text(text) = token {
            out.push_str(text);
        }
    }
    out
}

enum NavLabelToken<'a> {
    Text(&'a str),
    Icon(&'a str),
}

struct NavLabelTokens<'a> {
    rest: &'a str,
}

fn nav_label_tokens(label: &str) -> NavLabelTokens<'_> {
    NavLabelTokens { rest: label }
}

impl<'a> Iterator for NavLabelTokens<'a> {
    type Item = NavLabelToken<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.rest.is_empty() {
            return None;
        }
        let Some(start) = self.rest.find("{icon:") else {
            let text = self.rest;
            self.rest = "";
            return Some(NavLabelToken::Text(text));
        };
        if start > 0 {
            let text = &self.rest[..start];
            self.rest = &self.rest[start..];
            return Some(NavLabelToken::Text(text));
        }
        let after_start = &self.rest["{icon:".len()..];
        let Some(end) = after_start.find('}') else {
            let text = self.rest;
            self.rest = "";
            return Some(NavLabelToken::Text(text));
        };
        let icon = after_start[..end].trim();
        self.rest = &after_start[end + 1..];
        Some(NavLabelToken::Icon(icon))
    }
}
