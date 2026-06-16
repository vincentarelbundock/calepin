use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::Value;

use crate::utils::template::no_autoescape_env;

use super::bundle::{require_builtin, CALEPIN};
use super::{dir_theme_name, validate_theme_dir, ThemeSelection, DEFAULT_THEME_NAME};

const NOTEBOOK_TEMPLATE: &str = "notebook.typ.jinja";
const LEGACY_PAGED_TEMPLATE: &str = "paged.typ.jinja";

#[derive(Debug, Clone)]
pub struct NotebookTemplateContext {
    pub input_path: String,
    pub input_dir: String,
    pub input_stem: String,
    pub body: String,
    pub page_meta: Value,
    pub params: Value,
}

impl Default for NotebookTemplateContext {
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
pub struct NotebookSource {
    pub source: String,
    pub owns_body: bool,
}

#[derive(Serialize)]
struct NotebookTemplateRenderContext<'a> {
    theme: &'a str,
    target: &'static str,
    document: NotebookDocumentContext<'a>,
    params: &'a Value,
}

#[derive(Serialize)]
struct NotebookDocumentContext<'a> {
    path: &'a str,
    dir: &'a str,
    stem: &'a str,
    body: &'a str,
    meta: &'a Value,
}

/// The Typst source to inject while rendering a notebook. `None` disables
/// notebook theming entirely. An empty `notebook.typ.jinja` file is rendered
/// as-is (no styling), while an absent file falls back to the default bundle's
/// `notebook.typ.jinja`.
pub fn notebook_source(
    selection: &ThemeSelection,
    context: &NotebookTemplateContext,
) -> Result<Option<NotebookSource>> {
    let render = |name: &str, template_name: &str, source: String| {
        let owns_body = source.contains("document.body");
        let target = if template_name == LEGACY_PAGED_TEMPLATE {
            "paged"
        } else {
            "notebook"
        };
        render_notebook_template(name, template_name, target, source, context)
            .map(|source| NotebookSource { source, owns_body })
    };
    match selection {
        ThemeSelection::Disabled => Ok(None),
        ThemeSelection::Default => {
            render(DEFAULT_THEME_NAME, NOTEBOOK_TEMPLATE, default_source()).map(Some)
        }
        ThemeSelection::Builtin(name) => {
            let bundle = require_builtin(name)?;
            let source = bundle
                .file(NOTEBOOK_TEMPLATE)
                .or_else(|| bundle.file(LEGACY_PAGED_TEMPLATE))
                .map(str::to_string)
                .unwrap_or_else(default_source);
            render(bundle.name, NOTEBOOK_TEMPLATE, source).map(Some)
        }
        ThemeSelection::Dir(dir) => {
            validate_theme_dir(dir)?;
            let template_path = dir.join(NOTEBOOK_TEMPLATE);
            let legacy_template_path = dir.join(LEGACY_PAGED_TEMPLATE);
            if template_path.is_file() {
                let source = std::fs::read_to_string(&template_path)
                    .with_context(|| format!("failed to read {}", template_path.display()))?;
                let name = dir_theme_name(dir);
                render(&name, NOTEBOOK_TEMPLATE, source).map(Some)
            } else if legacy_template_path.is_file() {
                let source = std::fs::read_to_string(&legacy_template_path).with_context(|| {
                    format!("failed to read {}", legacy_template_path.display())
                })?;
                let name = dir_theme_name(dir);
                render(&name, LEGACY_PAGED_TEMPLATE, source).map(Some)
            } else {
                render(DEFAULT_THEME_NAME, NOTEBOOK_TEMPLATE, default_source()).map(Some)
            }
        }
    }
}

fn default_source() -> String {
    CALEPIN
        .file(NOTEBOOK_TEMPLATE)
        .map(str::to_string)
        .expect("builtin calepin bundle ships notebook.typ.jinja")
}

fn render_notebook_template(
    theme_name: &str,
    template_name: &str,
    target: &'static str,
    source: String,
    context: &NotebookTemplateContext,
) -> Result<String> {
    let mut env = no_autoescape_env();
    env.add_template_owned(template_name, source)
        .map_err(|error| notebook_template_error(theme_name, template_name, error))?;
    let template = env
        .get_template(template_name)
        .map_err(|error| notebook_template_error(theme_name, template_name, error))?;
    template
        .render(NotebookTemplateRenderContext {
            theme: theme_name,
            target,
            document: NotebookDocumentContext {
                path: &context.input_path,
                dir: &context.input_dir,
                stem: &context.input_stem,
                body: &context.body,
                meta: &context.page_meta,
            },
            params: &context.params,
        })
        .map_err(|error| notebook_template_error(theme_name, template_name, error))
}

fn notebook_template_error(
    name: &str,
    template_name: &str,
    error: minijinja::Error,
) -> anyhow::Error {
    anyhow!("theme `{name}` {template_name}: {error}")
}
