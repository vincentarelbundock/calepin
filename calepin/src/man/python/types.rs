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

//! Data types for extracted Python documentation.

/// The kind of a docstring section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Text,
    Parameters,
    OtherParameters,
    Returns,
    Yields,
    Raises,
    Examples,
    Notes,
    Warnings,
    References,
    Attributes,
    Deprecated,
    Admonition,
}

impl SectionKind {
    /// Section heading for .qmd output. `None` means preamble text (no heading).
    pub fn heading(&self) -> Option<&'static str> {
        match self {
            SectionKind::Text => None,
            SectionKind::Parameters => Some("Parameters"),
            SectionKind::OtherParameters => Some("Other Parameters"),
            SectionKind::Returns => Some("Returns"),
            SectionKind::Yields => Some("Yields"),
            SectionKind::Raises => Some("Raises"),
            SectionKind::Examples => Some("Examples"),
            SectionKind::Notes => Some("Notes"),
            SectionKind::Warnings => Some("Warnings"),
            SectionKind::References => Some("References"),
            SectionKind::Attributes => Some("Attributes"),
            SectionKind::Deprecated => Some("Deprecated"),
            SectionKind::Admonition => Some("Note"),
        }
    }
}

/// A parameter documented in a docstring.
#[derive(Debug, Clone)]
pub struct DocParam {
    pub name: String,
    pub annotation: Option<String>,
    pub description: String,
}

/// A return value documented in a docstring.
#[derive(Debug, Clone)]
pub struct DocReturn {
    pub name: Option<String>,
    pub annotation: Option<String>,
    pub description: String,
}

/// An exception documented in a docstring.
#[derive(Debug, Clone)]
pub struct DocRaise {
    pub annotation: String,
    pub description: String,
}

/// An item in an Examples section: either prose text or a code block.
#[derive(Debug, Clone)]
pub enum ExampleItem {
    Text(String),
    Code(String),
}

/// The content of a parsed docstring section.
#[derive(Debug, Clone)]
pub enum SectionContent {
    Text(String),
    Params(Vec<DocParam>),
    Returns(Vec<DocReturn>),
    Raises(Vec<DocRaise>),
    Examples(Vec<ExampleItem>),
    /// Generic list of named items (attributes, other_parameters, etc.).
    Generic(Vec<DocParam>),
    Admonition { title: String, description: String },
}

/// A parsed docstring section.
#[derive(Debug, Clone)]
pub struct DocSection {
    pub kind: SectionKind,
    pub content: SectionContent,
}

/// The kind of a function parameter (from the AST, not the docstring).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    PositionalOnly,
    Regular,
    VarPositional,
    KeywordOnly,
    VarKeyword,
}

/// A function parameter extracted from the Python AST.
#[derive(Debug, Clone)]
pub struct PyParam {
    pub name: String,
    pub annotation: Option<String>,
    pub default: Option<String>,
    pub kind: ParamKind,
}

/// The kind of a top-level Python object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyObjectKind {
    Function,
    Class,
}

/// A documented Python object extracted from source code.
#[derive(Debug, Clone)]
pub struct PyObject {
    pub name: String,
    pub path: String,
    pub kind: PyObjectKind,
    pub docstring: Option<String>,
    pub parameters: Vec<PyParam>,
}

/// Docstring style to use for parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocstringStyle {
    Google,
    Numpy,
    Sphinx,
    /// Pass the docstring through as-is (already markdown). Only `>>>`
    /// prompts are converted to ```` ```python ```` code blocks.
    Markdown,
    Auto,
}

impl DocstringStyle {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "google" => DocstringStyle::Google,
            "numpy" | "numpydoc" => DocstringStyle::Numpy,
            "sphinx" | "rst" | "rest" => DocstringStyle::Sphinx,
            "markdown" | "md" => DocstringStyle::Markdown,
            _ => DocstringStyle::Auto,
        }
    }
}
