# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**calepin** is a Rust CLI that renders `.qmd` (Quarto-compatible) documents to HTML, LaTeX, Typst, and Markdown. It runs R (via a persistent `Rscript` subprocess) and Python (via a persistent `python3` subprocess) to execute code chunks, processes citations with hayagriva, and resolves cross-references.

The tutorial (`website/basics.qmd`) must be valid Quarto syntax so it can be benchmarked against Quarto and litedown. calepin-specific extensions (plugins, `.hidden` divs, custom shortcodes) are documented in `website/templates.qmd`, `website/filters.qmd`, `website/shortcodes.qmd`, and `website/plugins.qmd`.

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

1. **`.qmd` text** → **`Block`** (parse stage) — Raw text, code chunks, fenced divs, raw blocks. Defined in `calepin/src/types.rs`.
2. **`Block`** → **`Element`** (evaluate stage) — Code is executed, shortcodes expanded, conditional content filtered. Elements are the intermediate representation: `Text`, `CodeSource`, `CodeOutput`, `Figure`, `Div`, `CodeAsis`. Defined in `calepin/src/render/elements.rs`.
3. **`Element`** → **output string** (render stage) — Each element passes through filters (callout enrichment, theorem numbering, template variable filling) then a format-specific template to produce HTML/LaTeX/Typst/Markdown.

### Pipeline stages

1. **Parse** -- YAML front matter (`parse/yaml.rs`), then recursive block parsing into `Block` enum (`parse/blocks.rs`). Chunk options use pipe-only syntax (`#| key: value`). Dashes in option names are normalized to dots internally (`fig-width` -> `fig.width`).
2. **Evaluate** (`engines/mod.rs`) -- Jinja body processing replaces shortcodes (via `jinja::process_body()`), then inline code, then blocks become `Element`s. `engines::evaluate()` orchestrates; `engines::block::evaluate_block()` handles code chunks; `engines::inline::evaluate_inline()` handles inline expressions. Conditional content (`.content-visible`/`.content-hidden` with `when-format`/`unless-format`/`when-meta`/`unless-meta`) filtered here. `.hidden` divs execute but emit nothing.
3. **Load plugin registry** (`registry.rs`) -- Plugins from front matter `calepin: { plugins: [name] }`. Each plugin is a directory with a `plugin.toml` manifest. Resolved from `_calepin/plugins/{name}/`. Built-in plugins (tabset, layout, figure-div, theorem, callout) are appended automatically.
4. **Bibliography** (`bibliography.rs`) -- Citation keys resolved via hayagriva. Operates on `Vec<Element>`.
5. **Render** -- `ElementRenderer` dispatches each element via `render_text()`, `render_div()`, or `render_templated()`. `Element::Text` blocks are converted from markdown to the output format by a shared AST walker (`render/emit/mod.rs`) that traverses comrak's parsed node tree via a `FormatEmitter` trait (one implementation per format in `render/emit/`). Per-element transforms (callout, theorem) live in `render/transform_element/`.
6. **Transform body** -- `OutputRenderer::transform_body()`. Format-specific body mutation (RevealJS slide splitting, LaTeX color definitions).
7. **Cross-ref resolution** (`crossref.rs`) -- Post-rendering pass resolves `@fig-x` references to links/numbers.
8. **Assemble page** -- `OutputRenderer::assemble_page()`. MiniJinja-based page template wrapping (`render/template.rs`) with `{{variable}}` substitution, conditionals, loops, and filters.
9. **Transform document** -- `OutputRenderer::transform_document()`. Post-template transformation (custom format scripts).

## Format Names

Internally, formats use canonical names: `html`, `latex`, `typst`, `markdown`. File extensions for output are: `.html`, `.tex`, `.typ`, `.md`. All template/filter resolution uses the canonical format name (e.g., `theorem.latex`, `calepin.typst`). Raw blocks must use canonical names (```` ```{=latex} ````, not ```` ```{=tex} ````). Format aliases are resolved at the CLI/config level by `resolve_format_from_extension()` and `create_renderer()`.

## Module Layout

### `calepin/src/engines/` — Code execution

The engines module owns all code evaluation — block-level, inline-level, the evaluate loop that drives them, and the language-specific backends.

