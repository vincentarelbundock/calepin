use anyhow::{Context, Result};
use std::path::{Component, Path, PathBuf};

use crate::utils::static_files::path_stays_under_root;

pub(super) fn inline_html_images(
    html: &str,
    root: &Path,
    base_dir: &Path,
    page_dir: Option<&Path>,
) -> Result<String> {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;

    while let Some(tag_start) = rest.find("<img") {
        out.push_str(&rest[..tag_start]);
        let tag_and_after = &rest[tag_start..];
        let Some(tag_end) = tag_and_after.find('>') else {
            out.push_str(tag_and_after);
            return Ok(out);
        };
        let tag_end = tag_end + 1;
        out.push_str(&inline_img_tag(
            &tag_and_after[..tag_end],
            root,
            base_dir,
            page_dir,
        )?);
        rest = &tag_and_after[tag_end..];
    }

    out.push_str(rest);
    Ok(out)
}

fn inline_img_tag(
    tag: &str,
    root: &Path,
    base_dir: &Path,
    page_dir: Option<&Path>,
) -> Result<String> {
    let Some((value_start, value_end, src)) = find_src_attr(tag) else {
        return Ok(tag.to_string());
    };
    if !is_inlineable_src(src) {
        return Ok(tag.to_string());
    }

    let Some(path) = resolve_html_asset_path(src, root, base_dir)
        .filter(|path| path.exists())
        // Guard against a root-relative or relative reference that climbs
        // outside the project root (e.g. `src="/../../etc/passwd"`), the same
        // containment check every other path in the site builder goes through.
        .filter(|path| path_stays_under_root(root, path))
        .or_else(|| page_relative_asset_path(src, root, page_dir?))
    else {
        return Ok(tag.to_string());
    };

    let data =
        std::fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mime = html_asset_mime(&path);
    let data_uri = format!("data:{mime};base64,{}", base64_encode(&data));

    let mut rewritten = String::with_capacity(tag.len() + data_uri.len());
    rewritten.push_str(&tag[..value_start]);
    rewritten.push_str(&data_uri);
    rewritten.push_str(&tag[value_end..]);
    Ok(rewritten)
}

/// Byte offset of the first character in `s` matching `predicate`, or `s.len()`
/// if none matches. Always a valid `str` char boundary, unlike advancing a
/// byte index one byte at a time and slicing at it: a multi-byte UTF-8
/// continuation byte is never itself a char boundary, so that pattern panics
/// the moment such a byte is scanned past. Every scan in this module goes
/// through this helper (or `str::find`, which has the same guarantee) instead.
fn find_char_boundary(s: &str, predicate: impl Fn(char) -> bool) -> usize {
    s.find(predicate).unwrap_or(s.len())
}

fn find_src_attr(tag: &str) -> Option<(usize, usize, &str)> {
    // `str::match_indices` only ever yields byte offsets that fall on char
    // boundaries (it walks `tag` character by character internally), so `i`
    // below is always safe to slice at, even when `tag` holds non-ASCII text
    // before, inside, or after the `src` attribute.
    for (i, _) in tag.match_indices("src") {
        let before = tag[..i].chars().next_back().unwrap_or(' ');
        if before.is_alphanumeric() || before == '-' || before == '_' {
            continue;
        }

        let after_name = &tag[i + 3..];
        let after_ws = after_name.trim_start();
        let Some(after_eq) = after_ws.strip_prefix('=') else {
            continue;
        };
        let value_region = after_eq.trim_start();
        if value_region.is_empty() {
            return None;
        }
        let value_offset = tag.len() - value_region.len();

        let first = value_region.chars().next().expect("checked non-empty");
        if first == '"' || first == '\'' {
            let quote = first;
            let rest = &value_region[quote.len_utf8()..];
            let end_in_rest = rest.find(quote)?;
            let value_start = value_offset + quote.len_utf8();
            let value_end = value_start + end_in_rest;
            return Some((value_start, value_end, &tag[value_start..value_end]));
        }

        let value_start = value_offset;
        let end_in_value = find_char_boundary(value_region, |ch| ch.is_whitespace() || ch == '>');
        let value_end = value_start + end_in_value;
        return Some((value_start, value_end, &tag[value_start..value_end]));
    }
    None
}

fn is_inlineable_src(src: &str) -> bool {
    !(src.starts_with("data:")
        || src.starts_with("http://")
        || src.starts_with("https://")
        || src.starts_with('#'))
}

fn resolve_html_asset_path(src: &str, root: &Path, base_dir: &Path) -> Option<PathBuf> {
    let path = src.split(['?', '#']).next().unwrap_or(src);
    if path.is_empty() {
        return None;
    }
    if let Some(root_relative) = path.strip_prefix('/') {
        return Some(root.join(root_relative));
    }
    let path = Path::new(path);
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    })
}

/// Resolve an asset reference the website runtime rewrote from a root-relative
/// path into a page-relative one. Those URLs are relative to the page's place
/// in the *output* tree, where generated figures never exist, so they are
/// re-anchored on the project root before the file is read.
fn page_relative_asset_path(src: &str, root: &Path, page_dir: &Path) -> Option<PathBuf> {
    let src = src.split(['?', '#']).next().unwrap_or(src);
    let path = Path::new(src);
    if src.is_empty() || src.starts_with('/') || path.is_absolute() {
        return None;
    }
    let mut relative = PathBuf::new();
    for component in page_dir.join(path).components() {
        match component {
            Component::CurDir => {}
            // A reference that climbs above the site root is not addressable
            // from the project root either.
            Component::ParentDir => {
                if !relative.pop() {
                    return None;
                }
            }
            Component::Normal(part) => relative.push(part),
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    let resolved = root.join(relative);
    resolved.is_file().then_some(resolved)
}

fn html_asset_mime(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}
