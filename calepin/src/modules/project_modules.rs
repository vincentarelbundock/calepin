//! Built-in project-level transform modules.
//!
//! These wrap the existing collection pipeline functionality as
//! `TransformProject` modules. The actual rendering logic lives in
//! `collection/` -- these modules delegate to it.

use std::collections::HashMap;

use anyhow::Result;

use crate::modules::registry::{TransformProject, RenderedPage, ProjectTransformContext};

// ---------------------------------------------------------------------------
// SiteWrap: wraps pages in site template with navigation
// ---------------------------------------------------------------------------

/// Wraps each page in the site template (base.html with navbar, sidebar,
/// prev/next, breadcrumbs). Handles listing pages and pagination.
pub struct SiteWrapModule;

impl TransformProject for SiteWrapModule {
    fn transform(
        &self,
        pages: &mut Vec<RenderedPage>,
        config: &crate::config::Metadata,
        _writer: &str,
        ctx: &ProjectTransformContext,
    ) -> Result<()> {
        let doc_infos: Vec<_> = pages.iter().map(|p| p.to_document_info()).collect();
        let results: HashMap<String, _> = pages.iter()
            .map(|p| (p.source.display().to_string(), p.to_render_result()))
            .collect();

        // No listing documents in the module path (listings are discovered
        // during the collection pipeline and passed to the module via pages).
        let empty_listings = HashMap::new();

        let url_mode = if ctx.portable {
            crate::utils::links::UrlMode::Relative
        } else {
            crate::utils::links::UrlMode::ServerRelative
        };

        crate::collection::templating::apply_collection_templates(
            config,
            &doc_infos,
            &results,
            &empty_listings,
            &ctx.base_dir,
            &ctx.output_dir,
            "html",
            &ctx.target_name,
            url_mode,
            ctx.serve,
        )?;

        // Re-read the wrapped pages from disk (apply_collection_templates
        // overwrites the output files with the site-wrapped versions).
        for page in pages.iter_mut() {
            let path = ctx.output_dir.join(&page.output);
            if let Ok(body) = std::fs::read_to_string(&path) {
                page.body = body;
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Orchestrator: assembles chapter fragments into a master file
// ---------------------------------------------------------------------------

/// Assembles chapter fragments into a master file for PDF compilation.
/// Renders the orchestrator template with the document tree.
pub struct OrchestratorModule;

impl TransformProject for OrchestratorModule {
    fn transform(
        &self,
        pages: &mut Vec<RenderedPage>,
        config: &crate::config::Metadata,
        writer: &str,
        ctx: &ProjectTransformContext,
    ) -> Result<()> {
        let doc_infos: Vec<_> = pages.iter().map(|p| p.to_document_info()).collect();
        let results: HashMap<String, _> = pages.iter()
            .map(|p| (p.source.display().to_string(), p.to_render_result()))
            .collect();

        // Auto-detect orchestrator template
        let ext = crate::paths::resolve_extension(writer);
        let orchestrator_filename = format!("orchestrator.{}", ext);
        let orchestrator_path = config.orchestrator.clone()
            .or_else(|| {
                let p = crate::paths::templates_dir(&ctx.base_dir)
                    .join(&ctx.target_name)
                    .join(&orchestrator_filename);
                if p.exists() { return Some(p.display().to_string()); }
                let builtin_path = format!("{}/{}", ctx.target_name, orchestrator_filename);
                if crate::render::elements::BUILTIN_TEMPLATES.get_file(&builtin_path).is_some() {
                    Some(format!("__builtin__:{}", builtin_path))
                } else {
                    None
                }
            });

        if let Some(ref orch_path) = orchestrator_path {
            crate::collection::orchestrator::render_orchestrator(
                config,
                &doc_infos,
                &results,
                &ctx.base_dir,
                &ctx.output_dir,
                orch_path,
                writer,
                ext,
                &ctx.target_name,
                false, // quiet
            )?;
        }

        Ok(())
    }
}

/// Resolve a built-in project module by name.
pub fn resolve_builtin_project(name: &str) -> Option<Box<dyn TransformProject>> {
    match name {
        "site_wrap" => Some(Box::new(SiteWrapModule)),
        "orchestrator" => Some(Box::new(OrchestratorModule)),
        _ => None,
    }
}
