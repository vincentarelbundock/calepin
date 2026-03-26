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

        // Built-in span types (handled before generic template lookup)
        match first_class {
            "pagebreak" => return render_pagebreak(format, raw_fragments, resolve_partial),
            "video" => return render_video(format, raw_fragments, resolve_partial, &kv, defaults),
            "placeholder" => return render_placeholder(format, raw_fragments, resolve_partial, &kv, defaults),
            "lorem" => return render_lorem(&kv, defaults),
            _ => {}
        }

        // Render inline markdown in span content (e.g. **bold**, *italic*)
        let rendered_content = crate::render::convert::render_inline(content, format);

        let mut vars = HashMap::new();
        for (k, v) in &kv {
            vars.insert(k.clone(), v.clone());
        }
        vars.insert("base".to_string(), format.to_string());
        vars.insert("engine".to_string(), format.to_string());
        vars.insert("content".to_string(), rendered_content.clone());
        vars.insert("class".to_string(), first_class.to_string());
        vars.insert("classes".to_string(), classes.join(" "));
        if let Some(ref id_val) = id {
            vars.insert("id".to_string(), id_val.clone());
        } else {
            vars.insert("id".to_string(), String::new());
        }

        // Plugin dispatch via registry
        let empty_attrs = HashMap::new();
        let _matching = registry.matching_modules(&classes, &empty_attrs, id.as_deref(), format, "span");

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

// ---------------------------------------------------------------------------
// Built-in span handlers
// ---------------------------------------------------------------------------

fn render_shortcode_template(
    name: &str,
    format: &str,
    vars: &HashMap<String, String>,
    fallback: &str,
    resolve_partial: &dyn Fn(&str) -> Option<String>,
) -> String {
    if let Some(tpl) = resolve_partial(name) {
        crate::render::template::apply_template(&tpl, vars)
    } else if let Some(tpl) = crate::render::elements::resolve_builtin_partial(name, format) {
        crate::render::template::apply_template(tpl, vars)
    } else {
        fallback.to_string()
    }
}

fn render_pagebreak(
    format: &str,
    raw_fragments: &std::cell::RefCell<Vec<String>>,
    resolve_partial: &dyn Fn(&str) -> Option<String>,
) -> String {
    let vars = HashMap::new();
    let output = render_shortcode_template("pagebreak", format, &vars, "\u{0C}", resolve_partial);
    wrap_output(format, raw_fragments, output)
}

fn render_video(
    format: &str,
    raw_fragments: &std::cell::RefCell<Vec<String>>,
    resolve_partial: &dyn Fn(&str) -> Option<String>,
    kv: &HashMap<String, String>,
    defaults: &crate::config::Metadata,
) -> String {
    let url = match kv.get("url") {
        Some(u) => u.as_str(),
        None => {
            cwarn!("[]{{.video}} requires a url attribute");
            return String::new();
        }
    };

    let vdefs = defaults.video.as_ref();
    let default_width = vdefs.and_then(|v| v.width.clone()).unwrap_or_else(|| "100%".to_string());
    let default_height = vdefs.and_then(|v| v.height.clone()).unwrap_or_else(|| "400".to_string());
    let default_title = vdefs.and_then(|v| v.title.clone()).unwrap_or_else(|| "Video".to_string());

    let width = kv.get("width").map(|s| s.as_str()).unwrap_or(&default_width);
    let height = kv.get("height").map(|s| s.as_str()).unwrap_or(&default_height);
    let title = kv.get("title").map(|s| s.as_str()).unwrap_or(&default_title);

    // YouTube/Vimeo URL normalization
    let embed_url = if url.contains("youtube.com/watch") || url.contains("youtu.be") {
        let id = url
            .split("v=").nth(1).map(|s| s.split('&').next().unwrap_or(s))
            .or_else(|| url.split("youtu.be/").nth(1).map(|s| s.split('?').next().unwrap_or(s)))
            .unwrap_or(url);
        format!("https://www.youtube.com/embed/{}", id)
    } else if url.contains("vimeo.com/") {
        let id = url.rsplit('/').next().unwrap_or(url);
        format!("https://player.vimeo.com/video/{}", id)
    } else {
        url.to_string()
    };

    let is_embed = embed_url.contains("youtube.com/embed") || embed_url.contains("player.vimeo.com");
    let mut vars = HashMap::new();
    vars.insert("src".to_string(), url.to_string());
    vars.insert("url".to_string(), embed_url);
    vars.insert("width".to_string(), width.to_string());
    vars.insert("height".to_string(), height.to_string());
    vars.insert("title".to_string(), title.to_string());
    vars.insert("is_embed".to_string(), is_embed.to_string());

    let fallback = format!("[{}]({})", title, url);
    let output = render_shortcode_template("video", format, &vars, &fallback, resolve_partial);
    wrap_output(format, raw_fragments, output)
}

fn render_placeholder(
    format: &str,
    raw_fragments: &std::cell::RefCell<Vec<String>>,
    resolve_partial: &dyn Fn(&str) -> Option<String>,
    kv: &HashMap<String, String>,
    defaults: &crate::config::Metadata,
) -> String {
    let pdefs = defaults.placeholder.as_ref();
    let default_pw = pdefs.and_then(|p| p.width.clone()).unwrap_or_else(|| "600".to_string());
    let default_ph = pdefs.and_then(|p| p.height.clone()).unwrap_or_else(|| "400".to_string());
    let default_color = pdefs.and_then(|p| p.color.clone()).unwrap_or_else(|| "#cccccc".to_string());

    let width = kv.get("width").map(|s| s.as_str()).unwrap_or(&default_pw);
    let height = kv.get("height").map(|s| s.as_str()).unwrap_or(&default_ph);
    let color = kv.get("color").map(|s| s.as_str()).unwrap_or(&default_color);
    let text = kv.get("text")
        .cloned()
        .unwrap_or_else(|| format!("{}\u{00d7}{}", width, height));

    let mut vars = HashMap::new();
    vars.insert("width".to_string(), width.to_string());
    vars.insert("height".to_string(), height.to_string());
    vars.insert("color".to_string(), crate::util::escape_html(color));
    vars.insert("text".to_string(), crate::util::escape_html(&text));

    let fallback = format!("[{} ({}x{})]", text, width, height);
    let output = render_shortcode_template("placeholder", format, &vars, &fallback, resolve_partial);
    wrap_output(format, raw_fragments, output)
}

fn render_lorem(
    kv: &HashMap<String, String>,
    defaults: &crate::config::Metadata,
) -> String {
    let default_paragraphs = defaults.lipsum.as_ref()
        .and_then(|l| l.paragraphs)
        .unwrap_or(1) as usize;

    if let Some(n) = kv.get("words").and_then(|s| s.parse::<usize>().ok()) {
        return crate::jinja::lipsum_words(n);
    }
    if let Some(n) = kv.get("sentences").and_then(|s| s.parse::<usize>().ok()) {
        return crate::jinja::lipsum::lipsum_sentences(n);
    }
    let n = kv.get("paragraphs")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(default_paragraphs);
    crate::jinja::lipsum_paragraphs(n)
}
