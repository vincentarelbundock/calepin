# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**calepin** is a Rust CLI that renders `.qmd` (Quarto-compatible) documents to HTML, LaTeX, Typst, and Markdown. It runs R (via a persistent `Rscript` subprocess) and Python (via a persistent `python3` subprocess) to execute code chunks, processes citations with hayagriva, and resolves cross-references.

The tutorial (`website/basics.qmd`) must be valid Quarto syntax so it can be benchmarked against Quarto and litedown. calepin-specific extensions (modules, `.hidden` divs, custom shortcodes) are documented in `website/templates.qmd`, `website/filters.qmd`, `website/shortcodes.qmd`, and `website/plugins.qmd`.

When referring to the software by name in documentation or notebooks, always write *Calepin* (italic, capital C).

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
make bench          # Benchmark vs Quarto on bench/*.qmd (requires hyperfine)
make plugins        # Build WASM plugins (requires wasm32-unknown-unknown target)
make site           # Build debug + serve static site from website/
make clean          # Remove build artifacts
make flush          # Delete all *_cache and *_files directories
make prof           # Profile with per-stage timing (set PROF_FILE=path/to/file.qmd)
make prof-samply    # Profile with samply in browser
```

Run a single test: `cargo test test_name`

CLI: `calepin <input.qmd> [-o PATH] [-t TARGET] [-s KEY=VALUE ...] [-q] [--base FORMAT] [--completions SHELL]`

**Important**: website/ must be rendered with `cd website && ../calepin/target/debug/calepin file.qmd` so that `_calepin/` overrides are found relative to the working directory. `make docs` handles this.

## Architecture

### Data flow

The pipeline transforms data through three representations:

1. **`.qmd` text** -> **`Block`** (parse stage) -- Raw text, code chunks, fenced divs, raw blocks. Defined in `base/types.rs`.
2. **`Block`** -> **`Element`** (evaluate stage) -- Code is executed, shortcodes expanded, conditional content filtered. Elements are: `Text`, `CodeSource`, `CodeOutput`, `Figure`, `Div`, `CodeAsis`. Defined in `base/types.rs`.
3. **`Element`** -> **output string** (render stage) -- Each element passes through var builders and partials to produce HTML/LaTeX/Typst/Markdown.

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

1. **Parse** -- TOML front matter is parsed (`config/parse.rs`) into metadata; non-TOML front matter (e.g., YAML) is silently ignored. Recursive block parsing into `Block` enum (`parse/blocks.rs`).
2. **Evaluate** (`engines/mod.rs`) -- Jinja body processing, code execution, blocks become `Element`s.
3. **Bibliography** (`references/bibliography.rs`) -- Citation keys resolved via hayagriva.
4. **TransformElement** -- Pre-render element mutations. Modules implementing `TransformElement` receive each element and can mutate it (e.g., `convert_svg_pdf` rewrites SVG figure paths to PDF).
5. **Render** -- `ElementRenderer` dispatches each element. Divs go through the module registry (`TransformElementChildren` for structural rewriting, then partial lookup). Code/figure elements go through `BuildElementVars` then partials.
6. **Cross-ref resolution** (`references/crossref.rs`) -- `@fig-x` references resolved to links/numbers.
7. **Assemble page** -- MiniJinja page template wrapping (`render/template.rs`).
8. **TransformDocument** -- Post-assembly document transforms. Modules receive the full document string and can modify it (highlight CSS injection, footnote appending, slide splitting, image embedding).
9. **Write** -- File output or pandoc conversion.

### Module system

All extensibility flows through the `ModuleRegistry` (`modules/registry.rs`). Three module kinds:

| Trait | `ModuleKind` | When | What |
|---|---|---|---|
| `TransformElement` | `Element` | Pre-render | Mutate individual elements (pipeline handles tree recursion) |
| `TransformElementChildren` | `ElementChildren` | During render, per div | Rewrite div children (tabset, layout) |
| `TransformDocument` | `Document` | Post-assembly | Transform the full document string |
| -- | `Noop` | -- | Partial/template providers only |

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
| `split_slides` | TransformDocument | Split body into `<section>` slides (RevealJS) |
| `embed_images` | TransformDocument | Base64-encode `<img>` sources (HTML) |

### Target configuration

Each output format is a Target in `config/document.toml`. Targets declare engine, modules, crossref strategy, and other options:

```toml
[targets.html]
engine = "html"
modules = ["highlight", "append_footnotes", "embed_images"]
crossref = "html"

[targets.latex]
engine = "latex"
modules = ["highlight", "convert_svg_pdf"]
crossref = "latex"
```

User targets in `_calepin.toml` inherit from built-in targets via `inherits`.

### FormatPipeline

`FormatPipeline` (`render/formats.rs`) reads pipeline config from the Target and dispatches to modules at each stage. Created via `FormatPipeline::from_target()` or `FormatPipeline::from_writer()`.

## Format Names

Internally, formats use canonical writer names: `html`, `latex`, `typst`, `markdown`. File extensions: `.html`, `.tex`, `.typ`, `.md`. Partial resolution uses the writer name (e.g., `partials/html/figure.html`). Raw blocks use canonical names (```` ```{=latex} ````).

## Source Layout

### `calepin/src/` -- Top-level

- `main.rs` -- Entry point (only file at src/ root)

### `cli/` -- CLI and command handlers

- `args.rs` -- CLI argument parsing (clap) + `cwarn!` macro
- `render.rs`, `preview.rs`, `info.rs`, `new.rs`, `flush.rs` -- Command handlers

### `config/` -- Configuration and project context

- `types.rs` -- `Metadata`, `Author`, `Target` structs
- `parse.rs` -- TOML front matter parsing: `split_frontmatter()`, `parse_metadata()`
- `merge.rs` -- Metadata merge logic (last wins)
- `targets.rs` -- Target resolution and inheritance
- `load.rs` -- Project config loading, `LanguageConfig`, `ContentSection`
- `context.rs` -- `ProjectContext`: resolves project config and target for a render
- `document.toml`, `shared.toml`, `collection.toml`, `modules.toml` -- Embedded default configs

### `render/` -- Element rendering and pipeline

- `pipeline.rs` -- Core render pipeline orchestrator: parse, evaluate, render
- `formats.rs` -- `FormatPipeline`: dispatches modules at each pipeline stage
- `elements.rs` -- `ElementRenderer`: dispatches each element, holds pre-compiled template env
- `div.rs` -- Div rendering pipeline: module dispatch, auto-numbering, partial lookup
- `span.rs` -- Span rendering pipeline
- `vars.rs` -- `BuildElementVars` trait + `BuildCodeVars`: per-element template var builders
- `convert.rs` -- Comrak options, `ImageAttrs`, `render_inline()` entry points
- `template.rs` -- MiniJinja template engine: `apply_template()`, page template loading, `build_template_vars()`
- `markers.rs` -- Unicode marker system for protecting content through conversion
- `metadata.rs` -- Author/citation/appendix formatting via partials

### `modules/` -- Module system and built-in modules

- `registry.rs` -- `ModuleRegistry`, `TransformElement`, `TransformElementChildren`, `TransformDocument` traits, `ModuleKind`, `ModuleContext`, `ModuleResult`, built-in module registration
- `manifest.rs` -- `module.toml` parsing: `ModuleManifest`, `MatchRule`, `MatchSpec`
- `transform_document.rs` -- `TransformDocument` trait + `ScriptTransformDocument` (user script execution)
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
- `escape.rs` -- Format-specific code escaping
- `lipsum.rs` -- Lorem ipsum text generation
- `cache.rs` -- Hash-based page cache for incremental builds
- `date.rs` -- Date formatting and resolution helpers

### `partials/` -- Built-in Jinja templates (embedded at compile time)

Per-engine partials for elements, page templates, shortcodes:
`partials/{html,latex,typst,markdown,revealjs,website,book}/`

Website template icons live in `partials/website/icons/` (used via `{% include %}`).

User overrides: `_calepin/partials/{engine}/{name}.{ext}`

### `scaffold/` -- Project scaffolding and shared assets

- `website/`, `book/`, `notebook/` -- Starter project templates for `calepin new`
- `assets/` -- Shared website assets (CSS, JS, social icons) copied to output at build time

### Other directories

- `engines/` -- Code execution: R, Python, shell subprocess management
- `parse/` -- Block parsing: `.qmd` text -> `Block` enum
- `references/` -- Bibliography (`bibliography.rs`) + cross-references (`crossref.rs`)
- `jinja/` -- Jinja body processing: `{% include %}` expansion, code block protection, template context
- `base/` -- Core types (`types.rs`), paths (`paths.rs`), utilities (`util.rs`, `value.rs`)
- `collection/` -- Multi-document builds (site/book rendering)
- `preview/` -- Live preview server with hot reload

## Partials and Module Resolution

Partials use Jinja syntax (`{{variable}}`, `{% if %}`, `{% for %}`). Variable names use underscores. CSS class names in source documents keep dashes; the resolver normalizes dashes to underscores for lookup.

**Partial resolution order** (first match wins):
1. Module element dirs (in registry order)
2. `_calepin/partials/{target}/{name}.{ext}` (target-specific)
3. `_calepin/partials/{engine}/{name}.{ext}` (engine-specific)
4. `_calepin/partials/common/{name}.jinja` (format-agnostic)
5. Built-in `partials/{engine}/{name}.{ext}` (embedded in binary)

**Module resolution**: `_calepin/modules/{name}/module.toml`

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

