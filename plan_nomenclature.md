

 Nomenclature: "document" and "collection"

 Context

 Calepin has two modes: rendering a single .qmd file, and rendering a manifest
 (_calepin.toml) that groups many files. The codebase uses inconsistent terminology --
 "site", "page", "single-file mode", "project" -- for these concepts. This plan standardizes
 on:

 - Document: a single .qmd file rendered to output (calepin render test.qmd -t html)
 - Collection: a group of documents rendered together (website, book, etc.)

 Items within a collection are also called "documents" (not "pages").

 Decisions to confirm during implementation

 - Template variables {{ site.* }} / {{ page.* }}: rename to {{ collection.* }} / {{
 document.* }}?
 - Docs directory website/websites/ → website/collections/?
 - paths::for_single_file() → for_document()?

 These will follow the same pattern (rename everything) unless the user says otherwise.

 ---
 Phase 1: Rename site/ module to collection/

 Files to create/move:
 - calepin/src/site/ → calepin/src/collection/
   - mod.rs, config.rs, discover.rs, context.rs, render.rs, assets.rs, templates.rs, icons.rs

 Files to edit:
 - calepin/src/main.rs: mod site; → mod collection;, all site:: references → collection::
 - calepin/src/preview/mod.rs: crate::site:: → crate::collection::

 Phase 2: Rename structs and functions (internals)

 In collection/context.rs (formerly site/context.rs):

 - SiteContext → CollectionContext
 - PageContext → DocumentContext
 - build_site_context() → build_collection_context()
 - build_page_context() → build_document_context()
 - Comment: {{ site.* }} → {{ collection.* }} (if renaming template vars)
 - Comment: {{ page.* }} → {{ document.* }} (if renaming template vars)

 In collection/discover.rs (formerly site/discover.rs):

 - PageMeta → DocumentMeta
 - PageInfo → DocumentInfo
 - discover_pages() → discover_documents()
 - discover_standalone_pages() → discover_standalone_documents()
 - discover_listing_pages() → discover_listing_documents()
 - sort_pages() → sort_documents()
 - Warning string: "page not found" → "document not found"

 In collection/render.rs (formerly site/render.rs):

 - SiteRenderResult → CollectionRenderResult
 - render_pages() → render_documents()
 - render_pages_with_crossref() → render_documents_with_crossref()
 - render_one_page() → render_one_document()
 - render_one_page_pass1() → render_one_document_pass1()

 In collection/mod.rs (formerly site/mod.rs):

 - build_site() → build_collection()
 - rebuild_pages() → rebuild_documents()
 - apply_site_templates() → apply_collection_templates()
 - Variables: site_target_name → collection_target_name, site_target → collection_target,
 site_ctx → collection_ctx, site_with_active → collection_with_active
 - All pages variables → documents (e.g., let mut pages → let mut documents, changed_pages →
 changed_documents, pages_to_render → documents_to_render, all_listing_pages →
 all_listing_documents)
 - page_ctx → doc_ctx, page_tree → document_tree, page_map → document_map

 In collection/config.rs (formerly site/config.rs):

 - collect_page_paths() → collect_document_paths()
 - collect_standalone_paths() → collect_standalone_paths() (keep, "standalone" is the
 qualifier)
 - Comments: update "site" → "collection", "page" → "document"

 In project.rs:

 - PageEntry → DocumentEntry
 - PageNode → DocumentNode (variants: Document { path, title } instead of Page { path, title
 })
 - collect_all_page_paths() → collect_all_document_paths()
 - expand_section_pages() → expand_section_documents()
 - Field pages: Vec<PageEntry> → documents: Vec<DocumentEntry> in ContentSection
 - Field pages: Vec<PageNode> → documents: Vec<DocumentNode> in DocumentNode::Section
 - Comment already says "Collection fields" -- good

 In cli.rs:

 - is_site_config() → is_collection_config()
 - Help text: update "file(s)" → "document" language

 In paths.rs:

 - for_single_file() → for_document()
 - Comments: "single-file mode" → "document mode"

 In main.rs:

 - Comments: "single-file mode" → "document mode", "Single-file mode" → "Document mode"
 - All call sites updated to match new function/struct names
 - format!("page-{:03}.qmd", ...) -- this is test scaffolding, leave as-is

 In preview/mod.rs and preview/server.rs:

 - run_site() → run_collection()
 - start_site() → start_collection()
 - String literal: "building site..." → "building collection..."

 In render/template.rs:

 - load_page_template() and render_page_template() -- these refer to the page-level output
 template wrapping, NOT a "page" in a collection. Keep as-is.

 In render/elements.rs, render/div.rs:

 - Scan for "page" / "site" in comments. Update if referring to collection concepts.

 Phase 3: Template variable namespaces (if confirmed)

 In collection/context.rs:

 - Struct field serialization: SiteContext serializes as collection (via #[serde(rename)] or
 struct rename)
 - PageContext serializes as document

 In collection/templates.rs:

 - Template registration: ensure collection and document are the namespace keys passed to
 Jinja

 In built-in templates (calepin/src/templates/):

 - All {{ site.* }} → {{ collection.* }}
 - All {{ page.* }} → {{ document.* }}
 - Files: templates/website/*.html (base.html, page.html, listing.html, search.html, etc.)

 User-facing documentation:

 - Update all template variable references in website/templates/sites.qmd

 Phase 4: Documentation updates

 website/templates/sites.qmd:

 - "site" (noun) → "collection" throughout prose
 - "Build the site" → "Build the collection"
 - "configures your site" → "configures your collection"
 - "site-wide variables" → "collection-wide variables"
 - "Customizing site templates" → "Customizing collection templates"
 - Template variable table: {{ site.* }} → {{ collection.* }}, {{ page.* }} → {{ document.*
 }} (if Phase 3)
 - Code examples using template vars: update accordingly

 website/templates/templates.qmd:

 - "full website" / "structured website" → "collection"

 website/templates/yaml.qmd:

 - "Site configuration" → "Collection configuration"
 - "site-wide options" → "collection-wide options"

 website/templates/shortcuts.qmd:

 - "site-wide branding" → "collection-wide branding"

 website/templates/jinja.qmd:

 - "site templates" → "collection templates"

 website/websites/serve.qmd:

 - "renders the site or file" → "renders the collection or document"
 - "Calepin sites" → "Calepin collections"

 website/websites/structure.qmd:

 - "Calepin website is a directory" → "Calepin collection is a directory"
 - "Site manifest" → "Collection manifest"

 website/index.qmd:

 - "website, and documentation generator" -- review; "website" here may refer to the output
 target

 website/authoring/paths.qmd:

 - Already uses "Document mode" / "Collection mode" -- verify consistency

 website/misc/plugins.qmd:

 - Review "page templates" references

 website/websites/ directory:

 - Rename to website/collections/
 - Update website/_calepin.toml: pages = ["websites/*.qmd"] → pages = ["collections/*.qmd"]
 - Update any internal cross-references

 Phase 5: CLAUDE.md

 Update CLAUDE.md to reflect new terminology:
 - "page" → "document" where it refers to collection items
 - Any references to site/ module → collection/
 - Ensure Architecture section matches new names

 Verification

 1. make check -- ensure all renames compile
 2. make test -- ensure tests pass
 3. make install -- install updated binary
 4. cd website && make docs -- render documentation, verify no broken references
 5. Grep for stale terms: rg
 "build_site|SiteContext|PageInfo|PageContext|is_site_config|single.file" calepin/src/ should
  return zero hits
 6. Grep docs: rg "\bsite\b" website/**/*.qmd -- review remaining hits are intentional
 (target name, URLs)
╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌
