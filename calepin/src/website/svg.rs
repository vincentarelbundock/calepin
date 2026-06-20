use std::str;

use anyhow::{anyhow, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

pub(super) fn sanitize_icon_svg(svg: &str, spec: &str) -> Result<String> {
    let svg = svg.trim();
    if !svg.to_ascii_lowercase().starts_with("<svg") {
        return Err(unsafe_icon_error(spec));
    }
    validate_svg_markup(svg, spec)?;
    Ok(svg.to_string())
}

fn validate_svg_markup(svg: &str, spec: &str) -> Result<()> {
    let mut reader = Reader::from_str(svg);
    reader.config_mut().check_comments = true;
    reader.config_mut().check_end_names = true;

    let mut depth = 0usize;
    let mut root_seen = false;
    let mut root_closed = false;
    loop {
        match reader.read_event().map_err(|_| unsafe_icon_error(spec))? {
            Event::Start(tag) => {
                let name = tag_name(tag.name().as_ref(), spec)?;
                validate_root_state(&name, depth, root_seen, root_closed, spec)?;
                validate_tag(&tag, &name, spec)?;
                if depth == 0 {
                    root_seen = true;
                }
                depth += 1;
            }
            Event::Empty(tag) => {
                let name = tag_name(tag.name().as_ref(), spec)?;
                validate_root_state(&name, depth, root_seen, root_closed, spec)?;
                validate_tag(&tag, &name, spec)?;
                if depth == 0 {
                    root_seen = true;
                    root_closed = true;
                }
            }
            Event::End(tag) => {
                let name = tag_name(tag.name().as_ref(), spec)?;
                if !allowed_svg_tag(&name) || depth == 0 {
                    return Err(unsafe_icon_error(spec));
                }
                if depth == 1 {
                    if name != "svg" {
                        return Err(unsafe_icon_error(spec));
                    }
                    root_closed = true;
                }
                depth -= 1;
            }
            Event::Text(text) => {
                if depth == 0 && !text.decode().is_ok_and(|text| text.trim().is_empty()) {
                    return Err(unsafe_icon_error(spec));
                }
            }
            Event::Comment(_) => {}
            Event::CData(_) => {
                if depth == 0 {
                    return Err(unsafe_icon_error(spec));
                }
            }
            Event::Eof => {
                return if root_seen && root_closed && depth == 0 {
                    Ok(())
                } else {
                    Err(unsafe_icon_error(spec))
                };
            }
            _ => return Err(unsafe_icon_error(spec)),
        }
    }
}

fn validate_root_state(
    name: &str,
    depth: usize,
    root_seen: bool,
    root_closed: bool,
    spec: &str,
) -> Result<()> {
    if depth == 0 && (root_seen || root_closed || name != "svg") {
        return Err(unsafe_icon_error(spec));
    }
    Ok(())
}

fn validate_tag(tag: &BytesStart<'_>, name: &str, spec: &str) -> Result<()> {
    if !allowed_svg_tag(name) {
        return Err(unsafe_icon_error(spec));
    }
    for attr in tag.attributes() {
        let attr = attr.map_err(|_| unsafe_icon_error(spec))?;
        let name = tag_name(attr.key.as_ref(), spec)?;
        if name.starts_with("on") || !allowed_svg_attribute(&name) {
            return Err(unsafe_icon_error(spec));
        }
        let value = attr
            .decode_and_unescape_value(tag.decoder())
            .map_err(|_| unsafe_icon_error(spec))?;
        validate_svg_attribute_value(&name, value.trim(), spec)?;
    }
    Ok(())
}

fn tag_name(raw: &[u8], spec: &str) -> Result<String> {
    str::from_utf8(raw)
        .map(str::to_ascii_lowercase)
        .map_err(|_| unsafe_icon_error(spec))
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
    let lower = value.to_ascii_lowercase();
    if lower.contains("javascript:") || contains_unsafe_url_function(&lower) {
        return Err(unsafe_icon_error(spec));
    }
    if matches!(name, "href" | "xlink:href") && !value.starts_with('#') {
        return Err(unsafe_icon_error(spec));
    }
    if name == "style"
        && (lower.contains("@import")
            || lower.contains("expression(")
            || lower.contains("-moz-binding"))
    {
        return Err(unsafe_icon_error(spec));
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

fn unsafe_icon_error(spec: &str) -> anyhow::Error {
    anyhow!("icon `{spec}` is not a safe inline SVG")
}
