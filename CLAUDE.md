# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**calepin** is a Rust CLI that renders `.qmd` (Quarto-compatible) documents to HTML, LaTeX, Typst, and Markdown. It embeds R (via extendr) and Python (via pyo3) runtimes to execute code chunks, processes citations with hayagriva, and resolves cross-references.

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

CLI: `calepin <input.qmd> [-o PATH] [-f FORMAT] [-s KEY=VALUE ...] [-q] [--compile] [--preview] [--completions SHELL]`

Batch mode: `calepin --batch manifest.json` or `calepin --batch - < manifest.json`. Add `--batch-stdout` to get rendered bodies in JSON output instead of writing files. See `batch.rs` for manifest format.

**Important**: website/ must be rendered with `cd website && ../calepin/target/debug/calepin file.qmd` so that `_calepin/` overrides are found relative to the working directory. `make docs` handles this.

## Architecture

### Data flow

The pipeline transforms data through three representations:

1. **`.qmd` text** → **`Block`** (parse stage) — Raw text, code chunks, fenced divs, raw blocks. Defined in `calepin/src/types.rs`.
2. **`Block`** → **`Element`** (evaluate stage) — Code is executed, shortcodes expanded, conditional content filtered. Elements are the intermediate representation: `Text`, `CodeSource`, `CodeOutput`, `Figure`, `Div`, `CodeAsis`. Defined in `calepin/src/render/elements.rs`.
3. **`Element`** → **output string** (render stage) — Each element passes through filters (callout enrichment, theorem numbering, template variable filling) then a format-specific template to produce HTML/LaTeX/Typst/Markdown.

### Pipeline stages

1. **Parse** — YAML front matter (`parse/yaml.rs`), then recursive block parsing into `Block` enum (`parse/blocks.rs`). Chunk options use pipe-only syntax (`#| key: value`). Dashes in option names are normalized to dots internally (`fig-width` → `fig.width`).
2. **Evaluate** (`engines/mod.rs`) — Tera body processing replaces shortcodes (via `tera_engine::process_body()`), then inline code, then blocks become `Element`s. `engines::evaluate()` orchestrates; `engines::block::evaluate_block()` handles code chunks; `engines::inline::evaluate_inline()` handles inline expressions. Conditional content (`.content-visible`/`.content-hidden` with `when-format`/`unless-format`/`when-meta`/`unless-meta`) filtered here. `.hidden` divs execute but emit nothing.
3. **Load plugin registry** (`registry.rs`) — Plugins from front matter `calepin: { plugins: [name] }`. Each plugin is a directory with a `plugin.yml` manifest. Resolved from `_calepin/plugins/{name}/` → `~/.config/calepin/plugins/{name}/`. Built-in plugins (tabset, layout, figure-div, theorem, callout) are appended automatically.
4. **Bibliography** (`filters/bibliography.rs`) — Citation keys resolved via hayagriva.
5. **Cross-ref markers** (`filters/crossref.rs`) — Inject anchors into elements.
6. **Render** — `ElementRenderer` dispatches each element to the appropriate filter, then applies a template. `OutputRenderer` wraps the result in a page template.
7. **Cross-ref resolution** — Post-processing pass resolves `@fig-x` references to links/numbers.
8. **Page template** (`render/template.rs`) — Tera-based rendering with `{{variable}}` substitution, conditionals, loops, and filters.

## Format Names

Internally, formats use canonical names: `html`, `latex`, `typst`, `markdown`. File extensions for output are: `.html`, `.tex`, `.typ`, `.md`. All template/filter resolution uses the canonical format name (e.g., `theorem.latex`, `calepin.typst`). Aliases (`tex`, `pdf`, `typ`, `md`) are only accepted in `format_matches()` for Quarto raw block compatibility.

## Module Layout

### `calepin/src/engines/` — Code execution

The engines module owns all code evaluation — block-level, inline-level, the evaluate loop that drives them, and the language-specific backends.

- `mod.rs` — `evaluate()` orchestrator (walks `Block`s → `Element`s), `execute_chunk()` dispatch, `process_results()` (shared sentinel protocol parser), `make_sentinel()`, `eval_inline()` dispatch, visibility logic (`content_is_visible`, `format_matches`)
- `block.rs` — `evaluate_block()`: chunk → `Element`s (handles `echo`, `eval`, `include`, `results`, `warning`, `message`, figures)
- `inline.rs` — `evaluate_inline()`: `` `{r}` ``/`` `{python}` `` in text → evaluated strings
- `r.rs` — `RSession` (extendr), R-specific `capture()` (wraps code in R script with graphics device + sentinel protocol), `eval_inline()` (R formatting with digit/comma support)
- `python.rs` — `PythonSession` (pyo3), Python-specific `capture()` (wraps code in Python script with stdout/matplotlib/warning capture), `eval_inline()`. Shared globals dict persists variables across chunks.
- `util.rs` — `needs_engine()`: scans blocks and body to determine if R/Python runtime should be initialized

