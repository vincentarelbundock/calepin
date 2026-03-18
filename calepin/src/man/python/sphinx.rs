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

//! Sphinx/reST-style docstring parser.
//!
//! Recognizes field list directives:
//!
//! ```text
//! :param str name: The person's name.
//! :param greeting: The greeting word.
//! :type greeting: str
//! :returns: A greeting string.
//! :rtype: str
//! :raises ValueError: If name is empty.
//! :var x: An attribute.
//! :vartype x: int
//! ```
//!
//! Credits to Patrick Lannigan who originally implemented this parser in
//! the pytkdocs project.

use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

use super::types::*;

/// Directive names for parameters.
const PARAM_NAMES: &[&str] = &["param", "parameter", "arg", "argument", "key", "keyword"];

/// Directive names for parameter types (separate from inline).
const PARAM_TYPE_NAMES: &[&str] = &["type"];

/// Directive names for attributes/variables.
const ATTRIBUTE_NAMES: &[&str] = &["var", "ivar", "cvar"];

/// Directive names for attribute types.
const ATTRIBUTE_TYPE_NAMES: &[&str] = &["vartype"];

/// Directive names for return descriptions.
const RETURN_NAMES: &[&str] = &["returns", "return"];

/// Directive names for return types.
const RETURN_TYPE_NAMES: &[&str] = &["rtype"];

/// Directive names for exceptions.
const EXCEPTION_NAMES: &[&str] = &["raises", "raise", "except", "exception"];

/// Regex matching a Sphinx field directive line: `:directive ...: value`
static RE_DIRECTIVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*:(\w+)").unwrap());

/// Accumulated state while parsing directives.
#[derive(Default)]
struct ParsedValues {
    description: Vec<String>,
    parameters: Vec<(String, DocParam)>,
    param_types: HashMap<String, String>,
    attributes: Vec<(String, DocParam)>,
    attribute_types: HashMap<String, String>,
    exceptions: Vec<DocRaise>,
    return_desc: Option<String>,
    return_type: Option<String>,
}

/// Parse a Sphinx/reST-style docstring into sections.
pub fn parse_sphinx(docstring: &str) -> Vec<DocSection> {
    let lines: Vec<&str> = docstring.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }

    let mut parsed = ParsedValues::default();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if let Some(directive_name) = get_directive_name(line) {
            if is_param_directive(&directive_name) {
                i = read_param(&lines, i, &mut parsed);
            } else if is_param_type_directive(&directive_name) {
                i = read_param_type(&lines, i, &mut parsed);
            } else if is_attribute_directive(&directive_name) {
                i = read_attribute(&lines, i, &mut parsed);
            } else if is_attribute_type_directive(&directive_name) {
                i = read_attribute_type(&lines, i, &mut parsed);
            } else if is_exception_directive(&directive_name) {
                i = read_exception(&lines, i, &mut parsed);
            } else if is_return_directive(&directive_name) {
                i = read_return(&lines, i, &mut parsed);
            } else if is_return_type_directive(&directive_name) {
                i = read_return_type(&lines, i, &mut parsed);
            } else {
                parsed.description.push(line.to_string());
                i += 1;
            }
        } else {
            parsed.description.push(line.to_string());
            i += 1;
        }
    }

    build_sections(parsed)
}

/// Extract the directive name from a line like `:param str name: desc`.
fn get_directive_name(line: &str) -> Option<String> {
    RE_DIRECTIVE.captures(line).map(|c| c[1].to_string())
}

fn is_param_directive(name: &str) -> bool {
    PARAM_NAMES.contains(&name)
}

fn is_param_type_directive(name: &str) -> bool {
    PARAM_TYPE_NAMES.contains(&name)
}

fn is_attribute_directive(name: &str) -> bool {
    ATTRIBUTE_NAMES.contains(&name)
}

fn is_attribute_type_directive(name: &str) -> bool {
    ATTRIBUTE_TYPE_NAMES.contains(&name)
}

fn is_exception_directive(name: &str) -> bool {
    EXCEPTION_NAMES.contains(&name)
}

fn is_return_directive(name: &str) -> bool {
    RETURN_NAMES.contains(&name)
}

fn is_return_type_directive(name: &str) -> bool {
    RETURN_TYPE_NAMES.contains(&name)
}

/// Parse a directive line into (directive_parts, value) and consolidate
/// continuation lines.
///
/// A directive line has the form `:directive_name arg1 arg2: value`.
/// Continuation lines are non-directive lines that follow.
fn parse_directive(lines: &[&str], offset: usize) -> (Vec<String>, String, usize) {
    // Consolidate continuation lines
    let mut block = vec![lines[offset].trim_start().to_string()];
    let mut next = offset + 1;
    while next < lines.len() {
        let line = lines[next];
        // Stop at next directive or blank line
        if line.trim_start().starts_with(':') && RE_DIRECTIVE.is_match(line) {
            break;
        }
        if line.trim().is_empty() {
            break;
        }
        block.push(line.to_string());
        next += 1;
    }

    let full_line = block.join(" ");

    // Split `:directive parts: value`
    // Find first colon, then second colon
    let trimmed = full_line.trim_start();
    if !trimmed.starts_with(':') {
        return (Vec::new(), full_line, next);
    }

    // Remove leading ':'
    let rest = &trimmed[1..];
    // Find closing ':'
    if let Some(colon_pos) = rest.find(':') {
        let directive_str = &rest[..colon_pos];
        let value = rest[colon_pos + 1..].trim().to_string();
        let parts: Vec<String> = directive_str.split_whitespace().map(|s| s.to_string()).collect();
        (parts, value, next)
    } else {
        (Vec::new(), full_line, next)
    }
}