Format-specific output from span partials must survive markdown-to-format conversion without being re-escaped. All markers use Unicode noncharacters (`\u{FFFF}` start, `\u{FFFE}` end) as delimiters. Input is sanitized by `markers::sanitize()` at the start of the pipeline.

Marker types (single-char prefix between delimiters):

- **`M`** -- Math expressions (`$...$` and `$$...$$`). Use `\$` for a literal dollar sign.
- **`D`** -- Escaped dollar signs. Resolved per-format by `markers::resolve_escaped_dollars()`.
- **`L`** -- Equation labels (`{#eq-...}` after display math).
- **`R`** -- Raw span/partial output (including built-in spans like pagebreak, video, placeholder).

## Configuration

Documents can carry TOML front matter between `---` delimiters. Non-TOML front matter (e.g., YAML) is silently ignored.

**Merge order** (last wins): built-in defaults < `_calepin/calepin.toml` < `{stem}_calepin/calepin.toml` (sidecar) < TOML front matter < CLI (`-s`)

**Sidecar directories**: Each document can have a `{stem}_calepin/` directory alongside it, mirroring the `_calepin/` structure (partials, modules, cache, files). For websites, `_calepin/` is the shared sidecar for all pages.

calepin-specific settings are nested under the `[calepin]` table:

```toml
[calepin]
plugins = ["txtfmt"]
```

Standard fields (`title`, `author`, `bibliography`, `format`, etc.) are top-level keys.

## Chunk Options

Both pipe syntax (`#| key: value`) and header key-value pairs (`{r, echo=FALSE}`) are accepted. Header options are converted internally to pipe-equivalent options; when both are present, pipe comments take precedence. Option names use dashes (`fig-width`), normalized to dots internally. `label` is rejected in pipe comments -- it must be in the header.

## Jinja Body Processing

The `.qmd` body text is processed as a Jinja template during the evaluate stage (`jinja_engine.rs`). Code blocks and inline code are protected from Jinja evaluation. Use `#| jinja: true` chunk option to opt-in to Jinja processing inside a code chunk.

Context variables:
- `{{ meta.title }}`, `{{ meta.author }}`, `{{ meta.date }}`, etc. -- document metadata
- `{{ var.key.subkey }}` -- non-standard front matter fields (with nesting)
- `{{ env.HOME }}`, `{{ env.USER }}`, etc. -- system environment variables
- `{{ format }}` -- current output format

File inclusion: `{% include "file.qmd" %}` (pre-parse, runs before block parsing). Escaping: `{% raw %}...{% endraw %}`.

## Built-in Spans

Bracketed spans `[content]{.class key=value}` are processed during rendering. Built-in spans (output driven by per-engine partials in `partials/{engine}/`):

- `[]{.pagebreak}` -- format-specific page break
- `[]{.video url="..." width="..." height="..." title="..."}` -- video embed
- `[]{.lorem paragraphs=2}` -- placeholder lorem ipsum text (also `sentences`, `words`)
- `[]{.placeholder width=600 height=400}` -- placeholder image (also `text`, `color`)

## Dependencies

- `comrak` -- CommonMark + GFM markdown parsing/rendering
- `hayagriva` -- Citation/bibliography processing
- `syntect` -- Syntax highlighting
- `minijinja` -- Template engine for element/page partials and body processing
- `clap` + `clap_complete` -- CLI and shell completions
- `toml` + `serde` -- TOML config parsing (front matter, `_calepin.toml`, sidecar config)
- `usvg` + `svg2pdf` -- SVG-to-PDF conversion for LaTeX targets

## WASM Plugins

Plugins are `.wasm` files in `_calepin/plugins/`, declared in front matter under `[calepin] plugins = ["name"]`. Plugin source lives in `plugins/{name}/` (each a Rust crate targeting `wasm32-unknown-unknown`). Build all plugins with `make plugins`.

Custom output formats can be defined via `_calepin/formats/{name}.yaml` with `base`, `extension`, and `plugin` fields.

## Profiling

Build with `make prof-build` (release + debug symbols). Profile a specific file:

```
make prof PROF_FILE=bench/text.qmd           # Per-stage timing (CALEPIN_TIMING=1)
make prof-samply PROF_FILE=bench/text.qmd    # Open samply in browser
make prof-batch PROF_N=100 PROF_FILE=bench/text.qmd  # Batch N iterations
```

## Function Naming Convention

Use `verb_noun` or `verb_noun_qualifier` format. Consistent verbs for similar operations:

- **`parse_*`** -- Convert text/input into structured data (`parse_body`, `parse_metadata`, `parse_attributes`)
- **`render_*`** -- Produce output strings from structured data (`render_html`, `render_div`, `render_image`)
- **`resolve_*`** -- Look up a resource/path or infer a value from context (`resolve_partial`, `resolve_module_dir`, `resolve_format`)
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
