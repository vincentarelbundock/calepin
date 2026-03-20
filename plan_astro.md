# Plan: Recreate Astro/Starlight Site Builder as External Tool

This document records everything that was removed from calepin core so it can be
rebuilt as a standalone tool that uses `calepin --batch` to render pages.

## What was removed

### 1. `calepin/src/website.rs` (306 lines)

Config parsing and page-collection utilities for the YAML-driven website builder.

**Config types:**
- `WebsiteConfig` — top-level: title, subtitle, favicon, navbar, pages, format_overrides, resources, builder plugin name, raw YAML
- `NavbarConfig` — logo path, right-side nav items
- `NavItem` — text + href
- `PageEntry` — enum: either `Page { text, href }` or `Section { title, pages }` (recursive)
- `FlatPage` — flattened page ref: text + href

**Config parsing (`parse_config`):**
- Reads a YAML file (e.g., `_calepin.yaml`)
- Extracts `website.title`, `website.subtitle`, `website.favicon`
- Parses `website.navbar.logo`, `website.navbar.right[]` (nav items)
- Parses `website.pages[]` recursively (pages and sections)
- Extracts `format.html.*` as format overrides (flattened to `key=value`, nested maps to `key.subkey=value`)
- Extracts `project.resources[]` (files to copy to public/)
- Extracts `calepin.builder` or `website.builder` (plugin name, defaulted to "astro")

**Page utilities:**
- `collect_pages(entries)` — flattens `PageEntry` tree into `Vec<FlatPage>`
- `read_page_titles(pages, base_dir)` — reads YAML front matter from each .qmd to get titles
- `title_from_filename(href)` — converts `front_matter.qmd` → `"Front Matter"`
- `page_display_title(href, explicit_text, titles)` — resolves display title: explicit > YAML title > filename

**Rendering:**
- `render_page_bare(input, output_path, format_overrides)` — calls `render_core()` with HTML format, returns `(body_html, metadata, syntax_css)`. Uses `DataTheme` color scope for Starlight compatibility.

**Helpers:**
- `copy_dir_recursive(src, dst)` — recursive directory copy

### 2. `calepin/src/site_builder.rs` (287 lines)

Generic plugin-based site builder. The host renders pages, then delegates layout/scaffolding to a WASM plugin.

**Protocol types (JSON serialized to/from plugin):**
- `SiteBuildContext` — `{ config: Value, pages: [RenderedPage], syntax_css: String }`
- `RenderedPage` — `{ stem, title, body_html, raw_source, is_index, vars: HashMap }`
- `SiteBuildResult` — `{ scaffold_command?, output_dir, cleanup_dirs, files: [SiteFile], copies: [CopyRule] }`
- `SiteFile` — `{ path, content }` (files to write)
- `CopyRule` — `{ from, to: [String] }` (assets to copy)

**`build(config_path, quiet)` flow:**
1. Parse config via `website::parse_config()`
2. Load builder WASM plugin by name (default "astro")
3. Require `index.qmd` exists
4. Collect and flatten all pages
5. Read page titles from YAML front matter
6. For each `.qmd` page:
   - Compute stem (e.g., `basics` from `basics.qmd`)
   - Call `render_page_bare()` to get body HTML + metadata + syntax CSS
   - Read raw .qmd source
   - Build template vars via `template::build_html_vars()`
   - Resolve display title
7. Serialize `SiteBuildContext` as JSON, call plugin's `build_site` function
8. Deserialize `SiteBuildResult`
9. If `output_dir/package.json` doesn't exist, run `scaffold_command` (npm create astro)
10. Clean up directories listed in `cleanup_dirs`
11. Write all `files` to `output_dir/`
12. Execute all `copies` (file or recursive dir copy)
13. Return output_dir path

**Scaffolding:**
- Runs scaffold command via shell (`$SHELL -lc "command"`)
- Removes `.git` if scaffold created one

**Helper:**
- `yaml_to_json()` — converts saphyr YamlOwned to serde_json::Value

### 3. `plugins/astro/` (WASM plugin crate, ~870 lines)

The Astro Starlight builder plugin. Compiled to `wasm32-unknown-unknown`, loaded by site_builder via extism.

**Cargo.toml deps:** `extism-pdk`, `serde`, `serde_json`