/// Read a `:param` directive.
///
/// Formats:
/// - `:param name: description` (no type)
/// - `:param type name: description` (with inline type)
fn read_param(lines: &[&str], offset: usize, parsed: &mut ParsedValues) -> usize {
    let (parts, value, next) = parse_directive(lines, offset);

    // parts[0] = directive name (param/arg/etc.)
    // parts[1] = type or name
    // parts[2] = name (if type is present)
    let (name, inline_type) = match parts.len() {
        2 => (parts[1].clone(), None),
        3 => (parts[2].clone(), Some(parts[1].clone())),
        n if n > 3 => (parts[n - 1].clone(), None),
        _ => return next,
    };

    // Resolve annotation: inline type > :type: directive > none
    let annotation = inline_type.or_else(|| parsed.param_types.get(&name).cloned());

    parsed.parameters.push((
        name.clone(),
        DocParam {
            name,
            annotation,
            description: value,
        },
    ));

    next
}

/// Read a `:type name:` directive.
fn read_param_type(lines: &[&str], offset: usize, parsed: &mut ParsedValues) -> usize {
    let (parts, value, next) = parse_directive(lines, offset);
    if parts.len() == 2 {
        let name = parts[1].clone();
        let type_str = value.replace(" or ", " | ");
        parsed.param_types.insert(name.clone(), type_str.clone());

        // Update existing parameter if already parsed
        for (pname, param) in &mut parsed.parameters {
            if pname == &name && param.annotation.is_none() {
                param.annotation = Some(type_str.clone());
            }
        }
    }
    next
}

/// Read a `:var`/`:ivar`/`:cvar` directive.
fn read_attribute(lines: &[&str], offset: usize, parsed: &mut ParsedValues) -> usize {
    let (parts, value, next) = parse_directive(lines, offset);
    if parts.len() == 2 {
        let name = parts[1].clone();
        let annotation = parsed.attribute_types.get(&name).cloned();
        parsed.attributes.push((
            name.clone(),
            DocParam {
                name,
                annotation,
                description: value,
            },
        ));
    }
    next
}

/// Read a `:vartype name:` directive.
fn read_attribute_type(lines: &[&str], offset: usize, parsed: &mut ParsedValues) -> usize {
    let (parts, value, next) = parse_directive(lines, offset);
    if parts.len() == 2 {
        let name = parts[1].clone();
        let type_str = value.replace(" or ", " | ");
        parsed.attribute_types.insert(name.clone(), type_str.clone());

        // Update existing attribute if already parsed
        for (aname, attr) in &mut parsed.attributes {
            if aname == &name && attr.annotation.is_none() {
                attr.annotation = Some(type_str.clone());
            }
        }
    }
    next
}

/// Read a `:raises` directive.
fn read_exception(lines: &[&str], offset: usize, parsed: &mut ParsedValues) -> usize {
    let (parts, value, next) = parse_directive(lines, offset);
    if parts.len() == 2 {
        parsed.exceptions.push(DocRaise {
            annotation: parts[1].clone(),
            description: value,
        });
    }
    next
}

/// Read a `:returns`/`:return` directive.
fn read_return(lines: &[&str], offset: usize, parsed: &mut ParsedValues) -> usize {
    let (_parts, value, next) = parse_directive(lines, offset);
    parsed.return_desc = Some(value);
    next
}

/// Read a `:rtype:` directive.
fn read_return_type(lines: &[&str], offset: usize, parsed: &mut ParsedValues) -> usize {
    let (_parts, value, next) = parse_directive(lines, offset);
    parsed.return_type = Some(value.replace(" or ", " | "));
    next
}

