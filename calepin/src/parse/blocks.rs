//! Block-level parser for .qmd documents.
//!
//! Splits a document body (after YAML front matter removal) into a tree of `Block` nodes
//! via recursive descent. Recognized structures:
//!
//! - **Code chunks**: ```` ```{r} ```` / ```` ```{python} ```` — executable blocks with
//!   pipe-comment options (`#| key: value`) and optional header labels.
//! - **Raw blocks**: ```` ```{=html} ```` / ```` ```{=latex} ```` — format-specific passthrough.
//! - **Unnamed fences**: ```` ``` ```` / ```` ``` python ```` — display-only code blocks.
//! - **Fenced divs**: `::: {.class #id key="val"}` — Pandoc-style containers, arbitrarily
//!   nested by increasing colon count. Includes `.verbatim` divs that skip recursive parsing.
//! - **Text blocks**: everything between structural elements.
//!
//! A closing backtick fence must use at least as many backticks as the opener.
//! Div nesting is tracked by colon count so `:::` cannot close a `::::` opener.

use std::collections::HashMap;
use std::sync::LazyLock;

use anyhow::Result;
use regex::Regex;

use crate::parse::options::{parse_header_label, parse_pipe_options};
use crate::types::{Block, CodeChunk, DivBlock, InlineCode, OptionValue, RawBlock, TextBlock};

// --- Static regexes (compiled once) ---

static RE_RAW_OPEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([\t >]*)(`{3,})\s*\{+=([a-zA-Z0-9_]+)\}+\s*$").unwrap()
});

static RE_CODE_OPEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([\t >]*)(`{3,})\s*\{+([a-zA-Z0-9_]+)(.*?)\}+\s*$").unwrap()
});

static RE_DIV_OPEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(:{3,})\s*\{(.*?)\}\s*$").unwrap()
});

/// Check if a line is a div closing fence with at least `min_colons` colons.
fn is_div_close(line: &str, min_colons: usize) -> bool {
    let trimmed = line.trim_end();
    let colon_count = trimmed.chars().take_while(|&c| c == ':').count();
    colon_count >= min_colons && colon_count == trimmed.len()
}

static RE_INLINE_CODE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"`\{([a-zA-Z0-9_]+(?:,\s*[^}]*)?)\}\s+([^`]+)`").unwrap()
});

/// Parse an .qmd document body (after YAML removal) into a list of blocks.
/// Supports code chunks (```{r}), fenced divs (::: {.class}), and nesting.
#[inline(never)]
pub fn parse_body(body: &str) -> Result<Vec<Block>> {
    let lines: Vec<&str> = body.lines().collect();
    let (blocks, _end) = parse_blocks(&lines, 0, &mut 0, 0)?;
    Ok(blocks)
}

