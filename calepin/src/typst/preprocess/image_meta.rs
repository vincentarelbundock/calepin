use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;
use xxhash_rust::xxh3::xxh3_64;

use crate::typst::io::write_if_changed;
use crate::typst::model::LayoutPaths;
use crate::typst::paths::slash_path;

const IMAGE_META_FILE: &str = "image-meta.json";

#[derive(Debug, Clone, Serialize)]
pub(super) struct ImageMetaDocument {
    schema: u8,
    images: BTreeMap<String, ImageMetaEntry>,
}

#[derive(Debug, Clone, Serialize)]
struct ImageMetaEntry {
    path: String,
    xxh3: String,
    bytes: u64,
    width: u32,
    height: u32,
}

impl ImageMetaDocument {
    pub(super) fn signature(&self) -> Result<u64> {
        let bytes = serde_json::to_vec(self)?;
        Ok(xxh3_64(&bytes))
    }
}

pub(crate) fn image_meta_relative_path(layout: &LayoutPaths) -> PathBuf {
    layout.artifact_relative_path(IMAGE_META_FILE)
}

pub(super) fn write_image_meta(layout: &LayoutPaths) -> Result<ImageMetaDocument> {
    let document = collect_image_meta(layout)?;
    let path = layout.artifact_path(IMAGE_META_FILE);
    write_if_changed(&path, serde_json::to_string_pretty(&document)?)?;
    Ok(document)
}

fn collect_image_meta(layout: &LayoutPaths) -> Result<ImageMetaDocument> {
    let mut keys_by_path = collect_project_image_keys(layout)?;
    collect_literal_image_keys(layout, &mut keys_by_path)?;

    let mut images = BTreeMap::new();
    for (path, keys) in keys_by_path {
        let bytes =
            fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let Some((width, height)) = dimensions(&path, &bytes) else {
            continue;
        };
        let rel = path
            .strip_prefix(&layout.root)
            .map(slash_path)
            .unwrap_or_else(|_| path.display().to_string());
        let entry = ImageMetaEntry {
            path: rel,
            xxh3: format!("{:016x}", xxh3_64(&bytes)),
            bytes: bytes.len() as u64,
            width,
            height,
        };
        for key in keys {
            images.insert(key, entry.clone());
        }
    }

    Ok(ImageMetaDocument { schema: 1, images })
}

fn collect_project_image_keys(layout: &LayoutPaths) -> Result<BTreeMap<PathBuf, BTreeSet<String>>> {
    let mut out = BTreeMap::new();
    let mut queue = VecDeque::from([layout.root.clone()]);

    while let Some(dir) = queue.pop_front() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        for entry in entries {
            let entry = entry.with_context(|| format!("failed to read {}", dir.display()))?;
            let path = entry.path();
            let file_type = entry
                .file_type()
                .with_context(|| format!("failed to stat {}", path.display()))?;
            if file_type.is_dir() {
                if should_skip_dir(layout, &path) {
                    continue;
                }
                queue.push_back(path);
            } else if file_type.is_file() && is_supported_image_path(&path) {
                if let Some(rel) = project_relative(layout, &path) {
                    let keys = out.entry(path).or_insert_with(BTreeSet::new);
                    keys.insert(rel.clone());
                    keys.insert(format!("/{rel}"));
                }
            }
        }
    }

    Ok(out)
}

fn collect_literal_image_keys(
    layout: &LayoutPaths,
    keys_by_path: &mut BTreeMap<PathBuf, BTreeSet<String>>,
) -> Result<()> {
    let source = fs::read_to_string(&layout.input)
        .with_context(|| format!("failed to read {}", layout.input.display()))?;
    for literal in string_literals(&source) {
        if !is_supported_image_reference(&literal) || is_external_reference(&literal) {
            continue;
        }
        let Some(path) = resolve_local_reference(layout, &literal) else {
            continue;
        };
        if !path.is_file() {
            continue;
        }
        let keys = keys_by_path.entry(path).or_insert_with(BTreeSet::new);
        keys.insert(literal);
    }
    Ok(())
}

