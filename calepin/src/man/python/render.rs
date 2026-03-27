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

//! Render extracted Python documentation as `.qmd` files.
//!
//! Produces the same output format as the original `extract_pydocs.py`:
//! TOML front matter, usage signature, and docstring sections.

use super::types::*;

/// Convert a `PyObject` to `.qmd` content.
///
/// Returns `None` if the object has no docstring.
pub fn obj_to_qmd(obj: &PyObject, sections: &[DocSection]) -> Option<String> {
    if obj.docstring.is_none() && sections.is_empty() {
        return None;
    }

    let short_name = obj.path.rsplit('.').next().unwrap_or(&obj.name);
    let mut out = format!("---\ntitle = \"`{}`\"\n---\n", short_name);

    // Usage / signature
    if !obj.parameters.is_empty() || obj.kind == PyObjectKind::Function {
        let sig = build_signature(short_name, &obj.parameters);
        out.push_str(&format!("\n## Usage\n\n```python\n{}\n```\n", sig));
    }

    // Docstring sections
    for section in sections {
        let heading = section.kind.heading();
        let body = render_section(&section.content);
        if body.is_empty() {
            continue;
        }
        match heading {
            None => {
                // Preamble text
                out.push_str(&format!("\n{}\n", body));
            }
            Some(h) => {
                out.push_str(&format!("\n## {}\n\n{}\n", h, body));
            }
        }
    }

    Some(out)
}

/// Render a `PyObject` in markdown-passthrough mode.
///
/// The docstring is already markdown, so we emit it as-is except for
/// converting `>>>` / `...` doctest prompts into ```` ```python ```` blocks.
pub fn obj_to_qmd_markdown(obj: &PyObject, docstring: &str) -> String {
    let short_name = obj.path.rsplit('.').next().unwrap_or(&obj.name);
    let mut out = format!("---\ntitle = \"`{}`\"\n---\n", short_name);

    // Usage / signature
    if !obj.parameters.is_empty() || obj.kind == PyObjectKind::Function {
        let sig = build_signature(short_name, &obj.parameters);
        out.push_str(&format!("\n## Usage\n\n```python\n{}\n```\n", sig));
    }

    // Emit the docstring body, converting >>> blocks to ```python
    out.push('\n');
    out.push_str(&convert_doctest_blocks(docstring));
    out.push('\n');

    out
}

/// Convert `>>>` / `...` doctest prompts in a markdown docstring into
/// fenced ```` ```python ```` code blocks. Non-doctest lines pass through
/// unchanged.
fn convert_doctest_blocks(text: &str) -> String {
    let mut out = String::new();
    let mut code_buf: Vec<String> = Vec::new();
    let mut in_doctest = false;

    for line in text.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with(">>>") {
            if !in_doctest {
                in_doctest = true;
            }
            let code = if trimmed.starts_with(">>> ") {
                &trimmed[4..]
            } else {
                &trimmed[3..]
            };
            code_buf.push(code.to_string());
        } else if trimmed.starts_with("...") && in_doctest {
            let code = if trimmed.starts_with("... ") {
                &trimmed[4..]
            } else {
                &trimmed[3..]
            };
            code_buf.push(code.to_string());
        } else if in_doctest {
            // End of doctest block -- flush
            if !code_buf.is_empty() {
                out.push_str("```python\n");
                out.push_str(&code_buf.join("\n"));
                out.push_str("\n```\n");
                code_buf.clear();
            }
            in_doctest = false;
            // Skip output lines (non-empty, non-prompt lines right after code)
            // but keep blank lines and new content
            if !trimmed.is_empty() {
                // Could be output or new prose. If it doesn't look like a
                // heading or list, treat it as output and skip.
                if !trimmed.starts_with('#')
                    && !trimmed.starts_with('-')
                    && !trimmed.starts_with('*')
                    && !trimmed.starts_with('>')
                    && !trimmed.starts_with('[')
                {
                    continue;
                }
            }
            out.push_str(line);
            out.push('\n');
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }

    // Flush any trailing doctest block
    if !code_buf.is_empty() {
        out.push_str("```python\n");
        out.push_str(&code_buf.join("\n"));
        out.push_str("\n```\n");
    }

    out
}

