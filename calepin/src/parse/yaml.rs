//! Front matter parsing (delegates to metadata module).

pub use crate::metadata::split_frontmatter as split_yaml;

#[cfg(test)]
mod tests {
    use crate::metadata::split_frontmatter;
    use crate::types::Metadata;

    #[test]
    fn test_split_yaml() {
        let text = "---\ntitle: Hello\nauthor: World\n---\n\n# Body\n\nSome text.";
        let (meta, body) = split_frontmatter(text).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Hello"));
        assert_eq!(meta.author, Some(vec!["World".to_string()]));
        assert!(body.starts_with("\n# Body"));
    }

    #[test]
    fn test_no_yaml() {
        let text = "# Just markdown\n\nNo front matter.";
        let (meta, body) = split_frontmatter(text).unwrap();
        assert!(meta.title.is_none());
        assert_eq!(body, text);
    }

    #[test]
    fn test_simple_author_string() {
        let text = "---\nauthor: Norah Jones\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.author, Some(vec!["Norah Jones".to_string()]));
        assert_eq!(meta.authors.len(), 1);
        assert_eq!(meta.authors[0].name.literal, "Norah Jones");
    }

    #[test]
    fn test_simple_author_list() {
        let text = "---\nauthor:\n  - Alice Smith\n  - Bob Lee\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.author, Some(vec!["Alice Smith".to_string(), "Bob Lee".to_string()]));
        assert_eq!(meta.authors.len(), 2);
    }

    #[test]
    fn test_rich_author_with_affiliations() {
        let text = "---\nauthor:\n  - name: Norah Jones\n    email: norah@example.com\n    orcid: 0000-0001-2345-6789\n    corresponding: true\n    affiliations:\n      - name: Carnegie Mellon University\n        city: Pittsburgh\n        region: PA\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.authors.len(), 1);
        assert_eq!(meta.authors[0].email.as_deref(), Some("norah@example.com"));
        assert_eq!(meta.authors[0].orcid.as_deref(), Some("0000-0001-2345-6789"));
        assert!(meta.authors[0].corresponding);
        assert_eq!(meta.authors[0].affiliation_ids, vec![0]);
        assert_eq!(meta.affiliations.len(), 1);
        assert_eq!(meta.affiliations[0].name.as_deref(), Some("Carnegie Mellon University"));
        assert_eq!(meta.affiliations[0].city.as_deref(), Some("Pittsburgh"));
        assert_eq!(meta.affiliations[0].region.as_deref(), Some("PA"));
        assert_eq!(meta.affiliations[0].number, 1);
    }

    #[test]
    fn test_shared_affiliations_via_ref() {
        let text = "---\nauthor:\n  - name: Alice\n    affiliations:\n      - ref: mit\n  - name: Bob\n    affiliations:\n      - ref: mit\naffiliations:\n  - id: mit\n    name: MIT\n    city: Cambridge\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.affiliations.len(), 1);
        assert_eq!(meta.authors[0].affiliation_ids, vec![0]);
        assert_eq!(meta.authors[1].affiliation_ids, vec![0]);
        assert_eq!(meta.affiliations[0].name.as_deref(), Some("MIT"));
    }

    #[test]
    fn test_multiple_affiliations() {
        let text = "---\nauthor:\n  - name: Alice\n    affiliations:\n      - MIT\n      - Stanford\n  - name: Bob\n    affiliations:\n      - Stanford\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.affiliations.len(), 2);
        assert_eq!(meta.authors[0].affiliation_ids, vec![0, 1]);
        assert_eq!(meta.authors[1].affiliation_ids, vec![1]);
    }

    #[test]
    fn test_yaml_block_scalar_with_dashes() {
        let text = "---\ntitle: Test\nabstract: |\n  some content\n  ---\n  more content\n---\nBody";
        let (meta, body) = split_frontmatter(text).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Test"));
        assert!(meta.abstract_text.as_ref().unwrap().contains("more content"));
        assert_eq!(body, "Body");
    }

    #[test]
    fn test_format_mapping() {
        let text = "---\ntitle: Test\nformat:\n  html: default\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.target.as_deref(), Some("html"));
    }

    #[test]
    fn test_format_string() {
        let text = "---\nformat: latex\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.target.as_deref(), Some("latex"));
    }

    #[test]
    fn test_bibliography_list() {
        let text = "---\nbibliography:\n  - refs.bib\n  - extra.bib\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.bibliography, vec!["refs.bib", "extra.bib"]);
    }

    #[test]
    fn test_bibliography_string() {
        let text = "---\nbibliography: refs.bib\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.bibliography, vec!["refs.bib"]);
    }

    #[test]
    fn test_toml_frontmatter() {
        let text = "---\ntitle = \"Hello\"\nauthor = \"World\"\nformat = \"html\"\n---\nBody";
        let (meta, body) = split_frontmatter(text).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Hello"));
        assert_eq!(meta.author, Some(vec!["World".to_string()]));
        assert_eq!(meta.target.as_deref(), Some("html"));
        assert_eq!(body, "Body");
    }

    #[test]
    fn test_toml_frontmatter_nested() {
        let text = "---\ntitle = \"Hello\"\n\n[calepin]\nplugins = [\"txtfmt\"]\n---\nBody";
        let (meta, _) = split_frontmatter(text).unwrap();
        assert_eq!(meta.title.as_deref(), Some("Hello"));
        assert_eq!(meta.plugins, vec!["txtfmt"]);
    }
}
