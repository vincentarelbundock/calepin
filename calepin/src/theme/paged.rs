use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::Value;

use crate::utils::template::no_autoescape_env;

use super::bundle::{require_builtin, CALEPIN};
use super::{dir_theme_name, validate_theme_dir, ThemeSelection, DEFAULT_THEME_NAME};

const PAGED_TEMPLATE: &str = "paged.typ.jinja";

#[derive(Debug, Clone)]
pub struct PagedTemplateContext {
    pub input_path: String,
    pub input_dir: String,
    pub input_stem: String,
    pub body: String,
    pub page_meta: Value,
    pub params: Value,
}

impl Default for PagedTemplateContext {
    fn default() -> Self {
        Self {
            input_path: String::new(),
            input_dir: String::new(),
            input_stem: String::new(),
            body: String::new(),
            page_meta: Value::Null,
            params: Value::Object(serde_json::Map::new()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagedSource {
    pub source: String,
    pub owns_body: bool,
}

#[derive(Serialize)]
struct PagedTemplateRenderContext<'a> {
    theme: &'a str,
    target: &'static str,
    document: PagedDocumentContext<'a>,
    params: &'a Value,
}

#[derive(Serialize)]
struct PagedDocumentContext<'a> {
    path: &'a str,
    dir: &'a str,
    stem: &'a str,
    body: &'a str,
    meta: &'a Value,
}

/// The Typst source to inject for paged output. `None` disables paged
/// theming entirely. An empty paged.typ.jinja file is rendered as-is (no
/// styling), while an absent file falls back to the default bundle's
/// paged.typ.jinja.
pub fn paged_source(
    selection: &ThemeSelection,
    context: &PagedTemplateContext,
) -> Result<Option<PagedSource>> {
    let render = |name: &str, source: String| {
        let owns_body = source.contains("document.body");
        render_paged_template(name, source, context).map(|source| PagedSource { source, owns_body })
    };
    match selection {
        ThemeSelection::Disabled => Ok(None),
        ThemeSelection::Default => render(DEFAULT_THEME_NAME, default_source()).map(Some),
        ThemeSelection::Builtin(name) => {
            let bundle = require_builtin(name)?;
            let source = bundle
                .file(PAGED_TEMPLATE)
                .map(str::to_string)
                .unwrap_or_else(default_source);
            render(bundle.name, source).map(Some)
        }
        ThemeSelection::Dir(dir) => {
            validate_theme_dir(dir)?;
            let template_path = dir.join(PAGED_TEMPLATE);
            if template_path.is_file() {
                let source = std::fs::read_to_string(&template_path)
                    .with_context(|| format!("failed to read {}", template_path.display()))?;
                let name = dir_theme_name(dir);
                render(&name, source).map(Some)
            } else {
                render(DEFAULT_THEME_NAME, default_source()).map(Some)
            }
        }
    }
}

fn default_source() -> String {
    CALEPIN
        .file(PAGED_TEMPLATE)
        .map(str::to_string)
        .expect("builtin calepin bundle ships paged.typ.jinja")
}

fn render_paged_template(
    theme_name: &str,
    source: String,
    context: &PagedTemplateContext,
) -> Result<String> {
    let mut env = no_autoescape_env();
    env.add_template_owned(PAGED_TEMPLATE, source)
        .map_err(|error| paged_template_error(theme_name, error))?;
    let template = env
        .get_template(PAGED_TEMPLATE)
        .map_err(|error| paged_template_error(theme_name, error))?;
    template
        .render(PagedTemplateRenderContext {
            theme: theme_name,
            target: "paged",
            document: PagedDocumentContext {
                path: &context.input_path,
                dir: &context.input_dir,
                stem: &context.input_stem,
                body: &context.body,
                meta: &context.page_meta,
            },
            params: &context.params,
        })
        .map_err(|error| paged_template_error(theme_name, error))
}

fn paged_template_error(name: &str, error: minijinja::Error) -> anyhow::Error {
    anyhow!("theme `{name}` paged.typ.jinja: {error}")
}