/// Build a function signature string from parameters.
fn build_signature(name: &str, params: &[PyParam]) -> String {
    let parts: Vec<String> = params
        .iter()
        .map(|p| {
            let mut s = p.name.clone();
            if let Some(ann) = &p.annotation {
                s.push_str(&format!(": {}", ann));
            }
            if let Some(default) = &p.default {
                s.push_str(&format!(" = {}", default));
            }
            s
        })
        .collect();
    format!("{}({})", name, parts.join(", "))
}

/// Render a section's content to markdown.
fn render_section(content: &SectionContent) -> String {
    match content {
        SectionContent::Text(text) => text.clone(),

        SectionContent::Params(params) => render_params(params),

        SectionContent::Returns(returns) => render_returns(returns),

        SectionContent::Raises(raises) => render_raises(raises),

        SectionContent::Examples(items) => render_examples(items),

        SectionContent::Generic(items) => render_params(items),

        SectionContent::Admonition { title, description } => {
            format!("**{}**\n\n{}", title, description)
        }
    }
}

/// Render parameters as a definition list.
fn render_params(params: &[DocParam]) -> String {
    let mut lines = Vec::new();
    for p in params {
        let ann = p
            .annotation
            .as_ref()
            .map(|a| format!(" (*{}*)", a))
            .unwrap_or_default();
        lines.push(format!("**`{}`**{}\n: {}\n", p.name, ann, p.description));
    }
    lines.join("\n")
}

/// Render return values.
fn render_returns(returns: &[DocReturn]) -> String {
    let mut lines = Vec::new();
    for r in returns {
        let ann = r
            .annotation
            .as_ref()
            .map(|a| format!("*{}*", a))
            .unwrap_or_default();

        if let Some(name) = &r.name {
            lines.push(format!("**`{}`** {}\n: {}\n", name, ann, r.description));
        } else if !ann.is_empty() {
            lines.push(format!("{}: {}\n", ann, r.description));
        } else {
            lines.push(format!("{}\n", r.description));
        }
    }
    lines.join("\n")
}

/// Render raised exceptions.
fn render_raises(raises: &[DocRaise]) -> String {
    let mut lines = Vec::new();
    for r in raises {
        lines.push(format!("**`{}`**\n: {}\n", r.annotation, r.description));
    }
    lines.join("\n")
}

/// Render examples as a mix of prose and code blocks.
fn render_examples(items: &[ExampleItem]) -> String {
    let mut out = String::new();
    for item in items {
        match item {
            ExampleItem::Text(text) => {
                let text = text.trim();
                if !text.is_empty() {
                    out.push_str(text);
                    out.push_str("\n\n");
                }
            }
            ExampleItem::Code(code) => {
                out.push_str("```python\n");
                out.push_str(code);
                out.push_str("\n```\n\n");
            }
        }
    }
    out.trim_end().to_string()
}

/// Sanitize a name for use as a filename.
pub fn safe_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_signature() {
        let params = vec![
            PyParam {
                name: "x".into(),
                annotation: Some("int".into()),
                default: None,
                kind: ParamKind::Regular,
            },
            PyParam {
                name: "y".into(),
                annotation: Some("str".into()),
                default: Some("\"hello\"".into()),
                kind: ParamKind::Regular,
            },
        ];
        assert_eq!(
            build_signature("foo", &params),
            "foo(x: int, y: str = \"hello\")"
        );
    }

    #[test]
    fn test_obj_to_qmd() {
        let obj = PyObject {
            name: "foo".into(),
            path: "pkg.foo".into(),
            kind: PyObjectKind::Function,
            docstring: Some("Do something.".into()),
            parameters: vec![PyParam {
                name: "x".into(),
                annotation: Some("int".into()),
                default: None,
                kind: ParamKind::Regular,
            }],
        };
        let sections = vec![DocSection {
            kind: SectionKind::Text,
            content: SectionContent::Text("Do something.".into()),
        }];
        let qmd = obj_to_qmd(&obj, &sections).unwrap();
        assert!(qmd.contains("title = \"`foo`\""));
        assert!(qmd.contains("foo(x: int)"));
        assert!(qmd.contains("Do something."));
    }

    #[test]
    fn test_safe_name() {
        assert_eq!(safe_name("my_func"), "my_func");
        assert_eq!(safe_name("my func!"), "my_func_");
    }
}
