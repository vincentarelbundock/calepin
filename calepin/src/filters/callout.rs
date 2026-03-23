// Callout filter: enriches callout divs with title, icon, collapse, appearance.
//
// - CalloutFilter::apply() -- Detect callout-* classes, set default title/icon,
//                             handle collapse (HTML <details>) and appearance.
//                             Auto-number when the div has a cross-referenceable ID.

use std::cell::RefCell;
use std::collections::HashMap;

use super::{Filter, FilterResult};
use crate::types::Element;

/// Map callout class to cross-reference prefix.
const CALLOUT_PREFIXES: &[(&str, &str)] = &[
    ("callout-note", "nte"),
    ("callout-tip", "tip"),
    ("callout-warning", "wrn"),
    ("callout-important", "imp"),
    ("callout-caution", "cau"),
];

/// Return the cross-reference prefix for a callout class, if any.
pub fn callout_prefix(class: &str) -> Option<&'static str> {
    CALLOUT_PREFIXES.iter().find(|(c, _)| *c == class).map(|(_, p)| *p)
}

pub struct CalloutFilter {
    counters: RefCell<HashMap<String, usize>>,
}

impl CalloutFilter {
    pub fn new() -> Self {
        Self { counters: RefCell::new(HashMap::new()) }
    }
}

impl Filter for CalloutFilter {
    fn apply(&self, element: &Element, _format: &str, vars: &mut HashMap<String, String>) -> FilterResult {
        let (classes, id) = match element {
            Element::Div { classes, id, .. } => (classes, id),
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

        // Auto-number when the div has an ID matching the callout prefix
        if let Some(ref id_val) = id {
            if let Some(prefix) = callout_prefix(&callout_class) {
                if id_val.starts_with(&format!("{}-", prefix)) {
                    let mut counters = self.counters.borrow_mut();
                    let count = counters.entry(callout_type.to_string()).or_insert(0);
                    *count += 1;
                    vars.insert("number".to_string(), count.to_string());
                }
            }
        }

        let title = vars.get("title").filter(|t| !t.is_empty())
            .cloned()
            .unwrap_or_else(|| default_title.to_string());
        vars.insert("title".to_string(), title.clone());
        vars.insert("icon".to_string(), icon.to_string());

        // Include number in header if present
        let header = if let Some(num) = vars.get("number") {
            format!("{} {} {}", icon, title, num)
        } else {
            format!("{} {}", icon, title)
        };
        vars.insert("header".to_string(), header);

        let collapse = vars.get("collapse").map(|v| v == "true").unwrap_or(false);
        vars.insert("collapse".to_string(), collapse.to_string());

        let appearance = vars.get("appearance").cloned().unwrap_or_else(|| "default".to_string());
        vars.insert("appearance".to_string(), appearance.clone());

        FilterResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_callout(class: &str, id: Option<&str>) -> Element {
        Element::Div {
            classes: vec![class.to_string()],
            id: id.map(|s| s.to_string()),
            attrs: HashMap::new(),
            children: vec![],
        }
    }

    #[test]
    fn test_callout_no_id_no_number() {
        let filter = CalloutFilter::new();
        let element = make_callout("callout-tip", None);
        let mut vars = HashMap::new();
        filter.apply(&element, "html", &mut vars);
        assert!(!vars.contains_key("number"));
        assert_eq!(vars["header"], "\u{1f4a1} Tip");
    }

    #[test]
    fn test_callout_with_id_gets_number() {
        let filter = CalloutFilter::new();
        let element = make_callout("callout-tip", Some("tip-example"));
        let mut vars = HashMap::new();
        filter.apply(&element, "html", &mut vars);
        assert_eq!(vars["number"], "1");
        assert_eq!(vars["header"], "\u{1f4a1} Tip 1");
    }

    #[test]
    fn test_callout_sequential_numbering() {
        let filter = CalloutFilter::new();

        let mut vars1 = HashMap::new();
        filter.apply(&make_callout("callout-tip", Some("tip-first")), "html", &mut vars1);
        assert_eq!(vars1["number"], "1");

        let mut vars2 = HashMap::new();
        filter.apply(&make_callout("callout-tip", Some("tip-second")), "html", &mut vars2);
        assert_eq!(vars2["number"], "2");
    }

    #[test]
    fn test_callout_per_type_numbering() {
        let filter = CalloutFilter::new();

        let mut vars_tip = HashMap::new();
        filter.apply(&make_callout("callout-tip", Some("tip-a")), "html", &mut vars_tip);
        assert_eq!(vars_tip["number"], "1");

        let mut vars_note = HashMap::new();
        filter.apply(&make_callout("callout-note", Some("nte-a")), "html", &mut vars_note);
        assert_eq!(vars_note["number"], "1");
    }

    #[test]
    fn test_callout_wrong_prefix_no_number() {
        let filter = CalloutFilter::new();
        // tip callout with nte- prefix should not get numbered
        let element = make_callout("callout-tip", Some("nte-wrong"));
        let mut vars = HashMap::new();
        filter.apply(&element, "html", &mut vars);
        assert!(!vars.contains_key("number"));
    }

    #[test]
    fn test_callout_prefix_mapping() {
        assert_eq!(callout_prefix("callout-note"), Some("nte"));
        assert_eq!(callout_prefix("callout-tip"), Some("tip"));
        assert_eq!(callout_prefix("callout-warning"), Some("wrn"));
        assert_eq!(callout_prefix("callout-important"), Some("imp"));
        assert_eq!(callout_prefix("callout-caution"), Some("cau"));
        assert_eq!(callout_prefix("callout-other"), None);
    }
}