**`build_site` entry point:**
Receives `SiteBuildContext` JSON, returns `SiteBuildResult` JSON.

For each rendered page, generates:
- `src/html/{stem}.html` — rendered body
- `src/qmd/{stem}.qmd` — raw source (for split view)
- `src/pages/{stem}.astro` — Astro page wrapper

Also generates:
- `src/content/docs/_placeholder.md` — keeps Starlight's content collection non-empty
- `astro.config.mjs` — full Starlight config with title, logo, favicon, social links, sidebar
- `src/styles/calepin.css` — embedded `astro.css` + syntax highlighting CSS

**Copy rules emitted:**
- Logo (+ dark variant) → `src/assets/` and `public/`
- Favicon → `public/`
- Resources → `public/`
- Figure directories (`{stem}_files`) → `public/`
- Non-.qmd page files (e.g., PDFs) → `public/`

**Scaffold command:** `npm create astro@latest -- --template starlight --no-install --yes _astro`
**Output directory:** `_astro`
**Cleanup dirs:** `src/content/docs` (cleared before each build)

**Astro page generation:**
- `build_astro_page(stem, title, body_html)` — imports raw HTML + source, renders in `StarlightPage` with split view wrapper and TOC headings
- `build_astro_index_page(stem, config, vars, body_html)` — hero header with logo (light/dark), subtitle, author, date, abstract, then split view
- TOC headings extracted from HTML via `extract_headings()` (h2–h4, regex-free parser)
- `relative_prefix(stem)` — computes `../` depth for Astro imports

**Astro config generation (`build_astro_config`):**
- Logo config with light/dark variants
- Favicon
- Social links (auto-detected icon from text/href: github, discord, twitter, mastodon, bluesky, linkedin, youtube)
- Sidebar from page hierarchy (recursive)

**Embedded assets:**
- `astro.css` (128 lines) — Starlight-compatible styles for code blocks, figures, callouts, theorems, tabsets, footnotes, layout grid, MathJax, split view toggle
- `ASTRO_SCRIPTS` — inline JS for MathJax config, tabset interaction, footnote previews, split view toggle button

**Helper functions:**
- `escape_js()`, `escape_astro_string()`, `strip_markdown()`, `strip_html_tags()`
- `file_name()` — extract filename from path
- `dark_variant("logo.png")` → `"logo_dark.png"`
- `guess_social_icon(text, href)` — maps to Starlight icon names

### 4. Code removed from `calepin/src/main.rs`

The YAML input detection block (lines 87–101):
```rust
let ext = input.extension().and_then(|e| e.to_str());
if ext == Some("yaml") || ext == Some("yml") {
    if let Some(parent) = input.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::env::set_current_dir(parent)?;
    }
    let config_name = input.file_name().unwrap();
    let config_path = Path::new(config_name);
    if cli.preview {
        return preview::run_website(config_path, &cli);
    }
    return site_builder::build(config_path, cli.quiet).map(|_| ());
}
```

### 5. Code removed from `calepin/src/plugins.rs`

- `has_build_site: bool` field on `PluginHandle`
- `call_build_site(&self, ctx_json: &str) -> Option<String>` method
- Detection of `build_site` export in `load_one()`

### 6. Code removed from `calepin/src/preview/mod.rs`

- `run_website(config_path, cli)` — website preview: build → npm install → npm build → serve dist/ with live-reload → watch directory for .qmd/.yaml/.bib changes → rebuild on change
- `site_npm_install(output_dir)` — runs `npm install` if `node_modules/` missing
- `site_npm_build(output_dir)` — runs `npm run build`

### 7. Code removed from `calepin/src/preview/server.rs`

- `start_site(port, version, serve_dir)` — static file server for built site with:
  - Clean URL support (try `/path` → `/path.html` → `/path/index.html`)
  - Live-reload script injection into all HTML responses
  - Version-based polling at `/__version`

### 8. Code removed from `calepin/src/preview/watcher.rs`

- `watch_dir(dir, stop, on_change)` — watches a directory (non-recursive) for changes to `.qmd`, `.yaml`, `.yml`, `.bib` files, with 100ms debounce

### 9. Makefile

- Removed `website` target: `cd website && calepin _calepin.yaml --preview`

---

## How to rebuild as an external tool

