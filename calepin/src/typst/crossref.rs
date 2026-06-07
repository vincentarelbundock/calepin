use anyhow::{anyhow, Result};

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
}

impl CrossrefLabel {
    pub fn to_doc(&self) -> crate::typst::model::CrossrefLabelDoc {
        crate::typst::model::CrossrefLabelDoc {
            kind: self.kind.as_str().to_string(),
            name: self.name.clone(),
        }
    }
}

/// Classify one label name by its prefix. Strict: an unrecognized prefix is an error.
pub fn classify_label(name: &str) -> Result<CrossrefLabel> {
    for (prefix, kind) in PREFIXES {
        if name.starts_with(prefix) && name.len() > prefix.len() {
            return Ok(CrossrefLabel { kind, name: name.to_string() });
        }
    }
    Err(anyhow!(
        "label `{}` has no recognized cross-reference prefix (expected one of: fig-, tbl-, lst-)",
        name
    ))
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
                &name[..name.find('-').map(|i| i + 1).unwrap_or(name.len())]
            ));
        }
        seen_kinds.push(label.kind);
        labels.push(label);
    }
    Ok(labels)
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
}
