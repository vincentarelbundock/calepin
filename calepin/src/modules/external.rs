//! External module execution: run scripts or WASM via JSON or text protocol.
//!
//! External modules communicate via stdin/stdout. The protocol depends on
//! the `protocol` field in the extension manifest:
//! - "json": structured JSON input/output
//! - "text": raw document text on stdin, transformed text on stdout

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::io::Write;

use anyhow::Result;
use serde::{Serialize, Deserialize};

use crate::modules::registry::{TransformProject, RenderedPage};
use crate::render::elements::ElementRenderer;

// ---------------------------------------------------------------------------
// JSON protocol types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct DocumentModuleInput<'a> {
    name: &'a str,
    kind: &'a str,
    writer: &'a str,
    body: &'a str,
    vars: serde_json::Value,
}

#[derive(Serialize)]
struct ProjectModuleInput<'a> {
    name: &'a str,
    kind: &'a str,
    writer: &'a str,
    pages: Vec<ProjectPageInput>,
    vars: serde_json::Value,
    config: serde_json::Value,
}

#[derive(Serialize)]
struct ProjectPageInput {
    source: String,
    output: String,
    body: String,
    title: Option<String>,
    url: String,
}

#[derive(Deserialize)]
struct ModuleOutput {
    action: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    pages: Vec<PageOutput>,
}

#[derive(Deserialize)]
struct PageOutput {
    output: String,
    body: String,
}

// ---------------------------------------------------------------------------
// Script execution helper
// ---------------------------------------------------------------------------

pub fn run_script(script_path: &Path, module_dir: &Path, input: &str, writer: &str) -> Result<String> {
    let result = Command::new("sh")
        .arg("-c")
        .arg(script_path.to_string_lossy().as_ref())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(module_dir)
        .env("CALEPIN_FORMAT", writer)
        .env("CALEPIN_ROOT", crate::paths::get_project_dir().to_string_lossy().as_ref())
        .spawn()
        .and_then(|mut child| {
            if let Some(ref mut stdin) = child.stdin {
                stdin.write_all(input.as_bytes()).ok();
            }
            drop(child.stdin.take());
            child.wait_with_output()
        })?;

    if result.status.success() {
        Ok(String::from_utf8(result.stdout).unwrap_or_default())
    } else {
        let stderr = String::from_utf8_lossy(&result.stderr);
        anyhow::bail!("script failed: {}", stderr.trim());
    }
}

// ---------------------------------------------------------------------------
// External element children transform (div-level)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ElementModuleInput<'a> {
    name: &'a str,
    kind: &'a str,
    writer: &'a str,
    context: &'a str,
    content: &'a str,
    classes: &'a [String],
    id: &'a Option<String>,
    attrs: &'a std::collections::HashMap<String, String>,
    vars: serde_json::Value,
}

/// An element_children transform backed by an external script.
pub struct ExternalElementChildrenTransform {
    pub name: String,
    pub script_path: PathBuf,
    pub module_dir: PathBuf,
    pub vars: serde_json::Value,
}

impl crate::modules::registry::TransformElementChildren for ExternalElementChildrenTransform {
    fn apply(&self, ctx: &mut crate::modules::registry::ModuleContext) -> crate::modules::registry::ModuleResult {
        // Render children first
        let children_html: String = ctx.children().iter()
            .map(|el| ctx.render_child(el))
            .collect::<Vec<_>>()
            .join("\n");

        let input = ElementModuleInput {
            name: &self.name,
            kind: "element_children",
            writer: ctx.format,
            context: "div",
            content: &children_html,
            classes: ctx.classes,
            id: ctx.id,
            attrs: ctx.attrs,
            vars: self.vars.clone(),
        };
        let json_input = match serde_json::to_string(&input) {
            Ok(j) => j,
            Err(_) => return crate::modules::registry::ModuleResult::Pass,
        };

        match run_script(&self.script_path, &self.module_dir, &json_input, ctx.format) {
            Ok(output) => {
                match serde_json::from_str::<ModuleOutput>(&output) {
                    Ok(result) if result.action == "rendered" => {
                        crate::modules::registry::ModuleResult::Rendered(result.output)
                    }
                    _ => crate::modules::registry::ModuleResult::Pass,
                }
            }
            Err(e) => {
                eprintln!("Warning: external module '{}' error: {}", self.name, e);
                crate::modules::registry::ModuleResult::Pass
            }
        }
    }
}

