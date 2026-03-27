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

//! Google-style docstring parser.
//!
//! Recognizes sections like:
//!
//! ```text
//! Args:
//!     x (int): The input value.
//!     y (str): Another input.
//!
//! Returns:
//!     bool: Whether it worked.
//!
//! Raises:
//!     ValueError: If x is negative.
//! ```

use regex::Regex;
use std::sync::LazyLock;

use super::types::*;

/// Known section names mapped to their kind.
fn section_kind(name: &str) -> Option<SectionKind> {
    match name.to_lowercase().as_str() {
        "args" | "arguments" | "params" | "parameters" => Some(SectionKind::Parameters),
        "keyword args" | "keyword arguments" | "other parameters" | "other params" => {
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
        _ => None,
    }
}

static RE_SECTION_HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)(\w[\w\s]*\w|\w)\s*:\s*$").unwrap());

static RE_ADMONITION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)(\w[\w\s-]*\w|\w):\s+(.+)$").unwrap());

/// Parse a Google-style docstring into sections.
pub fn parse_google(docstring: &str) -> Vec<DocSection> {
    let lines: Vec<&str> = docstring.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    let mut sections = Vec::new();
    let mut i = 0;

    // Collect preamble text (before any section header)
    let mut preamble = Vec::new();
    while i < lines.len() {
        if is_section_header(lines[i]) {
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
        if let Some(caps) = RE_SECTION_HEADER.captures(lines[i]) {
            let header_indent = caps.get(1).map_or(0, |m| m.as_str().len());
            let name = caps.get(2).map_or("", |m| m.as_str());

            i += 1;

            // Read section body: lines indented more than the header
            let (body_lines, next_i) = read_indented_block(&lines, i, header_indent);
            i = next_i;

            if let Some(kind) = section_kind(name) {
                let section = parse_section(kind, &body_lines);
                sections.push(section);
            } else {
                // Treat as admonition
                let body = body_lines.join("\n").trim().to_string();
                sections.push(DocSection {
                    kind: SectionKind::Admonition,
                    content: SectionContent::Admonition {
                        title: name.to_string(),
                        description: body,
                    },
                });
            }
        } else if let Some(caps) = RE_ADMONITION.captures(lines[i]) {
            // Inline admonition: "Note: some text"
            let name = caps.get(2).map_or("", |m| m.as_str());
            let inline_text = caps.get(3).map_or("", |m| m.as_str());
            i += 1;
            sections.push(DocSection {
                kind: SectionKind::Admonition,
                content: SectionContent::Admonition {
                    title: name.to_string(),
                    description: inline_text.to_string(),
                },
            });
        } else {
            i += 1;
        }
    }

    sections
}

/// Check if a line looks like a Google-style section header ("Args:", "Returns:", etc.).
fn is_section_header(line: &str) -> bool {
    RE_SECTION_HEADER.is_match(line)
}

/// Read an indented block of lines starting at `start`, where all lines are
/// indented more than `base_indent`.
fn read_indented_block(lines: &[&str], start: usize, base_indent: usize) -> (Vec<String>, usize) {
    let mut block = Vec::new();
    let mut i = start;
    let mut content_indent: Option<usize> = None;

    while i < lines.len() {
        let line = lines[i];

        // Empty lines are included in the block
        if line.trim().is_empty() {
            block.push(String::new());
            i += 1;
            continue;
        }

        let indent = line.len() - line.trim_start().len();

        // Stop at a new section header at or below base indent
        if indent <= base_indent && is_section_header(line) {
            break;
        }

        // Stop if we hit a line at or below base indent that isn't blank
        if indent <= base_indent {
            break;
        }

        // Determine content indentation from first non-empty line
        let ci = *content_indent.get_or_insert(indent);

        if indent >= ci {
            block.push(line[ci..].to_string());
        } else {
            block.push(line.trim().to_string());
        }
        i += 1;
    }

    // Trim trailing empty lines
    while block.last().map_or(false, |l| l.is_empty()) {
        block.pop();
    }

    (block, i)
}

/// Parse a section body based on its kind.
fn parse_section(kind: SectionKind, lines: &[String]) -> DocSection {
    let content = match kind {
        SectionKind::Parameters | SectionKind::OtherParameters | SectionKind::Attributes => {
            SectionContent::Params(parse_params(lines))
        }
        SectionKind::Returns | SectionKind::Yields => {
            SectionContent::Returns(parse_returns(lines))
        }
        SectionKind::Raises => SectionContent::Raises(parse_raises(lines)),
        SectionKind::Examples => SectionContent::Examples(parse_examples(lines)),
        _ => SectionContent::Text(lines.join("\n").trim().to_string()),
    };
    DocSection { kind, content }
}

/// Parse a Parameters/Attributes block.
///
/// Expected format:
/// ```text
/// name (type): Description.
///     Continuation.
/// name2: Description without type.
/// ```
fn parse_params(lines: &[String]) -> Vec<DocParam> {
    let items = read_block_items(lines);
    let mut params = Vec::new();

    for (first_line, rest) in &items {
        // Split on first colon: "name (type)" : "description"
        let (name_part, desc_first) = match first_line.split_once(':') {
            Some((n, d)) => (n.trim(), d.trim()),
            None => (first_line.as_str(), ""),
        };

        // Extract type annotation from parentheses: "name (type)"
        let (name, annotation) = if let Some(paren_start) = name_part.find('(') {
            let name = name_part[..paren_start].trim();
            let ann = name_part[paren_start + 1..]
                .trim_end_matches(')')
                .trim();
            (name.to_string(), Some(ann.to_string()))
        } else {
            (name_part.trim().to_string(), None)
        };

        let mut desc_lines = vec![desc_first.to_string()];
        desc_lines.extend(rest.iter().cloned());
        let description = desc_lines.join("\n").trim().to_string();

        params.push(DocParam {
            name,
            annotation,
            description,
        });
    }

    params
}

/// Parse a Returns/Yields block.
///
/// Expected format:
/// ```text
/// type: Description.
/// name (type): Description.
/// ```
fn parse_returns(lines: &[String]) -> Vec<DocReturn> {
    let items = read_block_items(lines);
    let mut returns = Vec::new();

    for (first_line, rest) in &items {
        let (name_or_type, desc_first) = match first_line.split_once(':') {
            Some((n, d)) => (n.trim(), d.trim()),
            None => ("", first_line.as_str()),
        };

        let (name, annotation) = if let Some(paren_start) = name_or_type.find('(') {
            let name = name_or_type[..paren_start].trim();
            let ann = name_or_type[paren_start + 1..]
                .trim_end_matches(')')
                .trim();
            (
                Some(name.to_string()),
                Some(ann.to_string()),
            )
        } else if !name_or_type.is_empty() {
            // Could be just a type (no name)
            (None, Some(name_or_type.to_string()))
        } else {
            (None, None)
        };

        let mut desc_lines = vec![desc_first.to_string()];
        desc_lines.extend(rest.iter().cloned());
        let description = desc_lines.join("\n").trim().to_string();

        returns.push(DocReturn {
            name,
            annotation,
            description,
        });
    }

    returns
}

/// Parse a Raises block.
///
/// Expected format:
/// ```text
/// ValueError: If x is negative.
/// TypeError: If x is not an int.
/// ```
fn parse_raises(lines: &[String]) -> Vec<DocRaise> {
    let items = read_block_items(lines);
    let mut raises = Vec::new();

    for (first_line, rest) in &items {
        let (exc, desc_first) = match first_line.split_once(':') {
            Some((e, d)) => (e.trim(), d.trim()),
            None => (first_line.as_str(), ""),
        };

        let mut desc_lines = vec![desc_first.to_string()];
        desc_lines.extend(rest.iter().cloned());
        let description = desc_lines.join("\n").trim().to_string();

        raises.push(DocRaise {
            annotation: exc.to_string(),
            description,
        });
    }

    raises
}

/// Parse an Examples block.
///
/// Distinguishes prose text from `>>>` doctest code blocks.
pub fn parse_examples(lines: &[String]) -> Vec<ExampleItem> {
    let mut items = Vec::new();
    let mut text_buf = Vec::new();
    let mut code_buf = Vec::new();
    let mut in_code = false;

    for line in lines {
        let trimmed = line.trim();

        if trimmed.starts_with(">>>") {
            // Start or continue code block
            if !in_code {
                // Flush text
                let text = text_buf.join("\n").trim().to_string();
                if !text.is_empty() {
                    items.push(ExampleItem::Text(text));
                }
                text_buf.clear();
                in_code = true;
            }
            // Strip >>> or ... prompt
            let code_line = if trimmed.starts_with(">>> ") {
                &trimmed[4..]
            } else {
                &trimmed[3..]
            };
            code_buf.push(code_line.to_string());
        } else if trimmed.starts_with("...") && in_code {
            let code_line = if trimmed.starts_with("... ") {
                &trimmed[4..]
            } else {
                &trimmed[3..]
            };
            code_buf.push(code_line.to_string());
        } else if in_code {
            // Output line or end of code block
            if trimmed.is_empty() || (!trimmed.starts_with('#') && !code_buf.is_empty()) {
                // End the code block
                let code = code_buf.join("\n").trim().to_string();
                if !code.is_empty() {
                    items.push(ExampleItem::Code(code));
                }
                code_buf.clear();
                in_code = false;
                if !trimmed.is_empty() {
                    text_buf.push(line.to_string());
                }
            }
        } else {
            text_buf.push(line.to_string());
        }
    }

    // Flush remaining
    if !code_buf.is_empty() {
        let code = code_buf.join("\n").trim().to_string();
        if !code.is_empty() {
            items.push(ExampleItem::Code(code));
        }
    }
    let text = text_buf.join("\n").trim().to_string();
    if !text.is_empty() {
        items.push(ExampleItem::Text(text));
    }

    items
}

/// Read block items from a section body. Each item starts at the base
/// indentation level, and continuation lines are further indented.
///
/// Returns a list of (first_line, continuation_lines).
fn read_block_items(lines: &[String]) -> Vec<(String, Vec<String>)> {
    let mut items: Vec<(String, Vec<String>)> = Vec::new();
    if lines.is_empty() {
        return items;
    }

    // Determine base indentation from first non-empty line
    let base_indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    let mut current_first: Option<String> = None;
    let mut current_rest: Vec<String> = Vec::new();

    for line in lines {
        if line.trim().is_empty() {
            if current_first.is_some() {
                current_rest.push(String::new());
            }
            continue;
        }

        let indent = line.len() - line.trim_start().len();

        if indent <= base_indent {
            // New item at base indentation
            if let Some(first) = current_first.take() {
                // Trim trailing empty lines from current rest
                while current_rest.last().map_or(false, |l| l.is_empty()) {
                    current_rest.pop();
                }
                items.push((first, std::mem::take(&mut current_rest)));
            }
            current_first = Some(line.trim().to_string());
        } else {
            // Continuation line
            current_rest.push(line.trim().to_string());
        }
    }

    // Flush last item
    if let Some(first) = current_first.take() {
        while current_rest.last().map_or(false, |l| l.is_empty()) {
            current_rest.pop();
        }
        items.push((first, current_rest));
    }

    items
}

/// Detect whether a docstring uses Google style.
///
/// Looks for patterns like "Args:", "Returns:", "Raises:" at consistent indentation.
pub fn is_google_style(docstring: &str) -> bool {
    for line in docstring.lines() {
        if RE_SECTION_HEADER.is_match(line) {
            let name = RE_SECTION_HEADER
                .captures(line)
                .and_then(|c| c.get(2))
                .map(|m| m.as_str())
                .unwrap_or("");
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
    fn test_parse_google_simple() {
        let doc = r#"Do something useful.

Args:
    x (int): The input.
    y (str): Another input.

Returns:
    bool: Whether it worked.

Raises:
    ValueError: If x is negative.
"#;
        let sections = parse_google(doc);
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
    fn test_parse_google_examples() {
        let doc = r#"Do something.

Examples:
    Some intro text.

    >>> foo(1)
    42
    >>> bar(2)
    99
"#;
        let sections = parse_google(doc);
        let examples = sections.iter().find(|s| s.kind == SectionKind::Examples);
        assert!(examples.is_some());
        if let SectionContent::Examples(items) = &examples.unwrap().content {
            assert!(items.len() >= 2);
        }
    }
}