- `mod.rs` — `evaluate()` orchestrator (walks `Block`s → `Element`s), `execute_chunk()` dispatch, `process_results()` (shared sentinel protocol parser), `make_sentinel()`, `eval_inline()` dispatch, visibility logic (`content_is_visible`, `format_matches`)
- `block.rs` — `evaluate_block()`: chunk → `Element`s (handles `echo`, `eval`, `include`, `results`, `warning`, `message`, figures)
- `inline.rs` — `evaluate_inline()`: `` `{r}` ``/`` `{python}` `` in text → evaluated strings
- `r.rs` — `RSession` (persistent `Rscript` subprocess), R-specific `capture()` (wraps code in R script with graphics device + sentinel protocol), `eval_inline()` (R formatting with digit/comma support)
- `python.rs` — `PythonSession` (persistent `python3` subprocess), Python-specific `capture()` (wraps code in Python script with stdout/matplotlib/warning capture), `eval_inline()`. Shared globals dict persists variables across chunks.
- `util.rs` — `needs_engine()`: scans blocks and body to determine if R/Python runtime should be initialized

### Root (`calepin/src/`) -- Orchestration and core types

- `main.rs` -- Entry point, CLI dispatch
- `pipeline.rs` -- Core render pipeline orchestrator: explicit stage calls (parse -> evaluate -> bibliography -> render -> transform_body -> crossref -> assemble_page -> transform_document)
- `bibliography.rs` -- Citation resolution via hayagriva. Operates on `Vec<Element>` between evaluate and render.
- `crossref.rs` -- Cross-reference resolution. Operates on the rendered body string after transform_body.
- `cli.rs` -- CLI argument parsing (clap) + `cwarn!` macro + `is_collection_config()`
- `util.rs` -- `slugify()`, `escape_html()`

### `calepin/src/render/` -- Element rendering

- `elements.rs` -- `ElementRenderer`: dispatches each element via `render_text()`, `render_div()`, `render_templated()`
- `div.rs` -- Div rendering pipeline: plugin registry dispatch (structural -> filter -> template)
- `span.rs` -- Span rendering pipeline: plugin registry dispatch -> template -> fallback
- `convert.rs` -- Comrak options, `ImageAttrs` parsing, `render_html()`/`render_typst()`/`render_inline()` entry points
- `template.rs` -- MiniJinja-based template rendering (`apply_template()`) + page template loading + `build_template_vars()`
- `markers.rs` -- Marker systems for protecting content through conversion
- `metadata.rs` -- Author/citation/appendix formatting
- `highlighting.rs` -- Syntax highlighting via syntect
- `math.rs` -- Math format conversion (LaTeX/Typst)
- `svg.rs` -- SVG to PDF conversion

### `calepin/src/render/emit/` -- AST emitters

Shared AST walker + format-specific implementations. All formats share a single comrak traversal via the `FormatEmitter` trait.

- `mod.rs` -- `FormatEmitter` trait + `walk_ast()`. Heading ID extraction, section numbering, footnote pre-pass, table state, math/marker protection.
- `html.rs` -- `HtmlEmitter`
- `latex.rs` -- `LatexEmitter`
- `typst.rs` -- `TypstEmitter`
- `markdown.rs` -- `MarkdownEmitter`

### `calepin/src/render/transform_element/` -- Per-element transforms

Enrich template variables or produce final output during the render stage. Dispatched per-element by the div pipeline.

- `mod.rs` -- `Filter` trait + `FilterResult` enum
- `callout.rs` -- Callout enrichment: title, icon, collapse/appearance
- `theorem.rs` -- Theorem auto-numbering: per-type counters, injects `{{number}}`
- `code.rs` -- Code block template variable filling (syntax highlighting)
- `figure.rs` -- Figure template vars, image helpers, path resolution

### `calepin/src/formats/` -- Output format backends

Each format implements `OutputRenderer` with stage-specific methods: `transform_body()`, `assemble_page()`, `transform_document()`.

- `mod.rs` -- `OutputRenderer` trait, `create_renderer()`, `CustomRenderer` (user-defined formats via `_calepin/formats/{name}.toml`)
- `html.rs`, `latex.rs`, `typst.rs`, `markdown.rs`, `revealjs.rs`, `word.rs`

### `calepin/src/structures/` -- Structural div handlers

