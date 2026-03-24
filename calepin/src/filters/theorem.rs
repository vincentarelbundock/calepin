// Theorem filter: auto-numbers theorem-type environments.
//
// - TheoremFilter::apply() — Increment per-type counter, inject {{number}} template var.
// - theorem_prefix()       — Map class name to cross-ref prefix (e.g. "theorem" → "thm").

use std::cell::RefCell;
use std::collections::HashMap;

use super::{Filter, FilterResult};
use crate::types::Element;

const THEOREM_TYPES: &[(&str, &str)] = &[
    ("theorem", "thm"), ("lemma", "lem"), ("corollary", "cor"),
    ("proposition", "prp"), ("conjecture", "cnj"), ("definition", "def"),
    ("example", "exm"), ("exercise", "exr"), ("solution", "sol"),
    ("remark", "rem"), ("algorithm", "alg"),
];

/// Return the cross-reference prefix for a theorem class, if any.
pub fn theorem_prefix(class: &str) -> Option<&'static str> {
    THEOREM_TYPES.iter().find(|(c, _)| *c == class).map(|(_, p)| *p)
}

pub struct TheoremFilter {
    counters: RefCell<HashMap<String, usize>>,
}

impl TheoremFilter {
    pub fn new() -> Self {
        Self { counters: RefCell::new(HashMap::new()) }
    }
}

impl Filter for TheoremFilter {
    fn apply(&self, element: &Element, _format: &str, vars: &mut HashMap<String, String>) -> FilterResult {
        let classes = match element {
            Element::Div { classes, .. } => classes,
            _ => return FilterResult::Pass,
        };

        for cls in classes {
            if theorem_prefix(cls).is_some() {
                let mut counters = self.counters.borrow_mut();
                let count = counters.entry(cls.clone()).or_insert(0);
                *count += 1;
                vars.insert("number".to_string(), count.to_string());
                vars.insert("type_class".to_string(), cls.clone());
                return FilterResult::Continue;
            }
        }

        FilterResult::Pass
    }
}
