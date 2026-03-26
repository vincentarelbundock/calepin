//! Span filter: processes bracketed spans `[content]{.class key=val}`.
//!
//! Built-in span types:
//! - `[]{.pagebreak}` — format-specific page break
//! - `[]{.video url="..."}` — video embed (YouTube/Vimeo auto-detected)
//! - `[]{.placeholder width=600 height=400}` — placeholder image
//! - `[]{.lorem paragraphs=3}` — lorem ipsum text generation

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::registry::ModuleRegistry;

static RE_BRACKETED_SPAN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[([^\]]*)\]\{([^}]+)\}").unwrap()
});

/// Process all bracketed spans in a text block.
/// Returns the text with spans replaced by their rendered output.
pub fn render(
    text: &str,
    format: &str,
    registry: &ModuleRegistry,
    raw_fragments: &std::cell::RefCell<Vec<String>>,
    defaults: &crate::config::Metadata,
    resolve_partial: &dyn Fn(&str) -> Option<String>,
    template_env: &crate::render::template::TemplateEnv,
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

        // Module dispatch for spans (pagebreak, video, placeholder, lorem, etc.)
        {
            use crate::registry::ModuleKind;
            let empty_attrs = HashMap::new();
            let matching = registry.matching_modules(&classes, &empty_attrs, id.as_deref(), format, "span");
            for (plugin, _) in &matching {
                if let ModuleKind::Span(ref handler) = plugin.kind {
                    if let Some(output) = handler.render(&kv, content, format, defaults) {
                        return wrap_output(format, raw_fragments, output);
                    }
                }
            }
        }

        // Render inline markdown in span content (e.g. **bold**, *italic*)
        let rendered_content = crate::render::convert::render_inline(content, format);

        let mut vars = HashMap::new();
        for (k, v) in &kv {
            vars.insert(k.clone(), v.clone());
        }
        vars.insert("base".to_string(), format.to_string());
        vars.insert("writer".to_string(), format.to_string());
        vars.insert("content".to_string(), rendered_content.clone());
        vars.insert("class".to_string(), first_class.to_string());
        vars.insert("classes".to_string(), classes.join(" "));
        if let Some(ref id_val) = id {
            vars.insert("id".to_string(), id_val.clone());
        } else {
            vars.insert("id".to_string(), String::new());
        }

        // Template lookup
        if !first_class.is_empty() {
            if let Some(tpl) = resolve_partial(first_class) {
                let rendered = template_env.render_dynamic(first_class, &tpl, &vars);
                return wrap_output(format, raw_fragments, rendered);
            }
        }

        // Default fallback: use span template
        let tpl = resolve_partial("span")
            .unwrap_or_else(|| crate::render::elements::resolve_builtin_partial("span", format).unwrap_or("").to_string());
        let output = template_env.render_dynamic("span", &tpl, &vars);
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
        _ => crate::render::convert::wrap_raw(&mut raw_fragments.borrow_mut(), output),
    }
}

