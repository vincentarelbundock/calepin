use anyhow::{anyhow, Result};

use crate::typst::model::CrossrefLabelDoc;

/// Recognized cross-reference kinds, selected by a label-name prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossrefKind {
    Fig,
    Tbl,
    Lst,
}

/// The fixed, ordered prefix table. Extend here to add a kind.
const PREFIXES: [(&str, CrossrefKind); 3] = [
    ("fig-", CrossrefKind::Fig),
    ("tbl-", CrossrefKind::Tbl),
    ("lst-", CrossrefKind::Lst),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossrefLabel {
    pub kind: CrossrefKind,
    /// Full label name including the prefix, e.g. `fig-plot`.
    pub name: String,
}

impl CrossrefKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CrossrefKind::Fig => "fig",
            CrossrefKind::Tbl => "tbl",
            CrossrefKind::Lst => "lst",
        }
    }

    fn prefix(self) -> &'static str {
        prefix_for_kind(self)
    }
}

impl CrossrefLabel {
    pub fn to_doc(&self) -> CrossrefLabelDoc {
        CrossrefLabelDoc {
            kind: self.kind.as_str().to_string(),
            name: self.name.clone(),
        }
    }
}

/// Classify one label name by its prefix. Strict: an unrecognized prefix is an error.
pub fn classify_label(name: &str) -> Result<CrossrefLabel> {
    let Some((prefix, kind)) = matching_prefix(name) else {
        return Err(anyhow!(
            "label `{}` has no recognized cross-reference prefix (expected one of: {})",
            name,
            expected_prefixes()
        ));
    };
    if name.len() == prefix.len() {
        return Err(anyhow!(
            "label `{}` has cross-reference prefix `{}` but no label name",
            name,
            prefix
        ));
    }
    Ok(CrossrefLabel {
        kind,
        name: name.to_string(),
    })
}

pub fn has_crossref_prefix(name: &str) -> bool {
    matches!(matching_prefix(name), Some((prefix, _)) if name.len() > prefix.len())
}

/// Classify a list of names; reject empty lists and repeated kinds.
pub fn parse_label_names(names: &[String]) -> Result<Vec<CrossrefLabel>> {
    if names.is_empty() {
        return Err(anyhow!("label list is empty"));
    }
    let mut labels = Vec::with_capacity(names.len());
    let mut seen_kinds: Vec<CrossrefKind> = Vec::new();
    for name in names {
        let label = classify_label(name)?;
        if seen_kinds.contains(&label.kind) {
            return Err(anyhow!(
                "label list has more than one `{}` entry; use one label per kind",
                label.kind.prefix()
            ));
        }
        seen_kinds.push(label.kind);
        labels.push(label);
    }
    Ok(labels)
}

/// Convert labels with a recognized cross-reference prefix into serialized docs.
///
/// Plain labels are valid chunk ids and are ignored here. Labels that start
/// with a cross-reference prefix are validated strictly.
pub fn parse_prefixed_label_docs(names: &[String]) -> Result<Vec<CrossrefLabelDoc>> {
    let prefixed = names
        .iter()
        .filter(|name| matching_prefix(name).is_some())
        .cloned()
        .collect::<Vec<_>>();
    if prefixed.is_empty() {
        return Ok(Vec::new());
    }

    parse_label_names(&prefixed)
        .map(|labels| labels.into_iter().map(|label| label.to_doc()).collect())
}

fn matching_prefix(name: &str) -> Option<(&'static str, CrossrefKind)> {
    PREFIXES
        .iter()
        .copied()
        .find(|(prefix, _)| name.starts_with(prefix))
}

fn prefix_for_kind(kind: CrossrefKind) -> &'static str {
    PREFIXES
        .iter()
        .find_map(|(prefix, candidate)| (*candidate == kind).then_some(*prefix))
        .expect("cross-reference kind missing from prefix table")
}

fn expected_prefixes() -> String {
    PREFIXES
        .iter()
        .map(|(prefix, _)| *prefix)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_recognized_prefixes() {
        assert_eq!(classify_label("fig-x").unwrap().kind, CrossrefKind::Fig);
        assert_eq!(classify_label("tbl-x").unwrap().kind, CrossrefKind::Tbl);
        assert_eq!(classify_label("lst-x").unwrap().kind, CrossrefKind::Lst);
    }

    #[test]
    fn keeps_full_name_including_prefix() {
        assert_eq!(classify_label("fig-plot").unwrap().name, "fig-plot");
    }

    #[test]
    fn rejects_unprefixed_label() {
        let err = classify_label("myplot").unwrap_err().to_string();
        assert!(err.contains("myplot"), "{err}");
        assert!(err.contains("fig-"), "{err}");
    }

    #[test]
    fn rejects_empty_prefixed_label_name() {
        for name in ["fig-", "tbl-", "lst-"] {
            let err = classify_label(name).unwrap_err().to_string();
            assert!(err.contains(name), "{err}");
            assert!(err.contains("no label name"), "{err}");
        }
    }

    #[test]
    fn parses_single_string_into_one_label() {
        let labels = parse_label_names(&["fig-x".to_string()]).unwrap();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].kind, CrossrefKind::Fig);
    }

    #[test]
    fn parses_distinct_kinds_list() {
        let labels = parse_label_names(&["fig-x".to_string(), "lst-y".to_string()]).unwrap();
        assert_eq!(labels.len(), 2);
    }

    #[test]
    fn rejects_duplicate_kinds() {
        let err = parse_label_names(&["fig-a".to_string(), "fig-b".to_string()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("fig"), "{err}");
    }

    #[test]
    fn parses_prefixed_label_docs_while_ignoring_plain_ids() {
        let labels = parse_prefixed_label_docs(&[
            "plot".to_string(),
            "fig-plot".to_string(),
            "lst-code".to_string(),
        ])
        .unwrap();
        assert_eq!(
            labels,
            vec![
                crate::typst::model::CrossrefLabelDoc {
                    kind: "fig".to_string(),
                    name: "fig-plot".to_string(),
                },
                crate::typst::model::CrossrefLabelDoc {
                    kind: "lst".to_string(),
                    name: "lst-code".to_string(),
                },
            ]
        );
    }

    #[test]
    fn prefixed_label_docs_reject_empty_label_name() {
        let err = parse_prefixed_label_docs(&["plot".to_string(), "fig-".to_string()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("fig-"), "{err}");
        assert!(err.contains("no label name"), "{err}");
    }
}
