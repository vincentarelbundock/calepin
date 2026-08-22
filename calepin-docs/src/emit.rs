//! Turn the extracted API model into Typst source.
//!
//! Each definition becomes a call to a template function (`api-function` /
//! `api-class`) declared in a generated `api.typ`. Keeping the markup in the
//! template means the site can restyle the reference without regenerating it.

use pydocstring::model::{Block, Docstring, FreeSectionKind, SectionKind};
use pydocstring::parse::parse;

use crate::model::{ApiItem, Class, Function};
use crate::typst_escape::{escape_content, escape_string, render_prose};

/// The template every generated page imports. Written once; left alone on
/// later runs so local styling survives regeneration.
pub const TEMPLATE: &str = include_str!("assets/api.typ");

fn parse_docstring(text: Option<&str>) -> Docstring {
    let mut doc = text.map(|t| parse(t).to_model()).unwrap_or_default();
    recover_plain_directive(&mut doc);
    doc
}

/// Recover a leading `.. deprecated::` from a section-less docstring.
///
/// `pydocstring` 0.4 only lifts directives out of Google and NumPy documents;
/// in a `Style::Plain` docstring the directive stays in the extended summary
/// and would otherwise render as body prose. Drop this once upstream parses
/// directives for plain documents too.
fn recover_plain_directive(doc: &mut Docstring) {
    if !doc.directives.is_empty() {
        return;
    }
    let Some(extended) = doc.extended_summary.clone() else {
        return;
    };
    let Some(rest) = extended.trim_start().strip_prefix(".. deprecated::") else {
        return;
    };

    let mut lines = rest.lines();
    let version = lines.next().unwrap_or("").trim().to_string();
    let body: Vec<&str> = lines.collect();
    let indent = body
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let description: String = body
        .iter()
        .map(|l| {
            if l.len() >= indent {
                &l[indent..]
            } else {
                l.trim()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    doc.directives.push(pydocstring::model::Directive {
        name: "deprecated".to_string(),
        argument: (!version.is_empty()).then_some(version),
        description: (!description.is_empty()).then_some(description),
    });
    doc.extended_summary = None;
}

/// Render docstring prose, honouring Markdown block structure when the text
/// uses it. Google and NumPy field text is usually a sentence or two and falls
/// through to the plain escaper unchanged.
fn render_body(text: &str) -> String {
    if crate::markdown::looks_like_markdown(text) {
        crate::markdown::render_markdown(text)
    } else {
        render_prose(text)
    }
}

/// `key: [content]`, omitted entirely when the prose is empty.
fn content_arg(key: &str, text: Option<&str>) -> Option<String> {
    let text = text?.trim();
    if text.is_empty() {
        return None;
    }
    Some(format!("  {key}: [{}],\n", render_body(text)))
}

fn string_arg(key: &str, text: &str) -> String {
    format!("  {key}: \"{}\",\n", escape_string(text))
}

/// Render one docstring entry as a Typst dictionary literal.
fn entry_dict(
    name: Option<&str>,
    type_annotation: Option<&str>,
    description: Option<&str>,
) -> String {
    entry_dict_from(name, type_annotation, description, None)
}

/// As [`entry_dict`], but recording the base class a member was inherited from.
fn entry_dict_from(
    name: Option<&str>,
    type_annotation: Option<&str>,
    description: Option<&str>,
    inherited_from: Option<&str>,
) -> String {
    let mut fields = Vec::new();
    if let Some(name) = name {
        fields.push(format!("name: \"{}\"", escape_string(name)));
    }
    if let Some(ty) = type_annotation {
        fields.push(format!("type: \"{}\"", escape_string(ty)));
    }
    let description = description.unwrap_or("").trim();
    fields.push(format!("desc: [{}]", render_body(description)));
    if let Some(source) = inherited_from {
        fields.push(format!("from: \"{}\"", escape_string(source)));
    }
    format!("(  {}  )", fields.join(", "))
}

/// Collect the docstring's parameter / return / raises entries into Typst
/// arrays, plus the free-text sections that survive as prose.
struct RenderedSections {
    params: Vec<String>,
    returns: Vec<String>,
    raises: Vec<String>,
    attributes: Vec<String>,
    seealso: Vec<String>,
    notes: Vec<(String, String)>,
}

fn render_sections(doc: &Docstring) -> RenderedSections {
    let mut out = RenderedSections {
        params: Vec::new(),
        returns: Vec::new(),
        raises: Vec::new(),
        attributes: Vec::new(),
        seealso: Vec::new(),
        notes: Vec::new(),
    };

    for section in &doc.sections {
        match &section.kind {
            SectionKind::Parameters
            | SectionKind::KeywordParameters
            | SectionKind::OtherParameters
            | SectionKind::Receives => {
                for block in &section.blocks {
                    if let Block::Parameter(p) = block {
                        // NumPy allows `x, y : int` — one row per name.
                        for name in &p.names {
                            out.params.push(entry_dict(
                                Some(name),
                                p.type_annotation.as_deref(),
                                p.description.as_deref(),
                            ));
                        }
                    }
                }
            }
            SectionKind::Returns | SectionKind::Yields => {
                for block in &section.blocks {
                    if let Block::Return(r) = block {
                        out.returns.push(entry_dict(
                            r.name.as_deref(),
                            r.type_annotation.as_deref(),
                            r.description.as_deref(),
                        ));
                    }
                }
            }
            SectionKind::Raises | SectionKind::Warns => {
                for block in &section.blocks {
                    if let Block::Exception(e) = block {
                        out.raises.push(entry_dict(
                            None,
                            Some(&e.type_name),
                            e.description.as_deref(),
                        ));
                    }
                }
            }
            SectionKind::Attributes => {
                for block in &section.blocks {
                    if let Block::Attribute(a) = block {
                        for name in &a.names {
                            out.attributes.push(entry_dict(
                                Some(name),
                                a.type_annotation.as_deref(),
                                a.description.as_deref(),
                            ));
                        }
                    }
                }
            }
            SectionKind::SeeAlso => {
                for block in &section.blocks {
                    if let Block::SeeAlso(s) = block {
                        out.seealso.push(entry_dict(
                            Some(&s.names.join(", ")),
                            None,
                            s.description.as_deref(),
                        ));
                    }
                }
            }
            SectionKind::FreeText(kind) => {
                let title = free_section_title(kind);
                let body: Vec<&str> = section
                    .blocks
                    .iter()
                    .filter_map(|b| match b {
                        Block::Paragraph(text) => Some(text.as_str()),
                        _ => None,
                    })
                    .collect();
                if !body.is_empty() {
                    out.notes.push((title, body.join("\n\n")));
                }
            }
            SectionKind::Methods | SectionKind::References => {}
            _ => {}
        }
    }

    out
}

fn free_section_title(kind: &FreeSectionKind) -> String {
    match kind {
        FreeSectionKind::Notes => "Notes",
        FreeSectionKind::Examples => "Examples",
        FreeSectionKind::Warnings => "Warnings",
        FreeSectionKind::Todo => "Todo",
        FreeSectionKind::Attention => "Attention",
        FreeSectionKind::Caution => "Caution",
        FreeSectionKind::Danger => "Danger",
        FreeSectionKind::Error => "Error",
        FreeSectionKind::Hint => "Hint",
        FreeSectionKind::Important => "Important",
        FreeSectionKind::Tip => "Tip",
        FreeSectionKind::Unknown(name) => name.as_str(),
        _ => "Notes",
    }
    .to_string()
}

fn array_arg(key: &str, entries: &[String]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let mut out = format!("  {key}: (\n");
    for entry in entries {
        out.push_str(&format!("    {entry},\n"));
    }
    out.push_str("  ),\n");
    out
}

fn notes_arg(notes: &[(String, String)]) -> String {
    if notes.is_empty() {
        return String::new();
    }
    let mut out = String::from("  notes: (\n");
    for (title, body) in notes {
        out.push_str(&format!(
            "    (title: \"{}\", body: [{}]),\n",
            escape_string(title),
            render_body(body)
        ));
    }
    out.push_str("  ),\n");
    out
}

/// Examples sections usually hold code, so they render as a raw block rather
/// than prose. Detected by title, since the docstring grammar does not mark it.
fn is_code_section(title: &str) -> bool {
    matches!(title, "Examples" | "Example")
}

fn render_function_call(function: &Function, call: &str) -> String {
    let doc = parse_docstring(function.docstring.as_deref());
    let sections = render_sections(&doc);

    let mut out = format!("#{call}(\n");
    out.push_str(&string_arg("name", &function.name));
    if let Some(source) = &function.inherited_from {
        out.push_str(&string_arg("inherited", source));
    }
    out.push_str(&string_arg("qualname", &function.qualname));
    out.push_str(&string_arg("signature", &function.signature()));

    if let Some(arg) = content_arg("summary", summary_for(&doc, &function.name).as_deref()) {
        out.push_str(&arg);
    }
    if let Some(arg) = content_arg("description", doc.extended_summary.as_deref()) {
        out.push_str(&arg);
    }
    if let Some(deprecated) = doc.deprecation() {
        out.push_str(&string_arg(
            "deprecated",
            deprecated.argument.as_deref().unwrap_or(""),
        ));
        if let Some(arg) = content_arg("deprecated-note", deprecated.description.as_deref()) {
            out.push_str(&arg);
        }
    }
    if !function.decorators.is_empty() {
        out.push_str(&array_arg(
            "decorators",
            &function
                .decorators
                .iter()
                .map(|d| format!("\"{}\"", escape_string(d)))
                .collect::<Vec<_>>(),
        ));
    }

    out.push_str(&array_arg("params", &sections.params));
    out.push_str(&array_arg("returns", &sections.returns));
    out.push_str(&array_arg("raises", &sections.raises));
    out.push_str(&array_arg("seealso", &sections.seealso));

    let (code_notes, prose_notes): (Vec<_>, Vec<_>) = sections
        .notes
        .into_iter()
        .partition(|(title, _)| is_code_section(title));
    out.push_str(&notes_arg(&prose_notes));
    if let Some((_, body)) = code_notes.first() {
        out.push_str(&format!("  examples: \"{}\",\n", escape_string(body)));
    }

    out.push_str(")\n");
    out
}

pub fn render_function(function: &Function) -> String {
    render_function_call(function, "api-function")
}

pub fn render_class(class: &Class) -> String {
    let doc = parse_docstring(class.docstring.as_deref());
    let sections = render_sections(&doc);

    let mut out = String::from("#api-class(\n");
    out.push_str(&string_arg("name", &class.name));
    out.push_str(&string_arg("qualname", &class.qualname));

    if !class.bases.is_empty() {
        out.push_str(&array_arg(
            "bases",
            &class
                .bases
                .iter()
                .map(|b| format!("\"{}\"", escape_string(b)))
                .collect::<Vec<_>>(),
        ));
    }
    if let Some(arg) = content_arg("summary", summary_for(&doc, &class.name).as_deref()) {
        out.push_str(&arg);
    }
    if let Some(arg) = content_arg("description", doc.extended_summary.as_deref()) {
        out.push_str(&arg);
    }

    // Attributes documented in the docstring, plus annotated class-level names
    // the docstring did not mention.
    let mut attributes = sections.attributes;
    for attribute in &class.attributes {
        let already_documented = attributes
            .iter()
            .any(|entry| entry.contains(&format!("name: \"{}\"", escape_string(&attribute.name))));
        if !already_documented {
            attributes.push(entry_dict_from(
                Some(&attribute.name),
                attribute.annotation.as_deref(),
                attribute.description.as_deref(),
                attribute.inherited_from.as_deref(),
            ));
        }
    }
    out.push_str(&array_arg("attributes", &attributes));
    out.push_str(&notes_arg(&sections.notes));
    out.push_str(")\n");

    // Methods render as nested function entries beneath the class.
    for method in &class.methods {
        out.push('\n');
        out.push_str(&render_function_call(method, "api-method"));
    }

    out
}

pub fn render_item(item: &ApiItem) -> String {
    match item {
        ApiItem::Function(f) => render_function(f),
        ApiItem::Class(c) => render_class(c),
    }
}

/// A full page: the template import plus the rendered definition.
/// A Calepin `<website-metadata>` header, so the page joins a site's navigation
/// and search index with a real title and summary.
fn website_header(title: &str, summary: &str) -> String {
    format!(
        "#set document(title: [{}])\n#metadata((\n  title: \"{}\",\n  summary: \"{}\",\n)) <website-metadata>\n\n",
        escape_content(title),
        escape_string(title),
        escape_string(summary),
    )
}

/// Plain-text summary for site metadata, where markup would leak into a
/// `<meta>` tag rather than render.
fn plain_summary(item: &ApiItem) -> String {
    let doc = parse_docstring(item.docstring());
    let text = summary_for(&doc, item.name())
        .or_else(|| {
            doc.extended_summary
                .as_deref()
                .map(|body| strip_heading_markup(body.lines().next().unwrap_or("")))
        })
        .unwrap_or_default();

    let text = collapse_whitespace(&text);
    if text.is_empty() {
        format!("API reference for {}.", item.qualname())
    } else {
        text
    }
}

/// Fold newlines and runs of spaces into single spaces, for metadata that ends
/// up in a `<meta>` tag rather than in the page body.
fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Strip Markdown heading and code markup from a summary.
///
/// Underscores and asterisks are left alone: `*args` and `get_dataset` are
/// names far more often than they are emphasis.
fn strip_heading_markup(text: &str) -> String {
    text.trim()
        .trim_start_matches('#')
        .trim()
        .replace('`', "")
        .trim()
        .to_string()
}

/// The summary to render, or `None` when it would only restate the heading.
///
/// Markdown docstrings routinely open with `# name()`, which `pydocstring`
/// reports as the summary — sometimes on its own, sometimes with the real
/// summary on the line below. The page already prints that name as its
/// heading, so the restatement is dropped either way.
fn summary_for(doc: &Docstring, name: &str) -> Option<String> {
    let raw = doc.summary.as_deref()?.trim();

    // A Markdown heading is a title, not a summary -- the page prints its own.
    // Drop the heading line and keep whatever followed it, if anything.
    let body = if raw.starts_with('#') {
        raw.split_once('\n').map(|(_, rest)| rest).unwrap_or("")
    } else {
        raw
    };

    let cleaned = strip_heading_markup(body);
    let names_the_definition = cleaned.trim_end_matches("()") == name;

    (!cleaned.is_empty() && !names_the_definition).then_some(cleaned)
}

/// A full page: the template import plus the rendered definition.
///
/// In `website` mode the page also carries Calepin site metadata; without it
/// the file is a standalone Typst document that compiles on its own.
pub fn render_page(item: &ApiItem, template_import: &str, website: bool) -> String {
    let header = if website {
        website_header(item.qualname(), &plain_summary(item))
    } else {
        String::new()
    };
    format!(
        "// Generated by calepin-docs. Do not edit; regenerate instead.\n\
         {header}#import \"{template_import}\": *\n\n{}",
        render_item(item)
    )
}

/// An index page linking every documented definition, grouped by module.
pub fn render_index(
    items: &[ApiItem],
    template_import: &str,
    package: &str,
    website: bool,
) -> String {
    let header = if website {
        website_header(
            &format!("{package} API reference"),
            &format!("Every exported function and class in {package}."),
        )
    } else {
        String::new()
    };
    let mut out = format!(
        "// Generated by calepin-docs. Do not edit; regenerate instead.\n\
         {header}#import \"{template_import}\": *\n\n#api-index(\n  package: \"{}\",\n  entries: (\n",
        escape_string(package)
    );

    for item in items {
        let kind = match item {
            ApiItem::Function(_) => "function",
            ApiItem::Class(_) => "class",
        };
        out.push_str(&format!(
            "    (name: \"{}\", qualname: \"{}\", kind: \"{}\", file: \"{}\", summary: [{}]),\n",
            escape_string(item.name()),
            escape_string(item.qualname()),
            kind,
            escape_string(&file_stem(item.qualname())),
            // The one-line index needs the same cleaned summary the page
            // metadata uses, not the raw first line of a Markdown docstring.
            render_prose(&plain_summary(item)),
        ));
    }

    out.push_str("  ),\n)\n");
    out
}

/// Filename for a definition: the dotted qualname, which is already unique.
pub fn file_stem(qualname: &str) -> String {
    qualname.to_string()
}
