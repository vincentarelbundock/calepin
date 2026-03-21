// Callout filter: enriches callout divs with title, icon, collapse, appearance.
//
// - CalloutFilter::apply() — Detect callout-* classes, set default title/icon,
//                             handle collapse (HTML <details>) and appearance.

use std::collections::HashMap;

use super::{Filter, FilterResult};
use crate::types::Element;

pub struct CalloutFilter;

impl CalloutFilter {
    pub fn new() -> Self { Self }
}

impl Filter for CalloutFilter {
    fn apply(&self, element: &Element, _format: &str, vars: &mut HashMap<String, String>) -> FilterResult {
        let classes = match element {
            Element::Div { classes, .. } => classes,
            _ => return FilterResult::Pass,
        };

        let callout_class = match classes.iter().find(|c| c.starts_with("callout-")) {
            Some(c) => c.clone(),
            None => return FilterResult::Pass,
        };

        let (default_title, icon) = match callout_class.as_str() {
            "callout-note" => ("Note", "\u{2139}\u{fe0f}"),
            "callout-tip" => ("Tip", "\u{1f4a1}"),
            "callout-warning" => ("Warning", "\u{26a0}\u{fe0f}"),
            "callout-important" => ("Important", "\u{2757}"),
            "callout-caution" => ("Caution", "\u{1f525}"),
            _ => ("Note", "\u{2139}\u{fe0f}"),
        };

        let callout_type = callout_class.strip_prefix("callout-").unwrap_or("note");
        vars.insert("callout_type".to_string(), callout_type.to_string());

        let title = vars.get("title").filter(|t| !t.is_empty())
            .cloned()
            .unwrap_or_else(|| default_title.to_string());
        vars.insert("title".to_string(), title.clone());
        vars.insert("icon".to_string(), icon.to_string());
        vars.insert("header".to_string(), format!("{} {}", icon, title));

        let collapse = vars.get("collapse").map(|v| v == "true").unwrap_or(false);
        vars.insert("collapse".to_string(), collapse.to_string());

        let appearance = vars.get("appearance").cloned().unwrap_or_else(|| "default".to_string());
        vars.insert("appearance".to_string(), appearance.clone());

        FilterResult::Continue
    }
}
