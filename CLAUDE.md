# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**calepin** is a Rust CLI that renders `.qmd` (Quarto-compatible) documents to HTML, LaTeX, Typst, and Markdown. It runs R (via a persistent `Rscript` subprocess) and Python (via a persistent `python3` subprocess) to execute code chunks, processes citations with hayagriva, and resolves cross-references.

The tutorial (`website/authoring/basics.qmd`) must be valid Quarto syntax so it can be benchmarked against Quarto and litedown. calepin-specific extensions (modules, custom templates, plugins) are documented in `website/templates/templates.qmd` and `website/extensions/plugins.qmd`.

When referring to the software by name in documentation or notebooks, always write *Calepin* (italic, capital C).

## Writing style

* Never use em or en dashes in documentation or websites.
* Never use **bold** words in documentation or websites unless it was there in the original source.

## Workflow

When you are done with changes and believe the feature works, run `make install` to install the updated binary.

### Git commits

When asked to commit, do NOT run `git add` or `git commit` yourself. Instead:

1. Check `git status` to see what files are staged/unstaged.
2. Propose a full `git commit -m "..."` command the user can paste into their shell.

## Build Commands

```
make build          # Debug build
make release        # Optimized release build
make install        # Install to ~/.cargo/bin + shell completions
make check          # Fast compile check (no linking)
make test           # cargo test
make docs           # Render all .qmd files in website/ to all formats
make bench          # Time single file render (requires hyperfine)
make bench-batch    # Time 1000 parallel files (gibberish)
make plugins        # Build WASM plugins (requires wasm32-unknown-unknown target)
make site           # Build debug + serve static site from website/
make clean          # Remove build artifacts
make flush          # Delete all *_cache and *_files directories
make prof           # Profile single file and print bottlenecks
make prof-batch     # Profile 1000 parallel files (gibberish)
```

Run a single test: `cargo test test_name`

CLI: `calepin <input.qmd> [-o PATH] [-t TARGET] [-s KEY=VALUE ...] [-q] [--writer FORMAT] [--no-highlight] [--clean]`

Subcommands: `render` (default), `preview`, `init`, `flush`, `man`, `extra`. Shell completions: `calepin extra completions SHELL`.

**Important**: website/ must be rendered with `cd website && ../calepin/target/debug/calepin file.qmd` so that sidecar overrides are found relative to the working directory. `make docs` handles this.

## Architecture

### Data flow

The pipeline transforms data through three representations:

1. **`.qmd` text** -> **`Block`** (parse stage) -- Raw text, code chunks, fenced divs, raw blocks. Defined in `base/types.rs`.
2. **`Block`** -> **`Element`** (evaluate stage) -- Code is executed, shortcodes expanded, conditional content filtered. Elements are: `Text`, `CodeSource`, `CodeOutput`, `CodeWarning`, `CodeMessage`, `CodeError`, `Figure`, `CodeAsis`, `Div`. Defined in `types.rs`.
3. **`Element`** -> **output string** (render stage) -- Each element passes through var builders and templates to produce HTML/LaTeX/Typst/Markdown.

### Pipeline stages

```
parse -> evaluate -> bibliography
  -> TransformElement (pre-render: SVG-to-PDF)
  -> render (includes TransformElementChildren per div)
  -> crossref
  -> assemble_page (page template wrapping)
  -> TransformDocument (highlight CSS/colors, footnotes, slides, image embedding)
  -> write
```

1. **Parse** -- TOML front matter is parsed (`config/parse.rs`) into metadata. Non-TOML front matter (e.g., YAML) falls back to a simple parser that extracts `title`, `author`, `date`, and `bibliography`. Recursive block parsing into `Block` enum (`parse/blocks.rs`).
2. **Evaluate** (`engines/mod.rs`) -- Jinja body processing, code execution, blocks become `Element`s.
3. **Bibliography** (`references/bibliography.rs`) -- Citation keys resolved via hayagriva.
4. **TransformElement** -- Pre-render element mutations. Modules implementing `TransformElement` receive each element and can mutate it (e.g., `convert_svg_pdf` rewrites SVG figure paths to PDF).
5. **Render** -- `ElementRenderer` dispatches each element. Divs go through the module registry (`TransformElementChildren` for structural rewriting, then template lookup). Code/figure elements go through `BuildElementVars` then templates.
6. **Cross-ref resolution** (`references/crossref.rs`) -- `@fig-x` references resolved to links/numbers.
7. **Assemble page** -- MiniJinja page template wrapping (`render/template/`).
8. **TransformDocument** -- Post-assembly document transforms. Modules receive the full document string and can modify it (highlight CSS injection, footnote appending, slide splitting, image embedding).
9. **Write** -- File output or pandoc conversion.

