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

/// Resolve a built-in project module by name.
pub fn resolve_builtin_project(name: &str) -> Option<Box<dyn TransformProject>> {
    match name {
        "site_wrap" => Some(Box::new(SiteWrapModule)),
        _ => None,
    }
}