/// Recursive block parser. Parses lines[start..] into blocks.
/// `chunk_counter` is shared across recursion for unique chunk labels.
/// `closer_min` is the minimum colon count for a closing div fence (0 = top level).
/// Returns (blocks, end_position) where end_position is the line after the last consumed line.
fn parse_blocks(
    lines: &[&str],
    start: usize,
    chunk_counter: &mut usize,
    closer_min: usize,
) -> Result<(Vec<Block>, usize)> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut i = start;

    while i < lines.len() {
        // Check for div closing fence (return to parent)
        if i > start && closer_min > 0 && is_div_close(lines[i], closer_min) {
            break;
        }

        // Try fenced div opening: ::: {.class #id}
        if let Some(caps) = RE_DIV_OPEN.captures(lines[i]) {
            let opener_colons = caps.get(1).unwrap().as_str().len();
            let attrs_str = caps.get(2).unwrap().as_str();
            let (classes, id, attrs) = parse_attributes(attrs_str);
            let is_verbatim = classes.iter().any(|c| c == "verbatim");
            i += 1;

            let children = if is_verbatim {
                let (raw, end) = collect_div_body(lines, i, opener_colons, true);
                i = end;
                vec![Block::Text(TextBlock {
                    content: raw,
                })]
            } else {
                let (children, end) = parse_blocks(lines, i, chunk_counter, opener_colons)?;
                // Skip past the closing ::: fence
                i = if end < lines.len() && is_div_close(lines[end], opener_colons) {
                    end + 1
                } else {
                    end
                };
                children
            };

            blocks.push(Block::Div(DivBlock {
                classes,
                id,
                attrs,
                children,
            }));
            continue;
        }

        // Try unnamed fenced code block: ``` (no {language})
        if let Some((block, end)) = parse_unnamed_fence(lines, i) {
            blocks.push(block);
            i = end;
            continue;
        }

        // Try raw block opening fence: ```{=format}
        if let Some((block, end)) = parse_raw_block(lines, i)? {
            blocks.push(block);
            i = end;
            continue;
        }

        // Skip comment block: ```{comment} or ````{comment} etc.
        if let Some(end) = skip_comment_block(lines, i) {
            i = end;
            continue;
        }

        // Try code chunk opening fence: ```{r} or ```{r, label}
        if let Some((block, end)) = parse_code_chunk(lines, i, chunk_counter)? {
            blocks.push(block);
            i = end;
            continue;
        }

        // Text block: collect lines until a fence opens or a div closes
        let start_line = i;
        let mut text = String::new();
        while i < lines.len()
            && !is_unnamed_fence(lines[i])
            && !RE_RAW_OPEN.is_match(lines[i])
            && !RE_CODE_OPEN.is_match(lines[i])
            && !RE_DIV_OPEN.is_match(lines[i])
            && !is_div_close(lines[i], 3)
        {
            // Skip HTML comments (<!-- ... -->), single-line or multi-line
            if let Some(end) = skip_html_comment(lines, i) {
                i = end;
                continue;
            }
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(lines[i]);
            i += 1;
        }

        if !text.is_empty() || start_line == 0 {
            blocks.push(Block::Text(TextBlock {
                content: text,
            }));
        }
    }

    Ok((blocks, i))
}

// --- Block parsers ---

/// Scan past a div body, tracking nested div depth to find the matching closing fence.
/// If `collect` is true, returns the raw content lines joined; otherwise returns an empty string.
/// Returns (content, position after the closing fence).
fn collect_div_body(lines: &[&str], start: usize, min_colons: usize, collect: bool) -> (String, usize) {
    let mut collected: Vec<&str> = Vec::new();
    let mut depth = 0;
    let mut i = start;

    while i < lines.len() {
        if RE_DIV_OPEN.is_match(lines[i]) {
            depth += 1;
            if collect { collected.push(lines[i]); }
        } else if is_div_close(lines[i], min_colons) {
            if depth == 0 {
                i += 1; // skip closing :::
                let content = if collect { collected.join("\n") } else { String::new() };
                return (content, i);
            }
            depth -= 1;
            if collect { collected.push(lines[i]); }
        } else if collect {
            collected.push(lines[i]);
        }
        i += 1;
    }

    let content = if collect { collected.join("\n") } else { String::new() };
    (content, i)
}

static RE_COMMENT_OPEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([\t >]*)(`{3,})\s*\{+comment\}+\s*$").unwrap()
});

/// Skip a ```` ```{comment} ```` block. Returns the position after the closing fence, or None.
fn skip_comment_block(lines: &[&str], i: usize) -> Option<usize> {
    let caps = RE_COMMENT_OPEN.captures(lines[i])?;
    let prefix = caps.get(1).map_or("", |m| m.as_str());
    let fence_marker = caps.get(2).map_or("```", |m| m.as_str());
    let mut j = i + 1;
    while j < lines.len() {
        if is_closing_fence(lines[j], prefix, fence_marker) {
            return Some(j + 1);
        }
        j += 1;
    }
    // Unclosed comment block: skip everything to end
    Some(j)
}

/// Skip an HTML comment (`<!-- ... -->`). Handles both single-line and multi-line comments.
/// Returns the position after the closing `-->`, or None if the line doesn't start a comment.
fn skip_html_comment(lines: &[&str], i: usize) -> Option<usize> {
    let trimmed = lines[i].trim_start();
    if !trimmed.starts_with("<!--") {
        return None;
    }
    // Check if the comment closes on the same line
    if let Some(pos) = trimmed[4..].find("-->") {
        // Single-line comment: only skip if nothing meaningful after the close
        let after = trimmed[4 + pos + 3..].trim();
        if after.is_empty() {
            return Some(i + 1);
        }
        return None; // Inline comment embedded in text; let markdown handle it
    }
    // Multi-line: scan forward for -->
    let mut j = i + 1;
    while j < lines.len() {
        if lines[j].contains("-->") {
            return Some(j + 1);
        }
        j += 1;
    }
    // Unclosed comment: skip to end
    Some(j)
}

/// Count the length of a backtick fence marker at the start of a line.
/// Returns (count, '`') or None if not a fence.
fn parse_fence_marker(line: &str) -> Option<(usize, char)> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('`') {
        return None;
    }
    let count = trimmed.chars().take_while(|&c| c == '`').count();
    if count >= 3 {
        Some((count, '`'))
    } else {
        None
    }
}