/// Convert accumulated parsed values into docstring sections.
fn build_sections(parsed: ParsedValues) -> Vec<DocSection> {
    let mut sections = Vec::new();

    // Description text
    let text = strip_blank_lines(&parsed.description).join("\n");
    let text = text.trim().to_string();
    if !text.is_empty() {
        sections.push(DocSection {
            kind: SectionKind::Text,
            content: SectionContent::Text(text),
        });
    }

    // Parameters
    if !parsed.parameters.is_empty() {
        let params: Vec<DocParam> = parsed.parameters.into_iter().map(|(_, p)| p).collect();
        sections.push(DocSection {
            kind: SectionKind::Parameters,
            content: SectionContent::Params(params),
        });
    }

    // Attributes
    if !parsed.attributes.is_empty() {
        let attrs: Vec<DocParam> = parsed.attributes.into_iter().map(|(_, a)| a).collect();
        sections.push(DocSection {
            kind: SectionKind::Attributes,
            content: SectionContent::Params(attrs),
        });
    }

    // Returns
    if parsed.return_desc.is_some() || parsed.return_type.is_some() {
        sections.push(DocSection {
            kind: SectionKind::Returns,
            content: SectionContent::Returns(vec![DocReturn {
                name: None,
                annotation: parsed.return_type,
                description: parsed.return_desc.unwrap_or_default(),
            }]),
        });
    }

    // Raises
    if !parsed.exceptions.is_empty() {
        sections.push(DocSection {
            kind: SectionKind::Raises,
            content: SectionContent::Raises(parsed.exceptions),
        });
    }

    sections
}

/// Strip leading and trailing blank lines.
fn strip_blank_lines(lines: &[String]) -> &[String] {
    let start = lines.iter().position(|l| !l.trim().is_empty()).unwrap_or(lines.len());
    let end = lines.iter().rposition(|l| !l.trim().is_empty()).map(|i| i + 1).unwrap_or(0);
    if start >= end {
        &[]
    } else {
        &lines[start..end]
    }
}

/// Detect whether a docstring uses Sphinx/reST style.
///
/// Looks for `:param`, `:type`, `:returns`, `:raises`, etc. directives.
pub fn is_sphinx_style(docstring: &str) -> bool {
    let all_directives: Vec<&str> = PARAM_NAMES
        .iter()
        .chain(PARAM_TYPE_NAMES)
        .chain(ATTRIBUTE_NAMES)
        .chain(ATTRIBUTE_TYPE_NAMES)
        .chain(RETURN_NAMES)
        .chain(RETURN_TYPE_NAMES)
        .chain(EXCEPTION_NAMES)
        .copied()
        .collect();

    for line in docstring.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(':') {
            for &directive in &all_directives {
                let prefix = format!(":{}",  directive);
                if trimmed.starts_with(&prefix) {
                    // Must be followed by space or another colon
                    let rest = &trimmed[prefix.len()..];
                    if rest.starts_with(' ') || rest.starts_with(':') {
                        return true;
                    }
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sphinx_simple() {
        let doc = r#"Do something useful.

:param str x: The input value.
:param y: Another input.
:type y: int
:returns: A result.
:rtype: bool
:raises ValueError: If x is empty.
"#;
        let sections = parse_sphinx(doc);
        assert!(sections.len() >= 3);

        // Text section
        assert_eq!(sections[0].kind, SectionKind::Text);

        // Parameters
        assert_eq!(sections[1].kind, SectionKind::Parameters);
        if let SectionContent::Params(params) = &sections[1].content {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name, "x");
            assert_eq!(params[0].annotation.as_deref(), Some("str"));
            assert_eq!(params[1].name, "y");
            assert_eq!(params[1].annotation.as_deref(), Some("int"));
        } else {
            panic!("expected Params");
        }

        // Returns
        let returns = sections.iter().find(|s| s.kind == SectionKind::Returns).unwrap();
        if let SectionContent::Returns(rets) = &returns.content {
            assert_eq!(rets[0].annotation.as_deref(), Some("bool"));
            assert_eq!(rets[0].description, "A result.");
        } else {
            panic!("expected Returns");
        }

        // Raises
        let raises = sections.iter().find(|s| s.kind == SectionKind::Raises).unwrap();
        if let SectionContent::Raises(excs) = &raises.content {
            assert_eq!(excs[0].annotation, "ValueError");
        } else {
            panic!("expected Raises");
        }
    }

    #[test]
    fn test_parse_sphinx_attributes() {
        let doc = r#"A class.

:ivar name: The name.
:vartype name: str
"#;
        let sections = parse_sphinx(doc);
        let attrs = sections.iter().find(|s| s.kind == SectionKind::Attributes).unwrap();
        if let SectionContent::Params(items) = &attrs.content {
            assert_eq!(items[0].name, "name");
            assert_eq!(items[0].annotation.as_deref(), Some("str"));
        } else {
            panic!("expected Params");
        }
    }

    #[test]
    fn test_is_sphinx_style() {
        assert!(is_sphinx_style("Summary.\n\n:param x: Value.\n"));
        assert!(is_sphinx_style(":returns: Result.\n"));
        assert!(!is_sphinx_style("Args:\n    x: Value.\n"));
        assert!(!is_sphinx_style("Parameters\n----------\nx : int\n"));
    }

    #[test]
    fn test_parse_sphinx_continuation() {
        let doc = ":param name: A long description\n    that spans multiple lines.\n:returns: Something.";
        let sections = parse_sphinx(doc);
        let params = sections.iter().find(|s| s.kind == SectionKind::Parameters).unwrap();
        if let SectionContent::Params(p) = &params.content {
            assert!(p[0].description.contains("that spans multiple lines"));
        } else {
            panic!("expected Params");
        }
    }
}
