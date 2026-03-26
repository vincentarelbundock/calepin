//! Built-in Jinja functions: pagebreak, video, kbd, lipsum, placeholder.
//!
//! Format-specific output is driven by element templates in
//! `project/partials/{engine}/`, making shortcodes user-overridable.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use minijinja::{self, Value, Error, ErrorKind};

use crate::render::markers;
use crate::render::template::apply_template;
use crate::render::elements::resolve_element_partial;

use super::lipsum;

/// Register all built-in Jinja functions on the given environment.
pub(crate) fn register(
    env: &mut minijinja::Environment<'_>,
    format: &str,
    fragments: &Arc<Mutex<Vec<String>>>,
    defaults: &crate::metadata::Metadata,
) {
    register_pagebreak(env, format, fragments);
    register_video(env, format, fragments, defaults);
    register_kbd(env, format, fragments);
    register_lipsum(env, defaults);
    register_placeholder(env, format, fragments, defaults);
}

/// Render a shortcode template by name. Falls back to a plain default if
/// no template is found for the given engine.
fn render_shortcode(name: &str, engine: &str, vars: &HashMap<String, String>, fallback: &str) -> String {
    if let Some(tpl) = resolve_element_partial(name, engine) {
        apply_template(&tpl, vars)
    } else {
        fallback.to_string()
    }
}

fn register_pagebreak(
    env: &mut minijinja::Environment<'_>,
    format: &str,
    fragments: &Arc<Mutex<Vec<String>>>,
) {
    let fmt = format.to_string();
    let frags = Arc::clone(fragments);
    env.add_function("pagebreak", move |_args: &[Value]| -> Result<Value, Error> {
        let vars = HashMap::new();
        let output = render_shortcode("pagebreak", &fmt, &vars, "\u{0C}");
        Ok(Value::from_safe_string(wrap_if_needed(&output, &fmt, &frags)))
    });
}

fn register_video(
    env: &mut minijinja::Environment<'_>,
    format: &str,
    fragments: &Arc<Mutex<Vec<String>>>,
    defaults: &crate::metadata::Metadata,
) {
    let fmt = format.to_string();
    let frags = Arc::clone(fragments);
    let video_defs = defaults.video.clone();
    env.add_function("video", move |kwargs: minijinja::value::Kwargs| -> Result<Value, Error> {
        let url: &str = kwargs.get("url")
            .map_err(|_| Error::new(ErrorKind::MissingArgument, "video() requires a `url` argument"))?;
        let vdefs = video_defs.clone();
        let default_width = vdefs.as_ref().and_then(|v| v.width.clone()).unwrap_or_else(|| "100%".to_string());
        let default_height = vdefs.as_ref().and_then(|v| v.height.clone()).unwrap_or_else(|| "400".to_string());
        let default_title = vdefs.as_ref().and_then(|v| v.title.clone()).unwrap_or_else(|| "Video".to_string());
        let width: &str = kwargs.get("width").unwrap_or(&default_width);
        let height: &str = kwargs.get("height").unwrap_or(&default_height);
        let title: &str = kwargs.get("title").unwrap_or(&default_title);
        kwargs.assert_all_used()?;

        // Detect YouTube/Vimeo and convert to embed URLs
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
        let output = render_shortcode("video", &fmt, &vars, &fallback);
        Ok(Value::from_safe_string(wrap_if_needed(&output, &fmt, &frags)))
    });
}