/// Try to parse an unnamed fenced code block (```, no {language}).
/// Also handles `{.lang attr="val"}` syntax for display-only blocks with attributes.
/// Returns the block and the position after the closing fence.
fn parse_unnamed_fence(lines: &[&str], i: usize) -> Option<(Block, usize)> {
    let (open_count, marker) = parse_fence_marker(lines[i])?;
    let trimmed = lines[i].trim_start();
    let after = trimmed[open_count..].trim();

    // Parse language and filename from the fence line
    let (lang, filename) = parse_code_block_info(after)?;

    let mut j = i + 1;
    let mut content_lines: Vec<&str> = Vec::new();

    while j < lines.len() {
        if let Some((close_count, close_marker)) = parse_fence_marker(lines[j]) {
            let close_trimmed = lines[j].trim_start();
            if close_marker == marker
                && close_count >= open_count
                && close_trimmed[close_count..].trim().is_empty()
            {
                j += 1;
                break;
            }
        }
        content_lines.push(lines[j]);
        j += 1;
    }

    Some((
        Block::CodeBlock(crate::types::CodeBlock {
            code: content_lines.join("\n"),
            lang,
            filename,
        }),
        j,
    ))
}

/// Parse the info string after a fence marker into (lang, filename).
/// Accepts:
///   - empty → ("", "")
///   - `python` → ("python", "")
///   - `{.python}` → ("python", "")
///   - `{.python filename="run.py"}` → ("python", "run.py")
/// Returns None for `{language}` (executable) or `{=format}` (raw) syntax.
fn parse_code_block_info(info: &str) -> Option<(String, String)> {
    if info.is_empty() {
        return Some((String::new(), String::new()));
    }

    // Bare language: `python`, `js`, etc (no braces, no =)
    if !info.starts_with('{') && !info.starts_with('=') {
        return Some((info.to_string(), String::new()));
    }

    // Must be `{.class ...}` syntax (display-only with attributes)
    if !info.starts_with("{.") {
        return None; // `{language}` or `{=format}` — not our job
    }

    let inner = info.trim_start_matches('{').trim_end_matches('}').trim();
    let mut lang = String::new();
    let mut filename = String::new();

    for token in tokenize_attrs(inner) {
        if token.starts_with('.') {
            lang = token[1..].to_string();
        } else if let Some(val) = token.strip_prefix("filename=") {
            filename = val.trim_matches('"').trim_matches('\'').to_string();
        }
    }

    Some((lang, filename))
}

// Uses tokenize_attrs() defined below for attribute parsing.

/// Try to parse a raw block (```{=format}).
/// Returns the block and position after the closing fence, or None.
fn parse_raw_block(lines: &[&str], i: usize) -> Result<Option<(Block, usize)>> {
    let caps = match RE_RAW_OPEN.captures(lines[i]) {
        Some(c) => c,
        None => return Ok(None),
    };

    let prefix = caps.get(1).map_or("", |m| m.as_str()).to_string();
    let fence_marker = caps.get(2).map_or("```", |m| m.as_str());
    let format = caps.get(3).map_or("", |m| m.as_str()).to_string();

    let (content_lines, _, j) = collect_fenced_body(lines, i + 1, &prefix, fence_marker);

    Ok(Some((
        Block::Raw(RawBlock {
            format,
            content: content_lines.join("\n"),
        }),
        j,
    )))
}