### Root (`calepin/src/`) — Orchestration and core types

- `main.rs` — Entry point, pipeline stages
- `batch.rs` — Batch rendering: `BatchJob`/`BatchResult` types, `run_batch()` parallel runner
- `types.rs` — Input types: `Block`, `CodeChunk`, `ChunkOptions`, `Metadata`, `FigureAttrs`
- `plugin_manifest.rs` — Plugin manifest (`plugin.yml`) parsing: `PluginManifest`, `FilterMatch`, `FilterSpec`, etc.
- `registry.rs` — Plugin registry: `PluginRegistry` loads user + built-in plugins, dispatches filters/shortcodes/postprocessors, resolves templates. `StructuralHandler` trait for built-in structural plugins.
- `cli.rs` — CLI argument parsing (clap) + `cwarn!` macro + `plugin init`/`plugin list` subcommands
- `tera_engine.rs` — Tera body processing: `process_body()` replaces shortcodes with Tera functions, code block protection, custom Tera function implementations (`PagebreakFn`, `EnvFn`, `VideoFn`, `BrandFn`, `KbdFn`), plugin shortcode bridge
- `util.rs` — `slugify()`, `escape_html()`, `resolve_path()`, `run_json_process()`

### `calepin/src/filters/` — Transforms

Each filter enriches template variables or produces final output. Built-in div filters implement the `DivFilter` trait with a uniform `(classes, id, content, format, vars) → FilterAction` interface.

- `callout.rs` — Callout enrichment: title, icon, collapse/appearance. Produces `<details>` for collapsible HTML callouts. Registered as built-in plugin.
- `theorem.rs` — Theorem auto-numbering: per-type counters, injects `{{number}}`. Registered as built-in plugin.
- `shortcodes.rs` — Pre-parse `{% include %}` expansion, shortcode marker resolution, `VARIABLES` cache
- `code.rs` — Code block template variable filling (syntax highlighting)
- `figure.rs` — Figure template vars, figure div rendering, image helpers, path resolution

### `calepin/src/render/` — Format conversion machinery

- `mod.rs` — `OutputRenderer` trait with `format()` and `extension()`
- `elements.rs` — `Element` enum + `ElementRenderer` dispatch
- `div.rs` — Div rendering pipeline: unified plugin registry dispatch (structural → filter → subprocess → template)
- `span.rs` — Span rendering pipeline: unified plugin registry dispatch → template → fallback
- `template.rs` — Tera-based template rendering (`apply_template()` using `Tera::one_off()`) + page template loading + `build_template_vars()`
- `markdown.rs` — comrak CommonMark+GFM with math/raw protection
- `latex.rs` — Markdown-to-LaTeX conversion
- `markers.rs` — Marker systems for protecting content through conversion

### `calepin/src/formats/` — Format-specific renderers

- `html.rs`, `latex.rs`, `typst.rs`, `markdown.rs` — Implement `OutputRenderer`
- `mod.rs` — `OutputRenderer` trait, `create_renderer()`, custom format loading, `run_script()` helper, preprocess/postprocess script support

### `calepin/src/structures/` — Structural div handlers

- `tabset.rs` — `.panel-tabset` → tabs (HTML) or plain sections
- `layout.rs` — `layout-ncol`/`layout-nrow`/`layout` → CSS Grid / minipage / #grid
- `figure.rs` — Figure div rendering

### `calepin/src/templates/` — Built-in templates (embedded at compile time)

- `elements/` — Element templates (`div.html`, `figure.latex`, `theorem.typst`, etc.)
- `pages/` — Page templates (`calepin.html`, `calepin.css`, `calepin.latex`, `calepin.typst`)
- `misc/` — Other resources (`default.csl`)

### `plugins/` — Plugin source (to be migrated to subprocess-based plugins)

Legacy WASM plugin sources. These need to be rewritten as executable scripts with `plugin.yml` manifests. See the plugin system documentation below.

## Plugin System

All extensibility flows through the `PluginRegistry` (`registry.rs`). A plugin is a directory with a `plugin.yml` manifest declaring its capabilities.

### Plugin types

