//! Metadata merge and override logic.

use super::{Metadata, TocConfig};
use crate::value;

/// Merge two `Option<T>` where `Some(inner)` structs are merged field-by-field.
macro_rules! merge_option_struct {
    ($user:expr, $builtin:expr, { $( $field:ident ),* $(,)? }) => {
        match (&$user, &$builtin) {
            (Some(u), Some(b)) => Some({
                let mut merged = b.clone();
                $(
                    if u.$field.is_some() {
                        merged.$field = u.$field.clone();
                    }
                )*
                merged
            }),
            (Some(u), None) => Some(u.clone()),
            (None, b) => b.clone(),
        }
    };
}

impl Metadata {
    /// Merge two Metadata values: `overlay` fields win over `self` (base).
    /// For Option fields, overlay replaces base when Some.
    /// For Vec fields, overlay replaces base when non-empty.
    /// For bool fields, overlay wins if it differs from default.
    pub fn merge(mut self, overlay: Metadata) -> Metadata {
        macro_rules! merge_opt {
            ($field:ident) => {
                if overlay.$field.is_some() { self.$field = overlay.$field; }
            };
        }
        macro_rules! merge_vec {
            ($field:ident) => {
                if !overlay.$field.is_empty() { self.$field = overlay.$field; }
            };
        }

        // Document identity
        merge_opt!(title);
        merge_opt!(subtitle);
        if !overlay.authors.is_empty() { self.authors = overlay.authors; }
        if !overlay.affiliations.is_empty() { self.affiliations = overlay.affiliations; }
        merge_opt!(date);
        merge_opt!(abstract_text);
        merge_vec!(keywords);
        merge_opt!(copyright);
        merge_opt!(license);
        merge_opt!(citation);
        merge_vec!(funding);
        merge_opt!(appendix_style);

        // Rendering
        merge_opt!(target);
        merge_opt!(theme);
        if overlay.number_sections { self.number_sections = true; }
        merge_opt!(date_format);
        merge_vec!(bibliography);
        merge_opt!(csl);
        merge_vec!(plugins);
        if overlay.convert_math { self.convert_math = true; }
        merge_opt!(html_math_method);

        // Project-level
        merge_opt!(output);
        merge_opt!(lang);
        merge_opt!(url);
        merge_opt!(favicon);
        self.logo = merge_option_struct!(overlay.logo, self.logo, { light, dark, text });
        merge_opt!(orchestrator);
        if overlay.global_crossref { self.global_crossref = true; }
        merge_vec!(static_dirs);
        merge_opt!(embed_resources);

        // Config sections: merge field-by-field within each struct
        self.preview_port = overlay.preview_port.or(self.preview_port);
        self.dpi = overlay.dpi.or(self.dpi);
        self.math = overlay.math.or(self.math);
        self.highlight = merge_option_struct!(overlay.highlight, self.highlight, { light, dark });
        self.figure = merge_option_struct!(overlay.figure, self.figure, { fig_width, fig_height, out_width, out_height, fig_asp, device, alignment });
        self.execute = merge_option_struct!(overlay.execute, self.execute, { cache, eval, echo, include, warning, message, error, comment, results, timeout });
        self.toc = merge_option_struct!(overlay.toc, self.toc, { enabled, depth, title });
        self.callout = merge_option_struct!(overlay.callout, self.callout, { appearance, note, tip, warning, important, caution, icon_note, icon_tip, icon_warning, icon_important, icon_caution });
        self.video = merge_option_struct!(overlay.video, self.video, { width, height, title });
        self.placeholder = merge_option_struct!(overlay.placeholder, self.placeholder, { width, height, color });
        self.lipsum = merge_option_struct!(overlay.lipsum, self.lipsum, { paragraphs });
        self.layout = merge_option_struct!(overlay.layout, self.layout, { valign, columns, rows });
        self.labels = merge_option_struct!(overlay.labels, self.labels, { abstract_title, keywords, appendix, citation, reuse, funding, copyright, listing, proof, contents });

        // Collection structure
        merge_vec!(contents);
        merge_vec!(languages);
        if !overlay.targets.is_empty() { self.targets = overlay.targets; }
        merge_vec!(post);

        // Extra variables: overlay keys win
        for (k, v) in overlay.var {
            self.var.insert(k, v);
        }

        self
    }

    /// Apply command-line overrides in "key=value" format.
    pub fn apply_overrides(&mut self, overrides: &[String]) {
        for item in overrides {
            // Support append syntax: "key+=value" appends to list fields
            if let Some((key, value)) = item.split_once("+=") {
                let key = key.trim();
                let value = value.trim();
                match key {
                    "bibliography" => {
                        self.bibliography.push(value.to_string());
                    }
                    _ => {}
                }
                continue;
            }
            let (raw_key, value) = match item.split_once('=') {
                Some((k, v)) => (k.trim(), v.trim()),
                None => continue,
            };
            let key = crate::util::normalize_key(raw_key);
            match key.as_str() {
                "title" => self.title = Some(value.to_string()),
                "subtitle" => self.subtitle = Some(value.to_string()),
                "author" => {
                    self.authors = vec![super::Author {
                        name: super::AuthorName {
                            literal: value.to_string(),
                            given: None,
                            family: None,
                        },
                        ..Default::default()
                    }];
                }
                "date" => self.date = Some(value.to_string()),
                "abstract" => self.abstract_text = Some(value.to_string()),
                "target" | "format" => self.target = Some(value.to_string()),
                "number_sections" => self.number_sections = value::coerce_value(value).as_bool() == Some(true),
                "toc" => self.toc = Some(TocConfig { enabled: Some(value::coerce_value(value).as_bool() == Some(true)), ..Default::default() }),
                "bibliography" => self.bibliography = vec![value.to_string()],
                "csl" => self.csl = Some(value.to_string()),
                _ => {
                    // Support dot-notation for nested keys: "a.b.c=val"
                    let parts: Vec<&str> = key.split('.').collect();
                    let coerced = value::coerce_value(value);
                    if parts.len() == 1 {
                        self.var.insert(key.to_string(), coerced);
                    } else {
                        let leaf = coerced;
                        let nested = value::build_nested_value(&parts, leaf);
                        value::merge_value(&mut self.var, parts[0], nested);
                    }
                }
            }
        }
    }
}