### Module system

All extensibility flows through the `ModuleRegistry` (`modules/registry.rs`). Five module kinds:

| Trait | `ModuleKind` | When | What |
|---|---|---|---|
| `TransformElement` | `Element` | Pre-render | Mutate individual elements (pipeline handles tree recursion) |
| `TransformElementChildren` | `ElementChildren` | During render, per div | Rewrite div children (tabset, layout) |
| `TransformDocument` | `Document` | Post-assembly | Transform the full document string |
| `TransformProject` | `Project` | After all pages rendered | Cross-page coordination (navigation, cross-refs) |
| -- | `Noop` | -- | Template providers only |

Auto-numbering is declarative: `number = true` on a `MatchRule` tells `div.rs` to inject `{{ number }}` and `{{ type_class }}` vars.

### Built-in modules

| Module | Kind | What |
|---|---|---|
| `convert_svg_pdf` | TransformElement | SVG-to-PDF figure conversion for LaTeX |
| `tabset` | TransformElementChildren | `.panel-tabset` -> tab navigation (HTML only) |
| `layout` | TransformElementChildren | `layout-ncol`/`layout-nrow` -> grid markup |
| `theorem` | Noop + number=true | Auto-number theorem-type divs |
| `highlight` | TransformDocument | Inject syntax CSS (HTML) or `\definecolor` (LaTeX) |
| `append_footnotes` | TransformDocument | Append footnote section (HTML) |
| `split_slides` | TransformDocument | Split body into `<section>` slides |
| `embed_images` | TransformDocument | Base64-encode `<img>` sources (HTML) |

### Extension system

Each output format is defined as an extension (`extensions/{name}/extension.toml`). Built-in extensions: `html`, `latex`, `typst`, `markdown`, `slides`, `website`, `book-typst`, `book-latex`. User extensions live in `{stem}_calepin/extensions/{name}/`.

An extension bundles a target definition, templates, modules, CSS/JS assets, and variables. Extensions inherit from a parent via `inherits`. Template resolution is layered: user overrides > extension templates > parent extension > built-in defaults.

Extension manifests are parsed by `config/extension.rs` (`ExtensionManifest` struct). Built-in manifests are embedded at compile time via `include_str!`.

### Target configuration

Targets are defined in `config/document.toml` (document targets) and `config/collection.toml` (collection targets), with built-in extension manifests as an additional source. Resolved by `config/targets.rs::resolve_target()`.

User targets in `{stem}_calepin/config.toml` inherit from built-in targets via `inherits`.

### FormatPipeline

`FormatPipeline` (`render/formats.rs`) reads pipeline config from the Target and dispatches to modules at each stage. Created via `FormatPipeline::from_target()` or `FormatPipeline::from_writer()`.

## Format Names

