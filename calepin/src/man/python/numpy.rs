// ISC License
//
// Copyright (c) 2021, Timothee Mazzucotelli
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

//! NumPy-style docstring parser.
//!
//! Recognizes sections delimited by underline dashes:
//!
//! ```text
//! Parameters
//! ----------
//! x : int
//!     The input value.
//! y : str, optional
//!     Another input.
//!
//! Returns
//! -------
//! bool
//!     Whether it worked.
//! ```

use super::types::*;

/// Known section names mapped to their kind.
fn section_kind(name: &str) -> Option<SectionKind> {
    match name.to_lowercase().as_str() {
        "parameters" | "params" | "args" => Some(SectionKind::Parameters),
        "other parameters" | "other params" | "keyword arguments" => {
            Some(SectionKind::OtherParameters)
        }
        "returns" | "return" => Some(SectionKind::Returns),
        "yields" | "yield" => Some(SectionKind::Yields),
        "raises" | "raise" | "except" | "exceptions" => Some(SectionKind::Raises),
        "examples" | "example" => Some(SectionKind::Examples),
        "notes" | "note" => Some(SectionKind::Notes),
        "warnings" | "warns" | "warning" => Some(SectionKind::Warnings),
        "references" => Some(SectionKind::References),
        "attributes" | "attrs" => Some(SectionKind::Attributes),
        "deprecated" => Some(SectionKind::Deprecated),
        "see also" => Some(SectionKind::Notes),
        "methods" => Some(SectionKind::Attributes),
        _ => None,
    }
}

/// Parse a NumPy-style docstring into sections.
pub fn parse_numpy(docstring: &str) -> Vec<DocSection> {
    let lines: Vec<&str> = docstring.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    let mut sections = Vec::new();
    let mut i = 0;

    // Collect preamble text (before any section header)
    let mut preamble = Vec::new();
    while i < lines.len() {
        if is_numpy_section_start(&lines, i) {
            break;
        }
        preamble.push(lines[i]);
        i += 1;
    }

    let preamble_text = preamble.join("\n").trim().to_string();
    if !preamble_text.is_empty() {
        sections.push(DocSection {
            kind: SectionKind::Text,
            content: SectionContent::Text(preamble_text),
        });
    }

    // Parse sections
    while i < lines.len() {
        if is_numpy_section_start(&lines, i) {
            let name = lines[i].trim();
            i += 2; // Skip header + underline

            // Read section body until next section or end
            let (body_lines, next_i) = read_section_body(&lines, i);
            i = next_i;

            if let Some(kind) = section_kind(name) {
                let section = parse_section(kind, &body_lines);
                sections.push(section);
            } else {
                // Unknown section: treat as admonition
                let body = body_lines.join("\n").trim().to_string();
                sections.push(DocSection {
                    kind: SectionKind::Admonition,
                    content: SectionContent::Admonition {
                        title: name.to_string(),
                        description: body,
                    },
                });
            }
        } else {
            i += 1;
        }
    }

    sections
}

/// Check if line `i` starts a NumPy section (name followed by dashes on line `i+1`).
fn is_numpy_section_start(lines: &[&str], i: usize) -> bool {
    if i + 1 >= lines.len() {
        return false;
    }
    let name = lines[i].trim();
    let underline = lines[i + 1].trim();

    // Name must be non-empty and underline must be all dashes of similar length
    if name.is_empty() || underline.is_empty() {
        return false;
    }
    if !underline.chars().all(|c| c == '-') {
        return false;
    }
    // Underline length should be at least 3 and roughly match the header
    underline.len() >= 3
}

/// Read the body of a section until the next section header or end of docstring.
fn read_section_body(lines: &[&str], start: usize) -> (Vec<String>, usize) {
    let mut body = Vec::new();
    let mut i = start;

    while i < lines.len() {
        if is_numpy_section_start(lines, i) {
            break;
        }
        body.push(lines[i].to_string());
        i += 1;
    }

    // Trim trailing empty lines
    while body.last().map_or(false, |l| l.trim().is_empty()) {
        body.pop();
    }

    (body, i)
}

/// Parse a section body based on its kind.
fn parse_section(kind: SectionKind, lines: &[String]) -> DocSection {
    let content = match kind {
        SectionKind::Parameters | SectionKind::OtherParameters | SectionKind::Attributes => {
            SectionContent::Params(parse_numpy_params(lines))
        }
        SectionKind::Returns | SectionKind::Yields => {
            SectionContent::Returns(parse_numpy_returns(lines))
        }
        SectionKind::Raises => SectionContent::Raises(parse_numpy_raises(lines)),
        SectionKind::Examples => SectionContent::Examples(parse_numpy_examples(lines)),
        _ => SectionContent::Text(dedent_body(lines)),
    };
    DocSection { kind, content }
}