fn should_skip_dir(layout: &LayoutPaths, path: &Path) -> bool {
    if layout
        .artifact_dir
        .parent()
        .is_some_and(|generated_root| path.starts_with(generated_root))
    {
        return true;
    }
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == ".git" || name == ".calepin")
}

fn project_relative(layout: &LayoutPaths, path: &Path) -> Option<String> {
    path.strip_prefix(&layout.root).ok().map(slash_path)
}

fn resolve_local_reference(layout: &LayoutPaths, reference: &str) -> Option<PathBuf> {
    let clean = reference.split(['?', '#']).next().unwrap_or(reference);
    let candidate = if let Some(root_relative) = clean.strip_prefix('/') {
        layout.root.join(root_relative)
    } else {
        layout.work_dir.join(clean)
    };
    let canonical = fs::canonicalize(candidate).ok()?;
    canonical.starts_with(&layout.root).then_some(canonical)
}

fn string_literals(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = source.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        if ch != '"' {
            continue;
        }
        let mut literal = String::new();
        let mut escaped = false;
        for (_, ch) in chars.by_ref() {
            if escaped {
                literal.push(ch);
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '"' {
                break;
            }
            literal.push(ch);
        }
        out.push(literal);
    }
    out
}

fn is_external_reference(reference: &str) -> bool {
    reference.starts_with("http://")
        || reference.starts_with("https://")
        || reference.starts_with("data:")
}

fn is_supported_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(is_supported_image_extension)
}

fn is_supported_image_reference(reference: &str) -> bool {
    let clean = reference.split(['?', '#']).next().unwrap_or(reference);
    Path::new(clean)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(is_supported_image_extension)
}

fn is_supported_image_extension(extension: &str) -> bool {
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "gif" | "jpeg" | "jpg" | "png" | "svg" | "webp"
    )
}

fn dimensions(path: &Path, bytes: &[u8]) -> Option<(u32, u32)> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => png_dimensions(bytes),
        Some("jpg" | "jpeg") => jpeg_dimensions(bytes),
        Some("gif") => gif_dimensions(bytes),
        Some("webp") => webp_dimensions(bytes),
        Some("svg") => svg_dimensions(bytes),
        _ => None,
    }
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    Some((
        u32::from_be_bytes(bytes[16..20].try_into().ok()?),
        u32::from_be_bytes(bytes[20..24].try_into().ok()?),
    ))
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return None;
    }
    let mut index = 2;
    while index + 9 < bytes.len() {
        while index < bytes.len() && bytes[index] != 0xff {
            index += 1;
        }
        while index < bytes.len() && bytes[index] == 0xff {
            index += 1;
        }
        if index >= bytes.len() {
            return None;
        }
        let marker = bytes[index];
        index += 1;
        if marker == 0xd9 || marker == 0xda {
            return None;
        }
        if index + 2 > bytes.len() {
            return None;
        }
        let length = u16::from_be_bytes(bytes[index..index + 2].try_into().ok()?) as usize;
        if length < 2 || index + length > bytes.len() {
            return None;
        }
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) {
            if length < 7 {
                return None;
            }
            let height = u16::from_be_bytes(bytes[index + 3..index + 5].try_into().ok()?) as u32;
            let width = u16::from_be_bytes(bytes[index + 5..index + 7].try_into().ok()?) as u32;
            return Some((width, height));
        }
        index += length;
    }
    None
}

fn gif_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 10 || !bytes.starts_with(b"GIF8") {
        return None;
    }
    Some((
        u16::from_le_bytes(bytes[6..8].try_into().ok()?) as u32,
        u16::from_le_bytes(bytes[8..10].try_into().ok()?) as u32,
    ))
}