- `tabset.rs` -- `.panel-tabset` -> tabs (HTML) or plain sections
- `layout.rs` -- `layout-ncol`/`layout-nrow`/`layout` -> CSS Grid / minipage / #grid
- `figure.rs` -- Figure div rendering
- `table.rs` -- Table div rendering

### `plugins/` -- Legacy WASM plugin sources

Legacy WASM plugin sources (not used at runtime). These are historical and may be removed.

## Plugin System

All extensibility flows through the `PluginRegistry` (`plugins/registry.rs`). A plugin is a directory with a `plugin.toml` manifest declaring its capabilities.

### Plugin types

- **Built-in structural** (`BuiltinStructural`) -- Receive raw `&[Element]` children + render closure. Run before child rendering. Used by: tabset, layout, figure-div, table-div.
- **Built-in filter** (`BuiltinFilter`) -- Implement the `Filter` trait (in `render/transform_element/`). Run after child rendering. Used by: theorem, callout.

### Dispatch order

`render/div.rs` iterates matching plugins in registry order (user plugins first, then built-in):

1. For each matching plugin (by classes/attrs/id_prefix/formats):
   - **Structural** -> call with raw children, return if handled
   - **Filter** -> lazy-render children, call, return if `Rendered`; accumulate vars if `Continue`
2. **Template lookup** -- explicit override -> class-based -> `div` fallback

### plugin.toml manifest

```toml
name = "myplugin"
version = "0.1.0"
description = "What this plugin does"

[filter]
match.classes = ["myclass"]     # CSS classes (OR'd)
match.attrs = ["my-attr"]       # Attribute names (OR'd)
match.id_prefix = "fig-"        # ID prefix
match.formats = ["html"]        # Output formats (omit = all)
contexts = ["div", "span"]      # Default: both

[elements]
dir = "elements/"

[templates]
dir = "templates/"

csl = "style.csl"

[format]
name = "myformat"
base = "html"
extension = "html"
```

### Resolution

1. `_calepin/plugins/{name}/plugin.yml` (project)

### CLI

- `calepin plugin init <name>` — scaffold a new plugin
- `calepin plugin list` — list available plugins

## Raw Output Protection

Format-specific output from span templates and shortcodes must survive markdown-to-format conversion without being re-escaped. All markers use Unicode noncharacters (`\u{FFFF}` start, `\u{FFFE}` end) as delimiters — these cannot appear in legitimate text, making collisions impossible. Input is sanitized by `markers::sanitize()` at the start of the pipeline. When protecting content from processing elsewhere (e.g., verbatim blocks in cross-ref resolution), always use the marker infrastructure from `render/markers.rs` rather than plain-text sentinel strings.

Marker types (single-char prefix between delimiters):

- **`M`** — Math expressions (`$...$` and `$$...$$`). Use `\$` for a literal dollar sign.
- **`D`** — Escaped dollar signs. Resolved per-format by `markers::resolve_escaped_dollars()`: HTML wraps in `<span class="nodollar">` (MathJax ignores it), LaTeX/Typst produce `\$`.
- **`L`** — Equation labels (`{#eq-...}` after display math).
- **`R`** — Raw span/template output. Stored in `ElementRenderer::raw_fragments`. Also used locally for temporary protection (e.g., verbatim blocks in cross-ref resolution).
- **`S`** — Shortcode raw output.
- **`X`** — Escaped shortcode literals.

## calepin-specific YAML

calepin-specific settings are nested under the `calepin:` key in front matter:

```yaml
calepin:
  plugins:
    - txtfmt
```

Standard Quarto fields (`title`, `author`, `bibliography`, `format`, etc.) remain at the top level.

## Template and Filter Resolution

Templates use Jinja syntax (`{{variable}}`, `{% if %}`, `{% for %}`, filters, macros). Template variable names use underscores (e.g., `id_attr`, `plain_title`). CSS class names in source documents keep dashes; the template resolver normalizes dashes to underscores when looking up templates by class name.

Resolution order: plugin-provided dirs (in plugin order) → `_calepin/{elements,templates}/` → built-in.

- Element templates: plugin `elements/` dir → `_calepin/elements/{name}.{format}` → built-in
- Page templates: plugin `templates/` dir → `_calepin/templates/calepin.{format}` → built-in
- CSL: plugin `csl` field → `_calepin/templates/calepin.csl` → built-in
- Filters: provided via plugin `filter` capability (subprocess executables with `plugin.yml`)
- Shortcodes: provided via plugin `shortcode` capability (registered as Jinja functions)
- Custom formats: provided via plugin `format` capability, or `_calepin/formats/{name}.yaml`

