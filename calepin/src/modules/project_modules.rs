//! Built-in project-level transform modules.
//!
//! These wrap the existing collection pipeline functionality as
//! `TransformProject` modules, enabling the unified pipeline architecture.
//!
//! Currently these are placeholders that establish the module interface.
//! The actual implementation delegates to the existing collection code.

use std::collections::HashMap;

use anyhow::Result;

use crate::modules::registry::{TransformProject, RenderedPage};

// ---------------------------------------------------------------------------
// SiteWrap: wraps pages in site template with navigation
// ---------------------------------------------------------------------------

/// Wraps each page in the site template (base.html with navbar, sidebar,
/// prev/next, breadcrumbs). Handles listing pages and pagination.
pub struct SiteWrapModule;

impl TransformProject for SiteWrapModule {
    fn transform(
        &self,
        _pages: &mut Vec<RenderedPage>,
        _config: &crate::config::Metadata,
        _writer: &str,
    ) -> Result<()> {
        // Currently handled by collection::templating::apply_collection_partials().
        // This module will eventually replace that code path.
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CrossrefGlobal: resolves cross-references across pages
// ---------------------------------------------------------------------------

/// Resolves cross-references across pages with chapter-qualified numbering.
/// Implements a two-pass pipeline: collect ref_data from all pages, build
/// global registry, resolve references.
pub struct CrossrefGlobalModule;

impl TransformProject for CrossrefGlobalModule {
    fn transform(
        &self,
        _pages: &mut Vec<RenderedPage>,
        _config: &crate::config::Metadata,
        _writer: &str,
    ) -> Result<()> {
        // Currently handled by collection::render::render_documents_with_crossref().
        // This module will eventually replace that code path.
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
        _pages: &mut Vec<RenderedPage>,
        _config: &crate::config::Metadata,
        _writer: &str,
    ) -> Result<()> {
        // Currently handled by collection::orchestrator::render_orchestrator().
        // This module will eventually replace that code path.
        Ok(())
    }
}

/// Resolve a built-in project module by name.
pub fn resolve_builtin_project(name: &str) -> Option<Box<dyn TransformProject>> {
    match name {
        "site_wrap" => Some(Box::new(SiteWrapModule)),
        "crossref_global" => Some(Box::new(CrossrefGlobalModule)),
        "orchestrator" => Some(Box::new(OrchestratorModule)),
        _ => None,
    }
}
