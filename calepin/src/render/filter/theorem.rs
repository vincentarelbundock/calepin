// Theorem utilities for cross-reference validation.
//
// The TheoremPlugin (in plugins/registry.rs) handles auto-numbering.
// This module provides the prefix mapping used by div.rs for ID validation.

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