fn webp_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 30 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return None;
    }
    match &bytes[12..16] {
        b"VP8X" => Some((
            1 + read_u24_le(&bytes[24..27])?,
            1 + read_u24_le(&bytes[27..30])?,
        )),
        b"VP8L" => {
            if bytes.len() < 25 || bytes[20] != 0x2f {
                return None;
            }
            let b0 = bytes[21] as u32;
            let b1 = bytes[22] as u32;
            let b2 = bytes[23] as u32;
            let b3 = bytes[24] as u32;
            Some((
                1 + (((b1 & 0x3f) << 8) | b0),
                1 + (((b3 & 0x0f) << 10) | (b2 << 2) | ((b1 & 0xc0) >> 6)),
            ))
        }
        b"VP8 " => {
            if bytes.len() < 30 {
                return None;
            }
            Some((
                u16::from_le_bytes(bytes[26..28].try_into().ok()?) as u32 & 0x3fff,
                u16::from_le_bytes(bytes[28..30].try_into().ok()?) as u32 & 0x3fff,
            ))
        }
        _ => None,
    }
}

fn read_u24_le(bytes: &[u8]) -> Option<u32> {
    if bytes.len() < 3 {
        return None;
    }
    Some((bytes[0] as u32) | ((bytes[1] as u32) << 8) | ((bytes[2] as u32) << 16))
}

fn svg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let text = std::str::from_utf8(bytes).ok()?;
    let tag = text.split('>').next()?;
    let width = svg_number_attr(tag, "width");
    let height = svg_number_attr(tag, "height");
    match (width, height) {
        (Some(width), Some(height)) => Some((width.round() as u32, height.round() as u32)),
        _ => svg_viewbox_dimensions(tag),
    }
}

fn svg_number_attr(tag: &str, name: &str) -> Option<f64> {
    let raw = svg_attr(tag, name)?;
    let number = raw
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect::<String>();
    number.parse().ok()
}

fn svg_viewbox_dimensions(tag: &str) -> Option<(u32, u32)> {
    let raw = svg_attr(tag, "viewBox").or_else(|| svg_attr(tag, "viewbox"))?;
    let values = raw
        .split(|ch: char| ch.is_ascii_whitespace() || ch == ',')
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<f64>().ok())
        .collect::<Vec<_>>();
    if values.len() != 4 {
        return None;
    }
    Some((values[2].round() as u32, values[3].round() as u32))
}

fn svg_attr(tag: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=");
    let mut search_start = 0;
    while let Some(offset) = tag[search_start..].find(&needle) {
        let name_start = search_start + offset;
        if !is_svg_attr_name_boundary(tag, name_start) {
            search_start = name_start + needle.len();
            continue;
        }
        return svg_quoted_attr_value(tag, name_start + needle.len());
    }
    None
}

fn is_svg_attr_name_boundary(tag: &str, name_start: usize) -> bool {
    tag[..name_start]
        .chars()
        .next_back()
        .is_some_and(|ch| ch == '<' || ch.is_ascii_whitespace())
}

fn svg_quoted_attr_value(tag: &str, start: usize) -> Option<String> {
    let quote = tag[start..].chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let value_start = start + quote.len_utf8();
    let value_end = tag[value_start..].find(quote)? + value_start;
    Some(tag[value_start..value_end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typst::testfixtures;

    #[test]
    fn skips_generated_artifact_root() {
        let dir = tempfile::tempdir().unwrap();
        let mut layout = testfixtures::layout(dir.path());
        layout.artifact_dir = layout.root.join("_calepin/paper");

        assert!(should_skip_dir(&layout, &layout.root.join("_calepin")));
        assert!(should_skip_dir(
            &layout,
            &layout.root.join("_calepin/paper")
        ));
        assert!(!should_skip_dir(&layout, &layout.root.join("assets")));
    }

    #[test]
    fn svg_dimensions_do_not_match_suffix_attributes() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" stroke-width="4" width="120" height="80"></svg>"#;

        assert_eq!(svg_dimensions(svg), Some((120, 80)));
    }
}