The new `--batch` flag replaces the internal rendering loop. The external tool should:

### Step 1: Parse the YAML config

Reuse the config parsing logic from `website.rs`. The YAML structure:

```yaml
website:
  title: "My Site"
  subtitle: "A subtitle"
  favicon: favicon.svg
  navbar:
    logo: logo.svg
    right:
      - text: GitHub
        href: https://github.com/user/repo
  pages:
    - index.qmd
    - section: Getting Started
      pages:
        - basics.qmd
        - advanced.qmd
    - href: paper.pdf
      text: "Download PDF"

format:
  html:
    toc: true
    highlight-style:
      light: github
      dark: nord

project:
  resources:
    - data/example.csv

calepin:
  builder: astro
```

### Step 2: Build a batch manifest

From the flattened page list, generate a JSON manifest for `calepin --batch`:

```json
[
  {"input": "index.qmd", "output": "_astro/src/html/index.html", "format": "html",
   "overrides": ["toc=true", "highlight-style.light=github", "highlight-style.dark=nord"]},
  {"input": "basics.qmd", "output": "_astro/src/html/basics.html", "format": "html",
   "overrides": ["toc=true"]}
]
```

Or use `--batch-stdout` to get bodies in the JSON result without writing files,
then write them yourself with the Astro wrapper.

### Step 3: Call calepin --batch

```bash
echo "$MANIFEST" | calepin --batch - --batch-stdout -q
```

The JSON result includes `title`, `date`, `subtitle`, `abstract`, and `body` for each page.

### Step 4: Generate Astro project files

Use the logic from `plugins/astro/src/lib.rs`:

1. For each page, generate:
   - `src/html/{stem}.html` (from batch result body)
   - `src/qmd/{stem}.qmd` (raw source — read from disk)
   - `src/pages/{stem}.astro` (Astro wrapper with StarlightPage)

2. Generate `astro.config.mjs` with sidebar, logo, social links

3. Generate `src/styles/calepin.css`

4. Copy assets: logo, favicon, resources, figure dirs, non-.qmd files

5. Scaffold if needed: `npm create astro@latest -- --template starlight --no-install --yes _astro`

6. Run `npm install && npm run build` in `_astro/`

### Step 5: Preview (optional)

For live preview, the external tool should:
1. Build the site (steps 1–4)
2. Run `npm install` if needed
3. Run `npm run build`
4. Serve `_astro/dist/` with a static file server (with clean URL support)
5. Watch for `.qmd`/`.yaml`/`.bib` changes
6. On change: re-run batch render + npm build, bump version for reload

### Implementation language

The external tool can be written in any language. Python or Rust are natural choices:
- **Python**: easy YAML/JSON handling, can shell out to `calepin --batch`
- **Rust**: can reuse the config parsing types directly, call calepin as subprocess

The key insight is that `calepin --batch` handles all the rendering (including parallel execution), so the external tool only needs to:
1. Parse config → build manifest
2. Call `calepin --batch`
3. Generate Astro wrapper files from the results
4. Copy assets
5. Run npm build

### Notes on syntax CSS

The old flow used `ElementRenderer::syntax_css_with_scope(DataTheme)` to get
Starlight-compatible syntax highlighting CSS (scoped to `[data-theme='light']`
and `[data-theme='dark']` instead of `@media (prefers-color-scheme: ...)`).

With `--batch`, syntax CSS is not included in the JSON output. Options:
- Add a `--syntax-css` flag to calepin that dumps the CSS
- Generate it externally based on the highlight-style config
- Use a fixed CSS file (the old `astro.css` already handles most styling)

### Reference: YAML config file example (`_calepin.yaml`)

```yaml
website:
  title: Calepin
  subtitle: A fast document renderer
  favicon: website/_calepin/favicon.svg
  navbar:
    logo: website/_calepin/logo.svg
    right:
      - text: GitHub
        href: https://github.com/vincentarelbundock/calepin
  pages:
    - index.qmd
    - section: Tutorial
      pages:
        - basics.qmd
        - templates.qmd
        - filters.qmd
        - shortcodes.qmd
        - plugins.qmd

format:
  html:
    toc: true
    highlight-style:
      light: github
      dark: nord

project:
  resources:
    - calepin_files
```
