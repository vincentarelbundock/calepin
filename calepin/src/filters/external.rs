// External subprocess filters for divs and spans.
//
// - ExternalFilter::apply()         — Run subprocess filter for divs matching a class.
// - ExternalFilter::run_span_filter() — Run subprocess filter for bracketed spans.
// - resolve_external_filter()       — Locate executable with format-specific fallback.
//
// The executable receives flat JSON on stdin and writes rendered output to stdout.
// Subprocess execution and path resolution are handled by util::run_json_process()
// and util::resolve_executable().

use std::collections::HashMap;
use std::path::PathBuf;

use super::{Filter, FilterResult};
use crate::types::Element;

/// An external filter backed by an executable file.
///
/// The executable receives flat JSON on stdin:
/// ```json
/// {
///   "context": "div",
///   "content": "<p>rendered children</p>",
///   "classes": ["myclass"],
///   "id": "my-id",
///   "format": "html",
///   "title": "user attr"
/// }
/// ```
/// It must write the rendered output to stdout.
pub struct ExternalFilter {
    path: PathBuf,
    class: String,
}

impl ExternalFilter {
    pub fn new(path: PathBuf, class: String) -> Self {
        Self { path, class }
    }

    /// Run this filter for a bracketed span.
    /// Returns `Some(output)` on success, `None` on failure (falls through to template).
    pub fn run_span_filter(
        &self,
        content: &str,
        classes: &[String],
        id: &Option<String>,
        format: &str,
        attrs: &HashMap<String, String>,
    ) -> Option<String> {
        let mut input = serde_json::Map::new();
        input.insert("context".into(), serde_json::Value::String("span".into()));
        input.insert("content".into(), serde_json::Value::String(content.into()));
        input.insert("classes".into(), serde_json::json!(classes));
        input.insert("id".into(), serde_json::Value::String(
            id.as_deref().unwrap_or("").into(),
        ));
        input.insert("format".into(), serde_json::Value::String(format.into()));

        // Flatten user key-value attributes
        for (k, v) in attrs {
            if !input.contains_key(k) {
                input.insert(k.clone(), serde_json::Value::String(v.clone()));
            }
        }

        crate::util::run_json_process(&self.path, &serde_json::Value::Object(input))
    }
}

impl Filter for ExternalFilter {
    fn apply(
        &self,
        element: &Element,
        format: &str,
        vars: &mut HashMap<String, String>,
    ) -> FilterResult {
        let (classes, id) = match element {
            Element::Div { classes, id, .. } => (classes.clone(), id.clone()),
            _ => return FilterResult::Pass,
        };

        if !classes.iter().any(|c| c == &self.class) {
            return FilterResult::Pass;
        }

        let mut input = serde_json::Map::new();
        input.insert("context".into(), serde_json::Value::String("div".into()));
        input.insert("content".into(), serde_json::Value::String(
            vars.get("children").cloned().unwrap_or_default(),
        ));
        input.insert("classes".into(), serde_json::json!(classes));
        input.insert("id".into(), serde_json::Value::String(
            id.as_deref().unwrap_or("").into(),
        ));
        input.insert("format".into(), serde_json::Value::String(format.into()));

        for (k, v) in vars.iter() {
            if !input.contains_key(k) {
                input.insert(k.clone(), serde_json::Value::String(v.clone()));
            }
        }

        match crate::util::run_json_process(&self.path, &serde_json::Value::Object(input)) {
            Some(output) => FilterResult::Rendered(output),
            None => FilterResult::Pass,
        }
    }
}

/// Look for an external filter executable for the given class.
/// Checks project-level then user-level, with and without format extension.
pub fn resolve_external_filter(class: &str, format: &str) -> Option<ExternalFilter> {
    crate::util::resolve_executable("filters", class, Some(format))
        .map(|path| ExternalFilter::new(path, class.to_string()))
}