/// Parse a NumPy Parameters block.
///
/// Expected format:
/// ```text
/// name : type
///     Description.
/// name : type, optional
///     Description.
/// ```
fn parse_numpy_params(lines: &[String]) -> Vec<DocParam> {
    let items = read_numpy_items(lines);
    let mut params = Vec::new();

    for (header, desc_lines) in &items {
        // Split "name : type" on " : "
        let (name, annotation) = if let Some(colon_pos) = header.find(" : ") {
            let name = header[..colon_pos].trim();
            let ann = header[colon_pos + 3..].trim();
            // Strip ", optional" or ", default: ..." suffix
            let ann = ann
                .split(',')
                .next()
                .unwrap_or(ann)
                .trim();
            (name.to_string(), if ann.is_empty() { None } else { Some(ann.to_string()) })
        } else {
            (header.trim().to_string(), None)
        };

        let description = desc_lines.join("\n").trim().to_string();
        params.push(DocParam {
            name,
            annotation,
            description,
        });
    }

    params
}

/// Parse a NumPy Returns block.
///
/// Expected format:
/// ```text
/// type
///     Description.
/// name : type
///     Description.
/// ```
fn parse_numpy_returns(lines: &[String]) -> Vec<DocReturn> {
    let items = read_numpy_items(lines);
    let mut returns = Vec::new();

    for (header, desc_lines) in &items {
        let (name, annotation) = if let Some(colon_pos) = header.find(" : ") {
            let name = header[..colon_pos].trim();
            let ann = header[colon_pos + 3..].trim();
            (Some(name.to_string()), Some(ann.to_string()))
        } else {
            // Just a type, no name
            let ann = header.trim();
            (None, if ann.is_empty() { None } else { Some(ann.to_string()) })
        };

        let description = desc_lines.join("\n").trim().to_string();
        returns.push(DocReturn {
            name,
            annotation,
            description,
        });
    }

    returns
}

/// Parse a NumPy Raises block.
fn parse_numpy_raises(lines: &[String]) -> Vec<DocRaise> {
    let items = read_numpy_items(lines);
    let mut raises = Vec::new();

    for (header, desc_lines) in &items {
        let description = desc_lines.join("\n").trim().to_string();
        raises.push(DocRaise {
            annotation: header.trim().to_string(),
            description,
        });
    }

    raises
}

/// Parse a NumPy Examples block.
///
/// Similar to Google style: distinguishes `>>>` doctest code from prose.
fn parse_numpy_examples(lines: &[String]) -> Vec<ExampleItem> {
    // Reuse the same logic as Google examples parser
    super::google::parse_examples(lines)
}

/// Read items from a NumPy-style section body.
///
/// Items are separated by their indentation: headers are at base indentation,
/// descriptions are indented further.
fn read_numpy_items(lines: &[String]) -> Vec<(String, Vec<String>)> {
    let mut items: Vec<(String, Vec<String>)> = Vec::new();
    if lines.is_empty() {
        return items;
    }

    // Find base indentation
    let base_indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    let mut current_header: Option<String> = None;
    let mut current_desc: Vec<String> = Vec::new();

    for line in lines {
        if line.trim().is_empty() {
            if current_header.is_some() {
                current_desc.push(String::new());
            }
            continue;
        }

        let indent = line.len() - line.trim_start().len();

        if indent <= base_indent {
            // New item header
            if let Some(header) = current_header.take() {
                while current_desc.last().map_or(false, |l| l.is_empty()) {
                    current_desc.pop();
                }
                items.push((header, std::mem::take(&mut current_desc)));
            }
            current_header = Some(line.trim().to_string());
        } else {
            // Description line
            current_desc.push(line.trim().to_string());
        }
    }

    // Flush last item
    if let Some(header) = current_header.take() {
        while current_desc.last().map_or(false, |l| l.is_empty()) {
            current_desc.pop();
        }
        items.push((header, current_desc));
    }

    items
}

/// Dedent body text by removing the common leading whitespace.
fn dedent_body(lines: &[String]) -> String {
    let min_indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                String::new()
            } else if l.len() >= min_indent {
                l[min_indent..].to_string()
            } else {
                l.trim().to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Detect whether a docstring uses NumPy style.
///
/// Looks for header + dash-underline patterns.
pub fn is_numpy_style(docstring: &str) -> bool {
    let lines: Vec<&str> = docstring.lines().collect();
    for i in 0..lines.len() {
        if is_numpy_section_start(&lines, i) {
            let name = lines[i].trim();
            if section_kind(name).is_some() {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_numpy_simple() {
        let doc = r#"Do something useful.

Parameters
----------
x : int
    The input value.
y : str, optional
    Another input.

Returns
-------
bool
    Whether it worked.

Raises
------
ValueError
    If x is negative.
"#;
        let sections = parse_numpy(doc);
        assert!(sections.len() >= 4);
        assert_eq!(sections[0].kind, SectionKind::Text);
        assert_eq!(sections[1].kind, SectionKind::Parameters);
        assert_eq!(sections[2].kind, SectionKind::Returns);
        assert_eq!(sections[3].kind, SectionKind::Raises);

        if let SectionContent::Params(params) = &sections[1].content {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "x");
            assert_eq!(params[0].annotation.as_deref(), Some("int"));
        } else {
            panic!("expected Params");
        }
    }

    #[test]
    fn test_is_numpy_style() {
        let numpy_doc = "Summary.\n\nParameters\n----------\nx : int\n    Desc.\n";
        assert!(is_numpy_style(numpy_doc));

        let google_doc = "Summary.\n\nArgs:\n    x (int): Desc.\n";
        assert!(!is_numpy_style(google_doc));
    }
}