fn register_kbd(
    env: &mut minijinja::Environment<'_>,
    format: &str,
    fragments: &Arc<Mutex<Vec<String>>>,
) {
    let fmt = format.to_string();
    let frags = Arc::clone(fragments);
    env.add_function("kbd", move |kwargs: minijinja::value::Kwargs| -> Result<Value, Error> {
        let keys_val: Value = kwargs.get("keys").unwrap_or(Value::from(Vec::<String>::new()));
        kwargs.assert_all_used()?;
        let keys: Vec<String> = keys_val.try_iter()
            .map(|iter| iter.filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();

        if keys.is_empty() {
            return Ok(Value::from(""));
        }

        // For kbd, we need to pass a list to the template. Since apply_template
        // works with string vars, we render inline using the template directly.
        let output = if let Some(tpl) = resolve_element_partial("kbd", &fmt) {
            let mut env = minijinja::Environment::new();
            env.add_template("kbd", &tpl).ok();
            if let Some(tmpl) = env.get_template("kbd").ok() {
                let ctx = minijinja::context! { keys => keys };
                tmpl.render(ctx).unwrap_or_else(|_| keys.join("+"))
            } else {
                keys.join("+")
            }
        } else {
            keys.join("+")
        };

        Ok(Value::from_safe_string(wrap_if_needed(&output, &fmt, &frags)))
    });
}

fn register_lipsum(env: &mut minijinja::Environment<'_>, defaults: &crate::metadata::Metadata) {
    let lipsum_default_paragraphs = defaults.lipsum.as_ref().and_then(|l| l.paragraphs).unwrap_or(1);
    env.add_function("lipsum", move |kwargs: minijinja::value::Kwargs| -> Result<Value, Error> {
        if let Ok(n) = kwargs.get::<u64>("words") {
            kwargs.assert_all_used()?;
            return Ok(Value::from(lipsum::lipsum_words(n as usize)));
        }
        if let Ok(n) = kwargs.get::<u64>("sentences") {
            kwargs.assert_all_used()?;
            return Ok(Value::from(lipsum::lipsum_sentences(n as usize)));
        }
        let n: u64 = kwargs.get("paragraphs").unwrap_or(lipsum_default_paragraphs);
        kwargs.assert_all_used()?;
        Ok(Value::from(lipsum::lipsum_paragraphs(n as usize)))
    });
}

fn register_placeholder(
    env: &mut minijinja::Environment<'_>,
    format: &str,
    fragments: &Arc<Mutex<Vec<String>>>,
    defaults: &crate::metadata::Metadata,
) {
    let fmt = format.to_string();
    let frags = Arc::clone(fragments);
    let placeholder_defs = defaults.placeholder.clone();
    env.add_function("placeholder", move |kwargs: minijinja::value::Kwargs| -> Result<Value, Error> {
        let pdefs = placeholder_defs.clone();
        let default_pw = pdefs.as_ref().and_then(|p| p.width.clone()).unwrap_or_else(|| "600".to_string());
        let default_ph = pdefs.as_ref().and_then(|p| p.height.clone()).unwrap_or_else(|| "400".to_string());
        let default_color = pdefs.as_ref().and_then(|p| p.color.clone()).unwrap_or_else(|| "#cccccc".to_string());
        let width: String = kwargs.get::<Value>("width")
            .map(|v| v.to_string()).unwrap_or(default_pw);
        let height: String = kwargs.get::<Value>("height")
            .map(|v| v.to_string()).unwrap_or(default_ph);
        let color: &str = kwargs.get("color").unwrap_or(&default_color);
        let text: Option<&str> = kwargs.get("text").ok();
        let text = text.map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}\u{00d7}{}", width, height));
        kwargs.assert_all_used()?;

        let mut vars = HashMap::new();
        vars.insert("width".to_string(), width.clone());
        vars.insert("height".to_string(), height.clone());
        vars.insert("color".to_string(), crate::util::escape_html(color));
        vars.insert("text".to_string(), crate::util::escape_html(&text));

        let fallback = format!("[{} ({}x{})]", text, width, height);
        let output = render_shortcode("placeholder", &fmt, &vars, &fallback);
        Ok(Value::from_safe_string(wrap_if_needed(&output, &fmt, &frags)))
    });
}

/// Wrap output in shortcode markers if needed (for LaTeX/Typst protection).
fn wrap_if_needed(output: &str, format: &str, fragments: &Arc<Mutex<Vec<String>>>) -> String {
    match format {
        "html" | "markdown" | "md" => output.to_string(),
        _ => {
            let mut frags = fragments.lock().unwrap();
            markers::wrap_shortcode_raw(&mut frags, output.to_string())
        }
    }
}
