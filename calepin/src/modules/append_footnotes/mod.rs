//! Footnote module: collection, accumulation, section rendering, and document injection.
//!
//! Owns all footnote-related state and logic:
//! - Pre-scan collection of cross-block footnote definitions
//! - Counter chaining across Text elements
//! - Accumulation of rendered footnote defs from the walker
//! - HTML footnote section rendering with backref injection
//! - Document injection (TransformDocument)

use regex::Regex;
use std::sync::LazyLock;

use crate::render::template::TemplateVars;

use crate::types::Element;
use crate::render::elements::ElementRenderer;
use crate::modules::transform_document::TransformDocument;

// ---------------------------------------------------------------------------
// Footnote state (owned by ElementRenderer, logic lives here)
// ---------------------------------------------------------------------------

/// Footnote state carried across Text element renders.
pub struct FootnoteState {
    /// Running footnote counter (chained across Text elements).
    counter: std::cell::Cell<usize>,
    /// Accumulated rendered footnote defs from all Text elements.
    accumulated_defs: std::cell::RefCell<Vec<(usize, String)>>,
    /// Global footnote definitions collected from pre-scan (for cross-block resolution).
    global_defs: std::cell::RefCell<String>,
}

impl FootnoteState {
    pub fn new() -> Self {
        Self {
            counter: std::cell::Cell::new(0),
            accumulated_defs: std::cell::RefCell::new(Vec::new()),
            global_defs: std::cell::RefCell::new(String::new()),
        }
    }

    /// Return the current footnote counter value (for walker options).
    pub fn counter(&self) -> usize {
        self.counter.get()
    }

    /// Update the counter after a walker pass.
    pub fn set_counter(&self, value: usize) {
        self.counter.set(value);
    }

    /// Pre-scan all elements for footnote definitions (`[^name]: ...`).
    /// Collected defs are injected into Text elements during rendering so comrak
    /// can resolve cross-block footnote references.
    pub fn collect_defs(&self, elements: &[Element]) {
        let mut defs = String::new();
        collect_defs_recursive(elements, &mut defs);
        *self.global_defs.borrow_mut() = defs;
    }

    /// Inject collected global footnote defs into a text block if it contains
    /// footnote references. Returns the (possibly augmented) text.
    /// Defs already present in the text block are not duplicated.
    pub fn inject_defs<'a>(&self, text: &'a str) -> std::borrow::Cow<'a, str> {
        let defs = self.global_defs.borrow();
        if !defs.is_empty() && text.contains("[^") {
            // Only inject defs whose names are not already defined in this block
            let mut extra = String::new();
            for m in RE_FN_DEF.find_iter(&defs) {
                let def_text = m.as_str();
                // Extract the name: `[^name]:` at the start
                if let Some(name_end) = def_text.find("]:") {
                    let name_with_prefix = &def_text[..name_end + 1]; // `[^name]`
                    // Check if this def name already appears as a definition in the block
                    let def_marker = format!("{}:", name_with_prefix);
                    if !text.contains(&def_marker) {
                        extra.push_str("\n\n");
                        extra.push_str(def_text);
                    }
                }
            }
            if extra.is_empty() {
                std::borrow::Cow::Borrowed(text)
            } else {
                std::borrow::Cow::Owned(format!("{}{}", text, extra))
            }
        } else {
            std::borrow::Cow::Borrowed(text)
        }
    }

    /// Accumulate rendered footnote defs from a walker pass (deduplicating by ID).
    pub fn accumulate(&self, defs: Vec<(usize, String)>) {
        if defs.is_empty() { return; }
        let mut acc = self.accumulated_defs.borrow_mut();
        for def in defs {
            if !acc.iter().any(|(id, _)| *id == def.0) {
                acc.push(def);
            }
        }
    }

    /// Render the combined footnote section (HTML only).
    /// Returns empty string if no footnotes accumulated.
    pub fn render_section(&self, format: &str) -> String {
        if format != "html" { return String::new(); }
        let defs = self.accumulated_defs.borrow();
        if defs.is_empty() { return String::new(); }
        render_footnote_section(&defs)
    }
}

// ---------------------------------------------------------------------------
// Footnote def collection (pre-scan)
// ---------------------------------------------------------------------------

static RE_FN_DEF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^\[\^[^\]]+\]:.*(?:\n(?:    |\t).*)*").unwrap()
});

fn collect_defs_recursive(elements: &[Element], defs: &mut String) {
    for el in elements {
        if let Element::Text { content } = el {
            for m in RE_FN_DEF.find_iter(content) {
                defs.push_str("\n\n");
                defs.push_str(m.as_str());
            }
        }
        if let Element::Div { children, .. } = el {
            collect_defs_recursive(children, defs);
        }
    }
}

// ---------------------------------------------------------------------------
// HTML footnote section rendering
// ---------------------------------------------------------------------------

/// Render a combined footnote section from accumulated defs.
pub fn render_footnote_section(defs: &[(usize, String)]) -> String {
    let mut footnote_items = String::new();
    for (id, content) in defs {
        let backref = format!(
            " <a href=\"#fnref-{}\" class=\"footnote-backref\" data-footnote-backref data-footnote-backref-idx=\"{}\" aria-label=\"Back to reference {}\">↩</a>",
            id, id, id
        );
        // Insert backref before the last </p> so it appears inline
        let body = if let Some(pos) = content.rfind("</p>") {
            format!("{}{}{}", &content[..pos], backref, &content[pos..])
        } else {
            format!("{}{}", content, backref)
        };
        footnote_items.push_str(&format!("<li id=\"fn-{}\">\n{}\n</li>\n", id, body));
    }

    let mut vars = TemplateVars::with_writer("html");
    vars.clp.insert("footnotes".to_string(), "true".to_string());
    vars.clp.insert("footnote_items".to_string(), footnote_items);
    let tpl = include_str!("../../templates/html/footnotes.html");
    crate::render::template::apply_template(tpl, &vars)
}

// ---------------------------------------------------------------------------
// TransformDocument: inject footnote section into assembled page
// ---------------------------------------------------------------------------

/// Insert `content` before the first matching tag, or append at end.
fn insert_before_tag(document: &str, content: &str, tags: &[&str]) -> String {
    for tag in tags {
        if let Some(pos) = document.find(tag) {
            return format!("{}{}\n{}", &document[..pos], content, &document[pos..]);
        }
    }
    format!("{}{}", document, content)
}

pub struct AppendFootnotes;

impl TransformDocument for AppendFootnotes {
    fn transform(&self, document: &str, writer: &str, renderer: &ElementRenderer) -> String {
        let footnotes = renderer.footnotes.render_section(writer);
        if footnotes.is_empty() {
            return document.to_string();
        }
        insert_before_tag(document, &footnotes, &["</main>", "</body>"])
    }
}