/// Try to parse a code chunk (```{r} or ```{r, label}).
/// Returns the block and position after the closing fence, or None.
fn parse_code_chunk(
    lines: &[&str],
    i: usize,
    chunk_counter: &mut usize,
) -> Result<Option<(Block, usize)>> {
    let caps = match RE_CODE_OPEN.captures(lines[i]) {
        Some(c) => c,
        None => return Ok(None),
    };

    let prefix = caps.get(1).map_or("", |m| m.as_str()).to_string();
    let fence_marker = caps.get(2).map_or("```", |m| m.as_str());
    let engine = caps[3].to_string();
    let header_str = caps.get(4).map_or("", |m| m.as_str());

    // Parse header: extract label and any inline key=value options
    let (header_label, header_opts) = parse_header_label(header_str);

    let (body_lines, _, j) = collect_fenced_body(lines, i + 1, &prefix, fence_marker);
    let code_lines: Vec<String> = body_lines.iter().map(|l| l.to_string()).collect();

    let (pipe_lines, actual_code) = collect_pipe_comments(&code_lines);
    let pipe_comments: Vec<String> = pipe_lines.iter().map(|s| s.to_string()).collect();
    let mut options = parse_pipe_options(&pipe_lines);
    // Merge header options as defaults (pipe options take precedence)
    for (key, value) in header_opts.inner {
        if !options.inner.contains_key(&key) {
            options.inner.insert(key, value);
        }
    }
    options
        .inner
        .insert("engine".to_string(), OptionValue::String(engine));

    *chunk_counter += 1;
    let label = header_label
        .unwrap_or_else(|| format!("chunk-{}", *chunk_counter));

    Ok(Some((
        Block::Code(CodeChunk {
            source: actual_code,
            options,
            label,
            pipe_comments,
        }),
        j,
    )))
}

/// Check if a line is a closing fence matching the given prefix and marker.
fn is_closing_fence(line: &str, prefix: &str, marker: &str) -> bool {
    let expected = format!("{}{}", prefix, marker);
    let trimmed_end = line.trim_end();
    trimmed_end == expected || (trimmed_end.starts_with(&expected) && trimmed_end[expected.len()..].trim().is_empty())
}

/// Collect lines inside a fenced block (code chunk or raw block), stripping the
/// leading prefix from each line. Returns (collected lines, end_line, next position).
fn collect_fenced_body<'a>(lines: &[&'a str], start: usize, prefix: &str, marker: &str) -> (Vec<&'a str>, usize, usize) {
    let mut content: Vec<&str> = Vec::new();
    let mut j = start;
    while j < lines.len() {
        if is_closing_fence(lines[j], prefix, marker) {
            break;
        }
        let line = if !prefix.is_empty() && lines[j].starts_with(prefix) {
            &lines[j][prefix.len()..]
        } else {
            lines[j]
        };
        content.push(line);
        j += 1;
    }
    let end_line = j;
    if j < lines.len() {
        j += 1;
    }
    (content, end_line, j)
}

// --- Attribute parsing ---

/// Parse Pandoc-style attributes: `#id .class1 .class2 key="value"`.
/// Returns (classes, id, key-value attrs). Used for both divs and spans.
pub fn parse_attributes(attrs: &str) -> (Vec<String>, Option<String>, HashMap<String, String>) {
    let tokens = tokenize_attrs(attrs);
    let mut classes = Vec::new();
    let mut id = None;
    let mut kv = HashMap::new();

    for token in &tokens {
        if let Some(cls) = token.strip_prefix('.') {
            classes.push(cls.to_string());
        } else if let Some(id_val) = token.strip_prefix('#') {
            id = Some(id_val.to_string());
        } else if let Some(eq_pos) = token.find('=') {
            let key = &token[..eq_pos];
            let val = token[eq_pos + 1..]
                .trim_matches('"')
                .trim_matches('\'');
            if !key.is_empty() {
                kv.insert(key.to_string(), val.to_string());
            }
        }
    }

    (classes, id, kv)
}