// ---------------------------------------------------------------------------
// External document transform (JSON protocol)
// ---------------------------------------------------------------------------

/// A document transform backed by an external script using JSON protocol.
pub struct JsonDocumentTransform {
    pub name: String,
    pub script_path: PathBuf,
    pub module_dir: PathBuf,
    pub vars: serde_json::Value,
}

impl crate::modules::transform_document::TransformDocument for JsonDocumentTransform {
    fn transform(&self, document: &str, writer: &str, _renderer: &ElementRenderer) -> String {
        let input = DocumentModuleInput {
            name: &self.name,
            kind: "document",
            writer,
            body: document,
            vars: self.vars.clone(),
        };
        let json_input = match serde_json::to_string(&input) {
            Ok(j) => j,
            Err(e) => {
                cwarn!("failed to serialize module input for '{}': {}", self.name, e);
                return document.to_string();
            }
        };

        match run_script(&self.script_path, &self.module_dir, &json_input, writer) {
            Ok(output) => {
                match serde_json::from_str::<ModuleOutput>(&output) {
                    Ok(result) if result.action == "rendered" => result.output,
                    Ok(_) => document.to_string(), // "pass"
                    Err(_) => {
                        // Maybe the script returned plain text
                        output
                    }
                }
            }
            Err(e) => {
                cwarn!("module '{}' error: {}", self.name, e);
                document.to_string()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// External project transform
// ---------------------------------------------------------------------------

/// A project transform backed by an external script.
pub struct ExternalProjectTransform {
    pub name: String,
    pub script_path: PathBuf,
    pub module_dir: PathBuf,
    pub protocol: String,
    pub vars: serde_json::Value,
}

impl TransformProject for ExternalProjectTransform {
    fn transform(
        &self,
        pages: &mut Vec<RenderedPage>,
        _config: &crate::config::Metadata,
        writer: &str,
        _ctx: &crate::modules::registry::ProjectTransformContext,
    ) -> Result<()> {
        if self.protocol == "text" {
            // Text protocol: concatenate all page bodies, transform, split back
            // (not very useful for project modules, but supported)
            let combined: String = pages.iter().map(|p| p.body.as_str()).collect::<Vec<_>>().join("\n---PAGE---\n");
            match run_script(&self.script_path, &self.module_dir, &combined, writer) {
                Ok(output) => {
                    let parts: Vec<&str> = output.split("\n---PAGE---\n").collect();
                    for (i, page) in pages.iter_mut().enumerate() {
                        if let Some(body) = parts.get(i) {
                            page.body = body.to_string();
                        }
                    }
                }
                Err(e) => { cwarn!("project module '{}' error: {}", self.name, e); }
            }
        } else {
            // JSON protocol
            let input = ProjectModuleInput {
                name: &self.name,
                kind: "project",
                writer,
                pages: pages.iter().map(|p| ProjectPageInput {
                    source: p.source.to_string_lossy().to_string(),
                    output: p.output.to_string_lossy().to_string(),
                    body: p.body.clone(),
                    title: p.title.clone(),
                    url: p.url.clone(),
                }).collect(),
                vars: self.vars.clone(),
                config: serde_json::json!({
                    "title": _config.title,
                    "target": _config.target,
                }),
            };
            let json_input = serde_json::to_string(&input)?;

            match run_script(&self.script_path, &self.module_dir, &json_input, writer) {
                Ok(output) => {
                    if let Ok(result) = serde_json::from_str::<ModuleOutput>(&output) {
                        if result.action == "rendered" {
                            // Update pages from output
                            for page_output in &result.pages {
                                if let Some(page) = pages.iter_mut().find(|p| p.output == PathBuf::from(&page_output.output)) {
                                    page.body = page_output.body.clone();
                                }
                            }
                            // Add new pages (pagination, etc.)
                            for page_output in &result.pages {
                                if !pages.iter().any(|p| p.output == PathBuf::from(&page_output.output)) {
                                    pages.push(RenderedPage {
                                        source: PathBuf::new(),
                                        output: PathBuf::from(&page_output.output),
                                        body: page_output.body.clone(),
                                        title: None,
                                        date: None,
                                        subtitle: None,
                                        abstract_text: None,
                                        url: String::new(),
                                        toc: None,
                                        lang: None,
                                        metadata: crate::config::Metadata::default(),
                                    });
                                }
                            }
                        }
                    }
                }
                Err(e) => { cwarn!("project module '{}' error: {}", self.name, e); }
            }
        }

        Ok(())
    }
}