Internally, formats use canonical writer names: `html`, `latex`, `typst`, `markdown`. File extensions: `.html`, `.tex`, `.typ`, `.md`. Template resolution uses the writer name (e.g., `templates/html/figure.html`). Raw blocks use canonical names (```` ```{=latex} ````).

## Source Layout

### `calepin/src/` -- Top-level

- `main.rs` -- Entry point (only file at src/ root)

### `cli/` -- CLI and command handlers

- `args.rs` -- CLI argument parsing (clap) + `cwarn!` macro
- `render.rs`, `preview.rs`, `info.rs`, `flush.rs` -- Command handlers
- `init_sidecar.rs`, `new_notebook.rs`, `new_website.rs`, `new_book.rs`, `new_extension.rs`, `new_gibberish.rs`, `templates.rs` -- Init/scaffolding handlers

### `config/` -- Configuration and project context

- `types.rs` -- `Metadata`, `Author`, `Target` structs
- `parse.rs` -- TOML front matter parsing: `split_frontmatter()`, `parse_metadata()`
- `merge.rs` -- Metadata merge logic (last wins)
- `targets.rs` -- Target resolution and inheritance
- `extension.rs` -- `ExtensionManifest` parsing, built-in extension embedding, discovery
- `load.rs` -- Project config loading, `LanguageConfig`, `ContentSection`
- `context.rs` -- `ProjectContext`: resolves project config and target for a render
- `toml/` -- Embedded default configs: `document.toml`, `shared.toml`, `collection.toml`, `modules.toml`

### `render/` -- Element rendering and pipeline

- `pipeline.rs` -- Core render pipeline orchestrator: parse, evaluate, render
- `formats.rs` -- `FormatPipeline`: dispatches modules at each pipeline stage
- `elements.rs` -- `ElementRenderer`: dispatches each element, holds pre-compiled template env. Key functions: `BUILTIN_TEMPLATES`, `resolve_builtin_template`, `resolve_element_template`
- `div.rs` -- Div rendering pipeline: module dispatch, auto-numbering, template lookup
- `span.rs` -- Span rendering pipeline
- `vars.rs` -- `BuildElementVars` trait + `BuildCodeVars`: per-element template var builders
- `convert.rs` -- Comrak options, `ImageAttrs`, `render_inline()` entry points
- `template/` -- MiniJinja template engine: `apply_template()`, page template loading, `build_template_vars()`, Jinja body processing (`{% include %}` expansion, code block protection, template context)
- `markers.rs` -- Unicode marker system for protecting content through conversion
- `metadata.rs` -- Author/citation/appendix formatting via templates

### `modules/` -- Module system and built-in modules

- `registry.rs` -- `ModuleRegistry`, `TransformElement`, `TransformElementChildren`, `TransformDocument`, `TransformProject` traits, `ModuleKind`, `RenderedPage`, built-in module registration
- `manifest.rs` -- `module.toml` parsing: `ModuleManifest`, `MatchRule`, `MatchSpec`
- `transform_document.rs` -- `TransformDocument` trait + `ScriptTransformDocument` (user script execution)
- `project_modules.rs` -- Built-in `TransformProject` implementations (site_wrap, crossref_global, orchestrator)
- `external.rs` -- External module execution via JSON/text protocol (scripts, WASM)
- `highlight/` -- Syntax highlighting: `Highlighter`, themes, CSS/LaTeX color generation
- `convert_svg_pdf/` -- `TransformElement`: SVG-to-PDF figure conversion
- `convert_math/` -- LaTeX-to-Typst math converter (parser, AST, emitter, symbols)
- `tabset/` -- `TransformElementChildren`: panel-tabset -> HTML tabs
- `layout/` -- `TransformElementChildren`: layout grids (CSS Grid, LaTeX minipage, Typst grid)
- `figure/` -- Figure div helper functions + `BuildFigureVars`
- `table/` -- Table div helper functions
- `append_footnotes/` -- `TransformDocument`: append HTML footnote section
- `split_slides/` -- `TransformDocument`: split body into RevealJS slides
- `embed_images/` -- `TransformDocument`: base64-encode images

### `emit/` -- AST emitters

Shared AST walker + format-specific implementations via `FormatEmitter` trait.

- `mod.rs` -- `FormatEmitter` trait + `walk_ast()`, heading IDs, section numbering, footnotes, tables
- `html.rs`, `latex.rs`, `typst.rs`, `markdown.rs` -- Per-format emitters

### `utils/` -- Shared utilities

- `tools.rs` -- External tool availability checks and error messages
- `paths.rs` -- Path utilities: `templates_dir`, `resolve_template`
- `escape.rs` -- Format-specific code escaping
- `lipsum.rs` -- Lorem ipsum text generation
- `cache.rs` -- Hash-based page cache for incremental builds
- `date.rs` -- Date formatting and resolution helpers

### `templates/` -- Built-in Jinja templates (embedded at compile time)

Per-engine templates for elements, page templates, shortcodes:
`templates/{html,latex,typst,markdown,slides,website,book-typst,book-latex}/`

Website template icons live in `templates/website/icons/` (used via `{% include %}`).

User overrides: `{stem}_calepin/templates/{engine}/{name}.{ext}`

### `scaffold/` -- Project scaffolding and shared assets

- `website/`, `book/`, `notebook/` -- Starter project templates for `calepin init`
- `assets/` -- Shared website assets (CSS, JS, social icons) copied to output at build time

### Other directories

- `engines/` -- Code execution: R, Python, shell subprocess management
- `parse/` -- Block parsing: `.qmd` text -> `Block` enum
- `references/` -- Bibliography (`bibliography.rs`) + cross-references (`crossref.rs`)
- `base/` -- Core types (`types.rs`), paths (`paths.rs`), utilities (`util.rs`, `value.rs`)
- `collection/` -- Multi-document builds (site/book rendering), includes `templates.rs` for template resolution
- `preview/` -- Live preview server with hot reload

## Templates and Module Resolution

Templates use Jinja syntax (`{{cfg.variable}}`, `{{clp.variable}}`, `{% if %}`, `{% for %}`). Variables are namespaced: `cfg.*` for user-authored values (front matter, attributes, labels), `clp.*` for engine-computed values (rendered content, format, assets). Variable names use underscores. CSS class names in source documents keep dashes; the resolver normalizes dashes to underscores for lookup.

**Template resolution order** (first match wins, layered):
1. Module element dirs (in registry order)
2. Sidecar templates: `{stem}_calepin/templates/{target|writer|common}/`
3. Project templates: `{stem}_calepin/templates/{target|writer|common}/` (root sidecar)
4. Extension templates: `{stem}_calepin/extensions/{name}/templates/{writer|common}/` (child-first inheritance)
5. Built-in `templates/{writer}/{name}.{ext}` (embedded in binary)

Resolution is layered: individual files can be overridden at any level. Missing files fall through to the next level. `calepin init` does not copy templates; projects start clean and override only what they need.

**Module resolution**: `{stem}_calepin/modules/{name}/module.toml`

**module.toml manifest**:

```toml
name = "mymodule"

[element]
match.classes = ["myclass"]     # CSS classes (OR'd)
match.attrs = ["my-attr"]       # Attribute names (OR'd)
match.id_prefix = "fig-"        # ID prefix
match.formats = ["html"]        # Output formats (omit = all)
match.number = true             # Auto-number matching divs

[document]
run = "postprocess.sh"          # Script: stdin=document, stdout=transformed
```

## Raw Output Protection

Format-specific output from span templates must survive markdown-to-format conversion without being re-escaped. All markers use Unicode noncharacters (`\u{FFFF}` start, `\u{FFFE}` end) as delimiters. Input is sanitized by `markers::sanitize()` at the start of the pipeline.

Marker types (single-char prefix between delimiters):

- **`M`** -- Math expressions (`$...$` and `$$...$$`). Use `\$` for a literal dollar sign.
- **`D`** -- Escaped dollar signs. Resolved per-format by `markers::resolve_escaped_dollars()`.
- **`L`** -- Equation labels (`{#eq-...}` after display math).
- **`R`** -- Raw span/template output (including built-in spans like pagebreak, video, placeholder).

## Configuration

Documents can carry TOML front matter between `---` delimiters. Non-TOML front matter (e.g., YAML) falls back to a simple parser for basic fields (title, author, date, bibliography).

**Merge order** (last wins): built-in defaults < `{stem}_calepin/config.toml` (root sidecar) < `{stem}_calepin/config.toml` (page sidecar) < TOML front matter < CLI (`-s`)

**Sidecar directories**: Each document has a `{stem}_calepin/` directory alongside it (e.g., `paper_calepin/` for `paper.qmd`). For websites, `index_calepin/` is the shared root sidecar for all pages.

calepin-specific settings are nested under the `[calepin]` table:

```toml
[calepin]
plugins = ["txtfmt"]
extensions = ["lightbox"]
```

Standard fields (`title`, `author`, `bibliography`, `format`, etc.) are top-level keys.

## Chunk Options

Both pipe syntax (`#| key: value`) and header key-value pairs (`{r, echo=FALSE}`) are accepted. Header options are converted internally to pipe-equivalent options; when both are present, pipe comments take precedence. Option names use dashes (`fig-width`), normalized to underscores internally. `label` is rejected in pipe comments -- it must be in the header.

## Jinja Body Processing

The `.qmd` body text is processed as a Jinja template during the evaluate stage (`jinja_engine.rs`). Code blocks and inline code are protected from Jinja evaluation. Use `#| jinja: true` chunk option to opt-in to Jinja processing inside a code chunk.

Context variables:
- `{{ cfg.title }}`, `{{ cfg.author }}`, `{{ cfg.date }}`, etc. -- document metadata
- `{{ cfg.key.subkey }}` -- non-standard front matter fields (with nesting)
- `{{ env.HOME }}`, `{{ env.USER }}`, etc. -- system environment variables
- `{{ clp.writer }}` -- current output format (`html`, `latex`, `typst`, `markdown`)
- `{{ cfg.target }}` -- current target name

File inclusion: `{% include "file.qmd" %}` (pre-parse, runs before block parsing). Escaping: `{% raw %}...{% endraw %}`.

## Built-in Spans

Bracketed spans `[content]{.class key=value}` are processed during rendering. Built-in spans (output driven by per-engine templates in `templates/{engine}/`):

- `[]{.pagebreak}` -- format-specific page break
- `[]{.video url="..." width="..." height="..." title="..."}` -- video embed
- `[]{.lorem paragraphs=2}` -- placeholder lorem ipsum text (also `sentences`, `words`)
- `[]{.placeholder width=600 height=400}` -- placeholder image (also `text`, `color`)

## Dependencies

- `comrak` -- CommonMark + GFM markdown parsing/rendering
- `hayagriva` -- Citation/bibliography processing
- `syntect` -- Syntax highlighting
- `minijinja` -- Template engine for element/page templates and body processing
- `clap` + `clap_complete` -- CLI and shell completions
- `toml` + `serde` -- TOML config parsing (front matter, sidecar `config.toml`)
- `usvg` + `svg2pdf` -- SVG-to-PDF conversion for LaTeX targets

## Extensions

Extensions are the unit of distribution and customization. An extension is a directory with an `extension.toml` manifest that can provide templates, CSS/JS assets, modules, and variables.

**Installation**: `{stem}_calepin/extensions/{name}/extension.toml`

**Activation**: Set `target = "name"` in document front matter or pass `-t name` on the CLI. Side-load with `[calepin] extensions = ["name"]`.

**Scaffolding**: `calepin init extension myext --inherits html`

**Debugging**: `calepin templates list html` shows the full resolution chain.

**External modules**: Extensions can declare modules with `run = "scripts/foo.sh"` that execute via stdin/stdout (text or JSON protocol).

See `website/extensions/extensions.qmd` for the full specification.

## Profiling

Profile with samply and print bottleneck summary:

```
make prof PROF_FILE=bench/text.qmd    # Single file
make prof-batch                       # 1000 parallel files (gibberish)
```

## Function Naming Convention

Use `verb_noun` or `verb_noun_qualifier` format. Consistent verbs for similar operations:

- **`parse_*`** -- Convert text/input into structured data (`parse_body`, `parse_metadata`, `parse_attributes`)
- **`render_*`** -- Produce output strings from structured data (`render_html`, `render_div`, `render_image`)
- **`resolve_*`** -- Look up a resource/path or infer a value from context (`resolve_template`, `resolve_module_dir`, `resolve_format`)
- **`load_*`** -- Read and parse file contents (`load_page_template`, `load_csl_style`)
- **`build_*`** -- Assemble compound data structures or template variable maps (`build_template_vars`, `build_figure_vars`, `build_author_block`)
- **`apply_*`** -- Transform input by applying something to it (`apply_template`, `apply_overrides`)
- **`escape_*`** -- Escape strings for a target format (`escape_html`, `escape_latex`)
- **`format_*`** -- Format or convert a value for output (`format_width`, `format_height`)
- **`wrap_*`** -- Wrap content in markers for protection (`wrap_raw`, `wrap_shortcode_raw`)
- **`collect_*`** -- Gather items from a sequence (`collect_div_body`, `collect_fenced_body`)
- **`inject_*`** -- Insert content into existing output (`inject_markers`, `inject_reload_script`)
- **`transform_*`** -- Module pipeline stage methods (`transform_document`, `transform_elements`)
- **`assemble_*`** -- Compose a complete output from parts (`assemble_page`)
- **`process_*`** -- Multi-step transformation of data (`process_shortcodes`, `process_results`)

When a function is format-specific, append the format as a qualifier: `number_sections_html`, `escape_latex_line`, `markdown_to_latex`.