/// Quote-aware tokenizer for attribute strings.
/// Splits on whitespace but respects `"quoted values"` and `'quoted values'`.
fn tokenize_attrs(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote: Option<char> = None;

    for ch in input.chars() {
        match in_quote {
            Some(q) if ch == q => {
                current.push(ch);
                in_quote = None;
            }
            Some(_) => {
                current.push(ch);
            }
            None if ch == '"' || ch == '\'' => {
                current.push(ch);
                in_quote = Some(ch);
            }
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            None => {
                current.push(ch);
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Check if a line starts an unnamed/display fenced code block.
/// Matches: empty info, bare language (`python`), or class syntax (`{.python ...}`).
fn is_unnamed_fence(line: &str) -> bool {
    match parse_fence_marker(line) {
        None => false,
        Some((count, _)) => {
            let trimmed = line.trim_start();
            let after = trimmed[count..].trim();
            after.is_empty()
                || (!after.starts_with('{') && !after.starts_with('='))
                || after.starts_with("{.")
        }
    }
}

/// Extract #| pipe comment lines from the start of a code chunk.
fn collect_pipe_comments(lines: &[String]) -> (Vec<&str>, Vec<String>) {
    let mut pipe_end = 0;
    for line in lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with("#|") {
            pipe_end += 1;
        } else {
            break;
        }
    }

    let pipe_lines: Vec<&str> = lines[..pipe_end]
        .iter()
        .map(|l| {
            let trimmed = l.trim_start();
            trimmed.strip_prefix("#|").unwrap_or(trimmed).trim_start()
        })
        .collect();

    let code_lines = lines[pipe_end..].to_vec();
    (pipe_lines, code_lines)
}

/// Find inline code expressions in text: `{r expr}` or `{r, opts} expr`
pub fn collect_inline_code(text: &str) -> Vec<(usize, usize, InlineCode)> {
    let mut results = Vec::new();

    for caps in RE_INLINE_CODE.captures_iter(text) {
        let full_match = caps.get(0).unwrap();
        let info = caps.get(1).unwrap().as_str();
        let expr = caps.get(2).unwrap().as_str().trim().to_string();

        let engine = info
            .split(',')
            .next()
            .unwrap_or("r")
            .trim()
            .to_string();

        results.push((
            full_match.start(),
            full_match.end(),
            InlineCode { engine, expr },
        ));
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_rmd() {
        let body = "# Hello\n\nSome text.\n\n```{r}\nx <- 1 + 1\nprint(x)\n```\n\nMore text.";
        let blocks = parse_body(body).unwrap();
        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[0], Block::Text(_)));
        assert!(matches!(&blocks[1], Block::Code(_)));
        assert!(matches!(&blocks[2], Block::Text(_)));

        if let Block::Code(chunk) = &blocks[1] {
            assert_eq!(chunk.source, vec!["x <- 1 + 1", "print(x)"]);
            assert_eq!(chunk.options.engine(), "r");
            assert_eq!(chunk.label, "chunk-1");
        }
    }

    #[test]
    fn test_parse_chunk_options_pipe() {
        let body = "```{r}\n#| echo: false\n#| fig-width: 10\nplot(1:10)\n```";
        let blocks = parse_body(body).unwrap();
        assert_eq!(blocks.len(), 1);
        if let Block::Code(chunk) = &blocks[0] {
            assert!(!chunk.options.echo());
            assert!((chunk.options.fig_width() - 10.0).abs() < f64::EPSILON);
            assert_eq!(chunk.source, vec!["plot(1:10)"]);
        }
    }

    #[test]
    fn test_parse_chunk_with_label() {
        let body = "```{r, setup}\n1 + 1\n```";
        let blocks = parse_body(body).unwrap();
        assert_eq!(blocks.len(), 1);
        if let Block::Code(chunk) = &blocks[0] {
            assert_eq!(chunk.label, "setup");
        }
    }

    #[test]
    fn test_parse_chunk_header_options_converted() {
        let body = "```{r, echo=FALSE, fig.width=10}\nplot(1:10)\n```";
        let blocks = parse_body(body).unwrap();
        assert_eq!(blocks.len(), 1);
        if let Block::Code(chunk) = &blocks[0] {
            assert!(!chunk.options.echo());
            assert!((chunk.options.fig_width() - 10.0).abs() < f64::EPSILON);
            assert_eq!(chunk.source, vec!["plot(1:10)"]);
        }
    }

    #[test]
    fn test_parse_pipe_comments() {
        let body = "```{r}\n#| echo: false\n#| fig-width: 12\nplot(1:10)\n```";
        let blocks = parse_body(body).unwrap();
        if let Block::Code(chunk) = &blocks[0] {
            assert!(!chunk.options.echo());
            assert!((chunk.options.fig_width() - 12.0).abs() < f64::EPSILON);
            assert_eq!(chunk.source, vec!["plot(1:10)"]);
        }
    }

    #[test]
    fn test_inline_code() {
        let inlines = collect_inline_code("The value is `{r} 1 + 1` and `{r} pi`.");
        assert_eq!(inlines.len(), 2);
        assert_eq!(inlines[0].2.expr, "1 + 1");
        assert_eq!(inlines[1].2.expr, "pi");
    }

    #[test]
    fn test_parse_fenced_div() {
        let body = "::: {.callout-note}\nThis is a note.\n:::";
        let blocks = parse_body(body).unwrap();
        assert_eq!(blocks.len(), 1);
        if let Block::Div(div) = &blocks[0] {
            assert_eq!(div.classes, vec!["callout-note"]);
            assert_eq!(div.children.len(), 1);
            if let Block::Text(t) = &div.children[0] {
                assert_eq!(t.content, "This is a note.");
            }
        }
    }

    #[test]
    fn test_parse_nested_divs() {
        let body = ":::: {.callout-note}\nOuter\n::: {.callout-warning}\nInner\n:::\n::::";
        let blocks = parse_body(body).unwrap();
        assert_eq!(blocks.len(), 1);
        if let Block::Div(outer) = &blocks[0] {
            assert_eq!(outer.classes, vec!["callout-note"]);
            assert_eq!(outer.children.len(), 2); // Text + Div
            if let Block::Div(inner) = &outer.children[1] {
                assert_eq!(inner.classes, vec!["callout-warning"]);
            }
        }
    }

    #[test]
    fn test_parse_div_with_code() {
        let body = "::: {.example}\nSome text\n```{r}\n1 + 1\n```\n:::";
        let blocks = parse_body(body).unwrap();
        assert_eq!(blocks.len(), 1);
        if let Block::Div(div) = &blocks[0] {
            assert_eq!(div.classes, vec!["example"]);
            assert_eq!(div.children.len(), 2); // Text + Code
            assert!(matches!(&div.children[0], Block::Text(_)));
            assert!(matches!(&div.children[1], Block::Code(_)));
        }
    }

    #[test]
    fn test_parse_attributes_basic() {
        let (classes, id, attrs) = parse_attributes(".theorem #thm-foo");
        assert_eq!(classes, vec!["theorem"]);
        assert_eq!(id, Some("thm-foo".to_string()));
        assert!(attrs.is_empty());
    }

    #[test]
    fn test_parse_attributes_key_value() {
        let (classes, id, attrs) = parse_attributes(r#".theorem #thm-foo title="Fermat's Last""#);
        assert_eq!(classes, vec!["theorem"]);
        assert_eq!(id, Some("thm-foo".to_string()));
        assert_eq!(attrs.get("title").unwrap(), "Fermat's Last");
    }

    #[test]
    fn test_parse_attributes_multiple_kv() {
        let (classes, _, attrs) = parse_attributes(r#".note key="value" width="50%""#);
        assert_eq!(classes, vec!["note"]);
        assert_eq!(attrs.get("key").unwrap(), "value");
        assert_eq!(attrs.get("width").unwrap(), "50%");
    }

    #[test]
    fn test_parse_attributes_multiple_classes() {
        let (classes, id, _) = parse_attributes("#my-id .big .red .bold");
        assert_eq!(id, Some("my-id".to_string()));
        assert_eq!(classes, vec!["big", "red", "bold"]);
    }

    #[test]
    fn test_parse_div_with_attrs() {
        let body = "::: {.theorem #thm-foo title=\"Pythagorean\"}\nContent here.\n:::";
        let blocks = parse_body(body).unwrap();
        assert_eq!(blocks.len(), 1);
        if let Block::Div(div) = &blocks[0] {
            assert_eq!(div.classes, vec!["theorem"]);
            assert_eq!(div.id, Some("thm-foo".to_string()));
            assert_eq!(div.attrs.get("title").unwrap(), "Pythagorean");
        }
    }

    #[test]
    fn test_parse_raw_block() {
        let body = "Before\n\n```{=html}\n<div class=\"custom\">raw html</div>\n```\n\nAfter";
        let blocks = parse_body(body).unwrap();
        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[0], Block::Text(_)));
        if let Block::Raw(raw) = &blocks[1] {
            assert_eq!(raw.format, "html");
            assert_eq!(raw.content, "<div class=\"custom\">raw html</div>");
        } else {
            panic!("expected Raw block");
        }
        assert!(matches!(&blocks[2], Block::Text(_)));
    }

    #[test]
    fn test_parse_raw_block_latex() {
        let body = "```{=latex}\n\\newpage\n\\vspace{1cm}\n```";
        let blocks = parse_body(body).unwrap();
        assert_eq!(blocks.len(), 1);
        if let Block::Raw(raw) = &blocks[0] {
            assert_eq!(raw.format, "latex");
            assert_eq!(raw.content, "\\newpage\n\\vspace{1cm}");
        } else {
            panic!("expected Raw block");
        }
    }

    #[test]
    fn test_unnamed_fence_empty() {
        let body = "```\nplain code\n```";
        let blocks = parse_body(body).unwrap();
        assert_eq!(blocks.len(), 1);
        if let Block::CodeBlock(cb) = &blocks[0] {
            assert_eq!(cb.lang, "");
            assert_eq!(cb.code, "plain code");
        } else {
            panic!("expected CodeBlock");
        }
    }

    #[test]
    fn test_div_fence_length_must_match() {
        // ::: should NOT close a :::: opener
        let body = ":::: {.outer}\nOuter text\n::: {.inner}\nInner text\n:::\nStill outer\n::::";
        let blocks = parse_body(body).unwrap();
        assert_eq!(blocks.len(), 1);
        if let Block::Div(outer) = &blocks[0] {
            assert_eq!(outer.classes, vec!["outer"]);
            // Should have: Text("Outer text"), Div(.inner), Text("Still outer")
            assert_eq!(outer.children.len(), 3);
            assert!(matches!(&outer.children[1], Block::Div(d) if d.classes == vec!["inner"]));
            if let Block::Text(t) = &outer.children[2] {
                assert_eq!(t.content, "Still outer");
            } else {
                panic!("expected Text block for 'Still outer'");
            }
        } else {
            panic!("expected Div block");
        }
    }

    // --- Comment block tests ---

    #[test]
    fn test_comment_block_skipped() {
        let body = "Before\n\n```{comment}\nThis is a comment.\nIt should be skipped.\n```\n\nAfter";
        let blocks = parse_body(body).unwrap();
        assert_eq!(blocks.len(), 2);
        if let Block::Text(t) = &blocks[0] {
            assert_eq!(t.content, "Before\n");
        }
        if let Block::Text(t) = &blocks[1] {
            assert_eq!(t.content, "After");
        }
    }

    #[test]
    fn test_comment_block_four_backticks() {
        let body = "Before\n\n````{comment}\n```{r}\nx <- 1\n```\nNested fences inside comment.\n````\n\nAfter";
        let blocks = parse_body(body).unwrap();
        assert_eq!(blocks.len(), 2);
        if let Block::Text(t) = &blocks[1] {
            assert_eq!(t.content, "After");
        }
    }

    // --- HTML comment tests ---

    #[test]
    fn test_html_comment_single_line() {
        let body = "Before\n<!-- This is a comment -->\nAfter";
        let blocks = parse_body(body).unwrap();
        assert_eq!(blocks.len(), 1);
        if let Block::Text(t) = &blocks[0] {
            assert_eq!(t.content, "Before\nAfter");
        }
    }

    #[test]
    fn test_html_comment_multi_line() {
        let body = "Before\n<!--\nThis is a\nmulti-line comment\n-->\nAfter";
        let blocks = parse_body(body).unwrap();
        assert_eq!(blocks.len(), 1);
        if let Block::Text(t) = &blocks[0] {
            assert_eq!(t.content, "Before\nAfter");
        }
    }

    #[test]
    fn test_html_comment_between_blocks() {
        let body = "Text\n\n<!-- comment -->\n\n```{r}\n1 + 1\n```";
        let blocks = parse_body(body).unwrap();
        assert_eq!(blocks.len(), 2);
        assert!(matches!(&blocks[0], Block::Text(_)));
        assert!(matches!(&blocks[1], Block::Code(_)));
    }
}
