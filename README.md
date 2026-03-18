# [EXPERIMENTAL] calepin [EXPERIMENTAL]

A fast, minimal `.qmd` renderer with an embedded R runtime. calepin converts Quarto-style documents to HTML, LaTeX, Typst, and Markdown.

## Why

**Speed.** calepin is a single compiled binary with an embedded R session. There is no startup overhead from spawning external processes or loading interpreters. Rendering a typical document takes tens of milliseconds, not seconds.

**Simplicity.** One binary, one command, easy to install. calepin implements a focused subset of Quarto's functionality: code chunks, inline code, figures, cross-references, citations, theorem environments, callouts, conditional content, and raw blocks. No YAML-driven project configuration, no multi-engine orchestration, no plugin ecosystem to navigate.

**Extensibility.** Custom templates use simple `{{variable}}` substitution. Custom filters are executables that receive JSON on stdin. WASM plugins extend calepin with new span/div filters, shortcodes, and output formats. Write plugins in Rust, Go, JavaScript, or any language with an [extism PDK](https://extism.org).

## Install

### From source

Requires [Rust](https://rustup.rs) and [R](https://cran.r-project.org) (both at compile time and runtime).

```
git clone https://github.com/vincentarelbundock/calepin
cd calepin
make install
```

### From crates.io

```
cargo install calepin
```

**Note:** R must be installed and discoverable on your system. The build links against R's shared library via [extendr](https://extendr.github.io/). If compilation fails, ensure `R` is on your `PATH` and that `R CMD config --ldflags` works.

## Usage

```
calepin document.qmd
```

The output format and filename are inferred from the YAML front matter. Override with `--format` and `--output`:

```
calepin document.qmd --format latex -o paper.tex
```

## Features

- R code chunks with `eval`, `echo`, `include`, `results`, `warning`, `message` options
- Inline R expressions: `` `{r} mean(1:10)` ``
- Figures with captions, labels, sizing, alignment, and cross-referencing
- Layout grids (`layout-ncol`, `layout-nrow`, custom layouts)
- Cross-references: `@fig-label`, `@sec-label`, `@thm-label`, etc.
- Citations via BibTeX/BibLaTeX (processed by hayagriva)
- Theorem environments with automatic numbering
- Callouts (note, warning, tip, caution, important) with collapse support
- Conditional content with `when-format`, `unless-format`, `when-meta`, `unless-meta`
- Tabsets (`.panel-tabset`)
- Shortcodes: `{{< pagebreak >}}`, `{{< meta title >}}`, `{{< include file.qmd >}}`, `{{< var key >}}`
- Definition lists, footnotes, tables, task lists, superscript
- Math: `$inline$` and `$$display$$` (MathJax in HTML, native in LaTeX/Typst)
- Syntax highlighting (syntect)
- WASM plugins for custom filters, shortcodes, and output formats

## Plugins

Plugins are `.wasm` files in `_calepin/plugins/`. Declare them in front matter:

```yaml
calepin:
  plugins:
    - txtfmt
```

Example plugins in the `plugins/` directory:

- **txtfmt**: inline text styling (color, size, smallcaps, underline, mark)
- **imgur**: upload images to imgur
- **revealjs**: HTML → reveal.js slide deck
- **slidev**: Markdown → Slidev slide deck

Build plugins with `make plugins` (requires `rustup target add wasm32-unknown-unknown`).

## Custom Formats

Define new output formats via `_calepin/formats/{name}.yaml`:

```yaml
base: html
extension: html
plugin: revealjs
```

Then use `format: revealjs` in front matter or `--format revealjs` on the CLI.

## Customization

Place files in `_calepin/` (project-level) or `~/.config/calepin/` (user-level):

- `elements/` — templates for divs, spans, code blocks, figures
- `filters/` — executable filters for fenced divs and spans
- `templates/` — page wrappers (`calepin.html`, `calepin.latex`, etc.)
- `plugins/` — WASM plugins
- `formats/` — custom format definitions

See `docs/extensions.qmd` for full documentation with live examples.

## Acknowledgments

calepin builds on ideas, code, and documentation from three projects:

- **[litedown](https://github.com/yihui/litedown)** by Yihui Xie — the internal architecture and execution model (fuse/render pipeline, chunk option semantics, output capture strategy) are directly inspired by litedown. MIT license.
- **[Quarto](https://quarto.org/)** — feature design, documentation structure, and syntax conventions (conditional content, raw blocks, cross-reference prefixes, callout types) follow Quarto's lead. MIT license.
- **[comrak](https://github.com/kivikakk/comrak)** — CommonMark and GFM parsing, including the markdown-to-LaTeX rendering path. BSD 2-Clause license.

## License

MIT
