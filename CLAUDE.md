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
make build          # Debug build (R only)
make release        # Optimized release build (R only)
make install        # Install to ~/.cargo/bin + shell completions
make check          # Fast compile check (no linking)
make test           # cargo test
make docs           # Render all .qmd files in website/ to all formats
make bench          # Benchmark vs litedown and Quarto (uses website/basics.qmd)
```

Run a single test: `cargo test test_name`

CLI: `calepin <input.qmd> [-o PATH] [-t TARGET] [-s KEY=VALUE ...] [-q] [--base FORMAT] [--completions SHELL]`

Batch mode: `calepin --batch manifest.json` or `calepin --batch - < manifest.json`. Add `--batch-stdout` to get rendered bodies in JSON output instead of writing files. See `batch.rs` for manifest format.

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

1. **Parse** -- YAML front matter (`config/parse.rs`), then recursive block parsing into `Block` enum (`parse/blocks.rs`).
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

`FormatPipeline` (`formats.rs`) replaces the old `OutputRenderer` trait. It reads pipeline config from the Target and dispatches to modules at each stage. Created via `FormatPipeline::from_target()` or `FormatPipeline::from_engine()`.

## Format Names

Internally, formats use canonical engine names: `html`, `latex`, `typst`, `markdown`. File extensions: `.html`, `.tex`, `.typ`, `.md`. Partial resolution uses the engine name (e.g., `partials/html/figure.html`). Raw blocks use canonical names (```` ```{=latex} ````).

## Source Layout

### `calepin/src/` -- Top-level files

- `main.rs` -- Entry point
- `pipeline.rs` -- Core render pipeline orchestrator
- `formats.rs` -- `FormatPipeline`: dispatches modules at each pipeline stage
- `context.rs` -- `ProjectContext`: resolves project config and target

### `cli/` -- CLI and command handlers

- `args.rs` -- CLI argument parsing (clap) + `cwarn!` macro
- `render.rs`, `preview.rs`, `info.rs`, `new.rs`, `flush.rs` -- Command handlers

### `modules/` -- Module system and built-in modules

- `registry.rs` -- `ModuleRegistry`, `TransformElement`, `TransformElementChildren`, `TransformDocument` traits, `ModuleKind`, `ModuleContext`, `ModuleResult`, built-in module registration
- `manifest.rs` -- `module.toml` parsing: `ModuleManifest`, `MatchRule`, `MatchSpec`
- `transform_document.rs` -- `TransformDocument` trait + `ScriptTransformDocument` (user script execution)
- `highlight/` -- Syntax highlighting: `Highlighter`, themes (`.tmTheme` files), CSS generation, LaTeX color defs, `TransformDocument` for injecting into assembled pages
- `convert_svg_pdf/` -- `TransformElement`: SVG-to-PDF figure conversion
- `convert_math/` -- LaTeX-to-Typst math converter (parser, AST, emitter, symbols)
- `tabset/` -- `TransformElementChildren`: panel-tabset -> HTML tabs
- `layout/` -- `TransformElementChildren`: layout grids (CSS Grid, LaTeX minipage, Typst grid)
- `figure/` -- Figure div helper functions
- `table/` -- Table div helper functions
- `append_footnotes/` -- `TransformDocument`: append HTML footnote section
- `split_slides/` -- `TransformDocument`: split body into RevealJS slides
- `embed_images/` -- `TransformDocument`: base64-encode images

### `emit/` -- AST emitters (the 4 irreducible atoms)

Shared AST walker + format-specific implementations. All formats share a single comrak traversal via the `FormatEmitter` trait.

- `mod.rs` -- `FormatEmitter` trait + `walk_ast()`, heading IDs, section numbering, footnotes, tables
- `html.rs` -- `HtmlEmitter`
- `latex.rs` -- `LatexEmitter`
- `typst.rs` -- `TypstEmitter`
- `markdown.rs` -- `MarkdownEmitter`

### `render/` -- Element rendering infrastructure

- `elements.rs` -- `ElementRenderer`: dispatches each element, holds pre-compiled template env
- `div.rs` -- Div rendering pipeline: module dispatch (TransformElementChildren), auto-numbering, partial lookup
- `span.rs` -- Span rendering pipeline
- `filter/` -- Per-element var builders (not modules, called directly by ElementRenderer)
  - `mod.rs` -- `BuildElementVars` trait
  - `code.rs` -- `BuildCodeVars`: syntax highlighting vars
  - `figure.rs` -- `BuildFigureVars`: image path, dimensions, alignment vars
  - `theorem.rs` -- `theorem_prefix()`: cross-ref ID prefix mapping
