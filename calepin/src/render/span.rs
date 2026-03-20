//! Span filter: processes bracketed spans `[content]{.class key=val}`.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::registry::{PluginKind, PluginRegistry};

static RE_BRACKETED_SPAN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[([^\]]+)\]\{([^}]+)\}").unwrap()
});

/// Process all bracketed spans in a text block.
/// Returns the text with spans replaced by their rendered output.
pub fn render(
    text: &str,
    format: &str,
    registry: &PluginRegistry,
    raw_fragments: &std::cell::RefCell<Vec<String>>,
    resolve_template: &dyn Fn(&str) -> Option<String>,
) -> String {
    raw_fragments.borrow_mut().clear();

    RE_BRACKETED_SPAN.replace_all(text, |caps: &regex::Captures| {
        let content = &caps[1];
        let attr_str = &caps[2];
        let (classes, id, kv) = crate::parse::blocks::parse_attributes(attr_str);

        if !crate::engines::content_is_visible(&classes, &kv, format, None) {
            return String::new();
        }

        let first_class = classes.first().map(|s| s.as_str()).unwrap_or("");

        // Render inline markdown in span content (e.g. **bold**, *italic*)
        let rendered_content = crate::render::markdown::render_inline(content, format);

        let mut vars = HashMap::new();
        for (k, v) in &kv {
            vars.insert(k.clone(), v.clone());
        }
        vars.insert("content".to_string(), rendered_content.clone());
        vars.insert("class".to_string(), first_class.to_string());
        vars.insert("classes".to_string(), classes.join(" "));
        if let Some(ref id_val) = id {
            vars.insert("id".to_string(), id_val.clone());
            vars.insert("id-attr".to_string(), format!(" id=\"{}\"", id_val));
        } else {
            vars.insert("id".to_string(), String::new());
            vars.insert("id-attr".to_string(), String::new());
        }

        // Plugin dispatch via registry
        let empty_attrs = HashMap::new();
        let matching = registry.matching_filters(&classes, &empty_attrs, id.as_deref(), format, "span");

        for (plugin, filter_spec) in &matching {
            match &plugin.kind {
                PluginKind::BuiltinFilter(filter) => {
                    let span_element = crate::types::Element::Text { content: content.to_string() };
                    match filter.apply(&span_element, format, &mut vars) {
                        crate::filters::FilterResult::Rendered(output) => {
                            return wrap_output(format, raw_fragments, output);
                        }
                        _ => {}
                    }
                }
                PluginKind::Subprocess { .. } | PluginKind::PersistentSubprocess { .. } => {
                    if let Some(output) = registry.call_subprocess_filter(
                        plugin,
                        filter_spec,
                        "span",
                        content,
                        &classes,
                        id.as_deref().unwrap_or(""),
                        format,
                        &kv,
                    ) {
                        return wrap_output(format, raw_fragments, output);
                    }
                }
                PluginKind::BuiltinStructural(_) => {}
            }
        }

        // Template lookup
        if !first_class.is_empty() {
            if let Some(tpl) = resolve_template(first_class) {
                let rendered = crate::render::template::apply_template(&tpl, &vars);
                return wrap_output(format, raw_fragments, rendered);
            }
        }

        // Default fallback
        let class_attr = if classes.is_empty() {
            String::new()
        } else {
            format!(" class=\"{}\"", classes.join(" "))
        };
        let id_attr = id
            .as_ref()
            .map_or(String::new(), |v| format!(" id=\"{}\"", v));
        let output = match format {
            "html" => format!("<span{}{}>{}</span>", id_attr, class_attr, rendered_content),
            "latex" => format!("\\text{{{}}}", rendered_content),
            "typst" => format!("[{}]", rendered_content),
            _ => rendered_content.to_string(),
        };
        wrap_output(format, raw_fragments, output)
    })
    .to_string()
}

/// Wrap non-HTML output in raw markers to protect from markdown conversion.
fn wrap_output(
    format: &str,
    raw_fragments: &std::cell::RefCell<Vec<String>>,
    output: String,
) -> String {
    match format {
        "html" => output,
        _ => crate::render::markdown::wrap_raw(&mut raw_fragments.borrow_mut(), output),
    }
}
