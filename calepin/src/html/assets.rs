use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub(super) fn inline_html_images(html: &str, root: &Path, base_dir: &Path) -> Result<String> {
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
        out.push_str(&inline_img_tag(&tag_and_after[..tag_end], root, base_dir)?);
        rest = &tag_and_after[tag_end..];
    }

    out.push_str(rest);
    Ok(out)
}

fn inline_img_tag(tag: &str, root: &Path, base_dir: &Path) -> Result<String> {
    let Some((value_start, value_end, src)) = find_src_attr(tag) else {
        return Ok(tag.to_string());
    };
    if !is_inlineable_src(src) {
        return Ok(tag.to_string());
    }

    let Some(path) = resolve_html_asset_path(src, root, base_dir) else {
        return Ok(tag.to_string());
    };
    if !path.exists() {
        return Ok(tag.to_string());
    }

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

fn find_src_attr(tag: &str) -> Option<(usize, usize, &str)> {
    let bytes = tag.as_bytes();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        if !tag[i..].starts_with("src") {
            i += 1;
            continue;
        }
        let before = if i == 0 { b' ' } else { bytes[i - 1] };
        if before.is_ascii_alphanumeric() || before == b'-' || before == b'_' {
            i += 3;
            continue;
        }
        let mut j = i + 3;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'=' {
            i += 3;
            continue;
        }
        j += 1;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() {
            return None;
        }
        let quote = bytes[j];
        if quote == b'"' || quote == b'\'' {
            let value_start = j + 1;
            let mut value_end = value_start;
            while value_end < bytes.len() && bytes[value_end] != quote {
                value_end += 1;
            }
            if value_end >= bytes.len() {
                return None;
            }
            return Some((value_start, value_end, &tag[value_start..value_end]));
        }
        let value_start = j;
        let mut value_end = value_start;
        while value_end < bytes.len()
            && !bytes[value_end].is_ascii_whitespace()
            && bytes[value_end] != b'>'
        {
            value_end += 1;
        }
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
