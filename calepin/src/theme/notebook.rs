use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::Value;

use crate::utils::template::no_autoescape_env;

use super::bundle::require_builtin;
use super::{dir_theme_name, resolve_theme_chain, ThemeLayer, ThemeSelection};

const NOTEBOOK_TEMPLATE: &str = "layouts/notebook.typ";
const REMOVED_NOTEBOOK_TEMPLATE: &str = "notebook.typ.jinja";
const REMOVED_PAGED_TEMPLATE: &str = "paged.typ.jinja";

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
/// notebook theming entirely. Only `layouts/notebook.typ` is supported.
pub fn notebook_source(
    selection: &ThemeSelection,
    context: &NotebookTemplateContext,
) -> Result<Option<NotebookSource>> {
    let chain = resolve_theme_chain(selection)?;
    if chain.layers.is_empty() && chain.terminal_typst {
        return Ok(None);
    }
    reject_removed_notebook_templates(&chain.layers)?;
    let Some((name, source)) = find_notebook_template(&chain.layers)? else {
        let theme = chain
            .layers
            .last()
            .map(layer_name)
            .unwrap_or_else(|| "typst".to_string());
        return Err(anyhow!(
            "theme `{theme}` does not contain {NOTEBOOK_TEMPLATE}"
        ));
    };
    let owns_body = source.contains("document.body");
    render_notebook_template(&name, NOTEBOOK_TEMPLATE, "notebook", source, context)
        .map(|source| Some(NotebookSource { source, owns_body }))
}

fn find_notebook_template(layers: &[ThemeLayer]) -> Result<Option<(String, String)>> {
    for layer in layers.iter().rev() {
        match layer {
            ThemeLayer::Builtin(name) => {
                let bundle = require_builtin(name)?;
                if let Some(source) = bundle.file(NOTEBOOK_TEMPLATE) {
                    return Ok(Some((bundle.name.to_string(), source.to_string())));
                }
            }
            ThemeLayer::Dir(dir) => {
                let path = dir.join(NOTEBOOK_TEMPLATE);
                if path.is_file() {
                    let source = std::fs::read_to_string(&path)
                        .with_context(|| format!("failed to read {}", path.display()))?;
                    return Ok(Some((dir_theme_name(dir), source)));
                }
            }
        }
    }
    Ok(None)
}

fn reject_removed_notebook_templates(layers: &[ThemeLayer]) -> Result<()> {
    for layer in layers {
        if let ThemeLayer::Dir(dir) = layer {
            for removed in [REMOVED_NOTEBOOK_TEMPLATE, REMOVED_PAGED_TEMPLATE] {
                let path = dir.join(removed);
                if path.is_file() {
                    return Err(anyhow!(
                        "{removed} is no longer supported; use {NOTEBOOK_TEMPLATE}"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn layer_name(layer: &ThemeLayer) -> String {
    match layer {
        ThemeLayer::Builtin(name) => (*name).to_string(),
        ThemeLayer::Dir(dir) => dir_theme_name(dir),
    }
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