## Chunk Options

Both pipe syntax (`#| key: value`) and header key-value pairs (`{r, echo=FALSE}`) are accepted. Header options are converted internally to pipe-equivalent options; when both are present, pipe comments take precedence. Option names use dashes (`fig-width`), normalized to dots internally. `label` is rejected in pipe comments -- it must be in the header.

## Jinja Body Processing

The `.qmd` body text is processed as a Jinja template during the evaluate stage (`jinja_engine.rs`). Code blocks and inline code are protected from Jinja evaluation. Use `#| jinja: true` chunk option to opt-in to Jinja processing inside a code chunk.

Built-in Jinja functions (replace old `{{< shortcode >}}` syntax):

- `{{ pagebreak() }}` — format-specific page break
- `{{ video(url="...", width="...", height="...", title="...") }}` — video embed
- `{{ kbd(keys=["Ctrl", "C"]) }}` — keyboard shortcuts
- `{{ lipsum(paragraphs=2) }}` — placeholder lorem ipsum text (also `sentences`, `words`)
- `{{ placeholder(width=600, height=400) }}` — placeholder image (also `text`, `color`)

Context variables:
- `{{ meta.title }}`, `{{ meta.author }}`, `{{ meta.date }}`, etc. — document metadata
- `{{ var.key.subkey }}` — non-standard front matter fields (with nesting)
- `{{ env.HOME }}`, `{{ env.USER }}`, etc. — system environment variables
- `{{ format }}` — current output format

File inclusion uses Jinja's include tag: `{% include "file.qmd" %}` (pre-parse directive, runs before block parsing via `jinja_engine::expand_includes()`). Escaping uses `{% raw %}...{% endraw %}`.

Plugin shortcodes are registered as Jinja functions via the plugin registry.

## Dependencies

- `comrak` — CommonMark + GFM markdown parsing/rendering
- `hayagriva` — Citation/bibliography processing
- `syntect` — Syntax highlighting
- `minijinja` — Template engine for element/page templates and body processing
- `clap` + `clap_complete` — CLI and shell completions
- `saphyr` — YAML parsing (DOM-style `YamlOwned` enum, not serde-based)

## Function Naming Convention

Use `verb_noun` or `verb_noun_qualifier` format. Consistent verbs for similar operations:

- **`parse_*`** — Convert text/input into structured data (`parse_body`, `parse_yaml`, `parse_attributes`)
- **`render_*`** — Produce output strings from structured data (`render_html`, `render_div`, `render_image`)
- **`resolve_*`** — Look up a resource/path or infer a value from context (`resolve_path`, `resolve_plugin`, `resolve_format`)
- **`load_*`** — Read and parse file contents (`load_plugins`, `load_page_template`, `load_csl_style`)
- **`build_*`** — Assemble compound data structures or template variable maps (`build_template_vars`, `build_figure_vars`, `build_author_block`, `build_template_output`)
- **`apply_*`** — Transform input by applying something to it (`apply_template`, `apply_image_attrs_html`, `apply_overrides`)
- **`escape_*`** — Escape strings for a target format (`escape_html`, `escape_latex`, `escape_code_for_format`)
- **`format_*`** — Format or convert a value for output (`format_width`, `format_height`, `format_narrative`)
- **`wrap_*`** — Wrap content in markers for protection through conversion (`wrap_raw`, `wrap_shortcode_raw`, `wrap_raw_output`)
- **`collect_*`** — Gather items from a sequence (`collect_div_body`, `collect_inline_code`, `collect_fenced_body`)
- **`inject_*`** — Insert content into existing output (`inject_markers`, `inject_reload_script`)
- **`transform_*`** -- Pipeline stage methods on `OutputRenderer` (`transform_body`, `transform_document`) and per-element transforms (`transform_element/`)
- **`assemble_*`** -- Compose a complete output from parts (`assemble_page`)
- **`process_*`** -- Multi-step transformation of data (`process_shortcodes`, `process_results`)

When a function is format-specific, append the format as a qualifier: `number_sections_html`, `escape_latex_line`, `markdown_to_latex`.