- `convert.rs` -- Comrak options, `ImageAttrs`, `render_inline()` entry points
- `template.rs` -- MiniJinja template engine: `apply_template()`, page template loading, `build_template_vars()`
- `markers.rs` -- Unicode marker system for protecting content through conversion
- `metadata.rs` -- Author/citation/appendix formatting via partials
- `typst_compile.rs` -- Typst PDF compilation

### `partials/` -- Built-in Jinja templates (embedded at compile time)

Per-engine partials for elements, page templates, shortcodes:
`partials/{html,latex,typst,markdown,revealjs,website,book}/`

User overrides: `_calepin/partials/{engine}/{name}.{ext}`

### Other directories

- `config/` -- Metadata types (`types.rs`, `parse.rs`, `merge.rs`) + embedded TOML configs (`document.toml`, `shared.toml`, `collection.toml`)
- `engines/` -- Code execution: R, Python, shell subprocess management
- `parse/` -- Block parsing: `.qmd` text -> `Block` enum
- `references/` -- Bibliography (`bibliography.rs`) + cross-references (`crossref.rs`)
- `jinja/` -- Jinja body processing, shortcode functions, lipsum
- `base/` -- Core types (`types.rs`), paths (`paths.rs`), utilities (`util.rs`, `value.rs`)
- `project/` -- Target resolution (`targets.rs`), content discovery (`content.rs`)
- `collection/` -- Multi-document builds (site/book rendering)
- `preview/` -- Live preview server with hot reload
- `assets/` -- Website CSS/JS + scaffold files (404.qmd, index.qmd)
- `scaffold/` -- (removed, merged into assets/website/)

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

Format-specific output from span partials and shortcodes must survive markdown-to-format conversion without being re-escaped. All markers use Unicode noncharacters (`\u{FFFF}` start, `\u{FFFE}` end) as delimiters. Input is sanitized by `markers::sanitize()` at the start of the pipeline.

Marker types (single-char prefix between delimiters):

- **`M`** -- Math expressions (`$...$` and `$$...$$`). Use `\$` for a literal dollar sign.
- **`D`** -- Escaped dollar signs. Resolved per-format by `markers::resolve_escaped_dollars()`.
- **`L`** -- Equation labels (`{#eq-...}` after display math).
- **`R`** -- Raw span/partial output.
- **`S`** -- Shortcode raw output.
- **`X`** -- Escaped shortcode literals.

## calepin-specific YAML

calepin-specific settings are nested under the `calepin:` key in front matter:

```yaml
calepin:
  plugins:
    - txtfmt
```

Standard Quarto fields (`title`, `author`, `bibliography`, `format`, etc.) remain at the top level.

## Chunk Options

Both pipe syntax (`#| key: value`) and header key-value pairs (`{r, echo=FALSE}`) are accepted. Header options are converted internally to pipe-equivalent options; when both are present, pipe comments take precedence. Option names use dashes (`fig-width`), normalized to dots internally. `label` is rejected in pipe comments -- it must be in the header.

## Jinja Body Processing

The `.qmd` body text is processed as a Jinja template during the evaluate stage (`jinja_engine.rs`). Code blocks and inline code are protected from Jinja evaluation. Use `#| jinja: true` chunk option to opt-in to Jinja processing inside a code chunk.

Built-in Jinja functions (output driven by per-engine partials in `partials/{engine}/`):

- `{{ pagebreak() }}` -- format-specific page break
- `{{ video(url="...", width="...", height="...", title="...") }}` -- video embed
- `{{ lipsum(paragraphs=2) }}` -- placeholder lorem ipsum text (also `sentences`, `words`)
- `{{ placeholder(width=600, height=400) }}` -- placeholder image (also `text`, `color`)

Context variables:
- `{{ meta.title }}`, `{{ meta.author }}`, `{{ meta.date }}`, etc. -- document metadata
- `{{ var.key.subkey }}` -- non-standard front matter fields (with nesting)
- `{{ env.HOME }}`, `{{ env.USER }}`, etc. -- system environment variables
- `{{ format }}` -- current output format

File inclusion: `{% include "file.qmd" %}` (pre-parse, runs before block parsing). Escaping: `{% raw %}...{% endraw %}`.

## Dependencies

- `comrak` -- CommonMark + GFM markdown parsing/rendering
- `hayagriva` -- Citation/bibliography processing
- `syntect` -- Syntax highlighting
- `minijinja` -- Template engine for element/page partials and body processing
- `clap` + `clap_complete` -- CLI and shell completions
- `saphyr` -- YAML parsing (DOM-style `YamlOwned` enum, not serde-based)
- `usvg` + `svg2pdf` -- SVG-to-PDF conversion for LaTeX targets

## Function Naming Convention

Use `verb_noun` or `verb_noun_qualifier` format. Consistent verbs for similar operations:

- **`parse_*`** -- Convert text/input into structured data (`parse_body`, `parse_yaml`, `parse_attributes`)
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