- **Built-in structural** (`BuiltinStructural`) — Receive raw `&[Element]` children + render closure. Run before child rendering. Used by: tabset, layout, figure-div.
- **Built-in filter** (`BuiltinFilter`) — Implement the `Filter` trait. Run after child rendering. Used by: theorem, callout.
- **Subprocess** — External executables receiving JSON on stdin, returning output on stdout. One process per call.
- **Persistent subprocess** — Long-running process communicating via JSON lines. Spawned once, reused across calls.

### Dispatch order

`render/div.rs` iterates matching plugins in registry order (user plugins first, then built-in):

1. For each matching plugin (by classes/attrs/id_prefix/formats):
   - **Structural** → call with raw children, return if handled
   - **Filter/Subprocess** → lazy-render children, call, return if `Rendered`; accumulate vars if `Continue`
2. **Template lookup** — explicit override → class-based → `div` fallback

### plugin.yml manifest

```yaml
name: myplugin
version: 0.1.0
description: "What this plugin does"
provides:
  filter:
    run: filter.py
    match:
      classes: [myclass]     # CSS classes (OR'd)
      attrs: [my-attr]       # Attribute names (OR'd)
      id_prefix: "fig-"      # ID prefix
      formats: [html]        # Output formats (omit = all)
    contexts: [div, span]    # Default: both
    persistent: false         # JSON-lines protocol
  shortcode:
    run: shortcode.py
    names: [mysc]
  postprocess:
    run: postprocess.py
    formats: [html]
  elements:
    dir: elements/
  templates:
    dir: templates/
  csl: style.csl
  format:
    name: myformat
    base: html
    extension: html
```

### Resolution

1. `_calepin/plugins/{name}/plugin.yml` (project)
2. `~/.config/calepin/plugins/{name}/plugin.yml` (user)

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

Templates use Tera syntax (`{{variable}}`, `{% if %}`, `{% for %}`, filters, macros). Template variable names use underscores (e.g., `id_attr`, `plain_title`). CSS class names in source documents keep dashes; the template resolver normalizes dashes to underscores when looking up templates by class name.

Resolution order: plugin-provided dirs (in plugin order) → `_calepin/{elements,templates}/` → `~/.config/calepin/` → built-in.

- Element templates: plugin `elements/` dir → `_calepin/elements/{name}.{format}` → built-in
- Page templates: plugin `templates/` dir → `_calepin/templates/calepin.{format}` → built-in
- CSL: plugin `csl` field → `_calepin/templates/calepin.csl` → built-in
- Filters: provided via plugin `filter` capability (subprocess executables with `plugin.yml`)
- Shortcodes: provided via plugin `shortcode` capability (registered as Tera functions)
- Custom formats: provided via plugin `format` capability, or `_calepin/formats/{name}.yaml`

## Chunk Options

Only pipe syntax (`#| key: value`) is accepted. The header accepts only language and optional label: `{r}` or `{r, label}` (also `{python}` or `{python, label}`). Key=value in headers produces an informative error with the corrected pipe syntax. Option names use dashes (`fig-width`), normalized to dots internally. `label` is rejected in pipe comments — it must be in the header.

## Tera Body Processing

The `.qmd` body text is processed as a Tera template during the evaluate stage (`tera_engine.rs`). Code blocks and inline code are protected from Tera evaluation.

Built-in Tera functions (replace old `{{< shortcode >}}` syntax):

- `{{ pagebreak() }}` — format-specific page break
- `{{ env(name="VAR") }}` — environment variable
- `{{ video(url="...", width="...", height="...", title="...") }}` — video embed
- `{{ brand(type="color", name="primary") }}` — brand assets
- `{{ kbd(keys=["Ctrl", "C"]) }}` — keyboard shortcuts

Context variables:
- `{{ meta.title }}`, `{{ meta.author }}`, `{{ meta.date }}`, etc. — document metadata
- `{{ var.key.subkey }}` — values from `_variables.yml`
- `{{ format }}` — current output format

File inclusion uses Tera's include tag: `{% include "file.qmd" %}` (pre-parse directive, runs before block parsing via `filters/shortcodes.rs::expand_includes()`). Escaping uses `{% raw %}...{% endraw %}`.

Plugin shortcodes are registered as Tera functions via the plugin registry.

## Dependencies

- `comrak` — CommonMark + GFM markdown parsing/rendering
- `hayagriva` — Citation/bibliography processing
- `syntect` — Syntax highlighting
- `tera` — Template engine for element/page templates and body processing (replaces custom regex substitution)
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
- **`process_*`** — Multi-step transformation of data (`process_citations`, `process_shortcodes`, `process_results`)

When a function is format-specific, append the format as a qualifier: `number_sections_html`, `postprocess_html`, `escape_latex_line`, `markdown_to_latex`.
