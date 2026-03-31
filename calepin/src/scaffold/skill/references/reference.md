# Calepin Reference

## Minimal Document

A `.qmd` file with TOML front matter between `---` delimiters:

````markdown
---
title = "My Document"
date = "today"
bibliography = "references.bib"

[toc]
enabled = true
---

## Introduction

```{r}
summary(mtcars[, 1:4])
```

See @knuth1984 for details.
````

Render with:

```
calepin render notebook.qmd
```

Output formats: HTML (default), LaTeX (`.tex`), Typst (`.typ`), Markdown (`.md`), PDF.


## CLI Commands

### render

```
calepin render document.qmd                     # HTML (default)
calepin render document.qmd -t latex             # .tex
calepin render document.qmd -t typst             # .typ
calepin render document.qmd -t markdown          # .md
calepin render document.qmd -t pdf               # PDF via Typst
calepin render document.qmd -t html,latex,typst  # multiple formats
calepin render document.qmd -o paper.tex         # explicit output path
calepin render document.qmd -s title="Draft" number-sections=true
calepin render document.qmd -q                   # quiet mode
```

### preview

```
calepin preview document.qmd
calepin preview document.qmd --port 8080
calepin preview my_website/
```

### init

```
calepin init paper.qmd                  # document with sidecar (html target)
calepin init paper.qmd -t latex         # document with sidecar (latex target)
calepin init my-site -t website         # website project
calepin init my-book -t book            # book project
```

### extra

```
calepin extra csl                  # list CSL citation styles
calepin extra themes               # list syntax highlighting themes
calepin extra highlight            # list syntax highlighting themes
calepin templates list html         # show template resolution chain
calepin extra completions bash     # shell completions (bash/zsh/fish)
```

### man

```
calepin man r ggplot2              # R package docs as .qmd
calepin man python numpy           # Python package docs as .qmd
```

### flush

```
calepin flush                      # delete caches and artefacts
calepin flush -y                   # skip confirmation
calepin flush --cache              # cache only
calepin flush --files              # generated files only
calepin flush --compilation        # LaTeX artefacts (.aux, .log, etc.)
calepin flush path/to/dir          # specific directory
```


## Code Chunks

### Syntax

````
```{r, my-label}
#| echo = false
#| fig-width = 7
x <- 1:10
mean(x)
```
````

Four parts:

1. Language: `{r}`, `{python}`, or `{sh}`
2. Label (optional): after the language, comma-separated. Used for cross-referencing and error messages.
3. Chunk options: lines starting with `#|` in `key = value` (TOML) or `key: value` (Quarto-compatible) format. Names use dashes (e.g., `fig-width`).
4. Code body: executed by the language engine.

Each language runs in a persistent subprocess. Variables persist across chunks within the same language but not across languages.

### Chunk Option Reference

| Option | Default | Values | Description |
|--------|---------|--------|-------------|
| `eval` | `true` | `true`, `false` | Execute the code |
| `echo` | `true` | `true`, `false`, `fenced` | Display source code. `fenced` shows fence markers and options. |
| `include` | `true` | `true`, `false` | Include any output. `false` runs silently. |
| `results` | `markup` | `markup`, `asis`, `hide` | Output handling. `asis` injects raw output. |
| `warning` | `true` | `true`, `false` | Display warnings |
| `message` | `true` | `true`, `false` | Display messages |
| `fig-width` | `6` | number (inches) | Graphics device width. Auto-scaled from `out-width` unless set explicitly. |
| `fig-height` | derived | number (inches) | Derived from `fig-width * fig-asp` |
| `fig-asp` | `0.618` | number | Aspect ratio (height / width). Golden ratio default. |
| `fig-cap` | | string | Figure caption |
| `fig-alt` | | string | Figure alt text for accessibility |
| `fig-align` | `center` | `left`, `center`, `right` | Figure alignment |
| `out-width` | `70%` | percentage string | Display width in the document |
| `dev` | `png` | `png`, `pdf`, `svg` | Graphics device |
| `label` | | string | Chunk label (must be in the header, not pipe comments) |
| `filename` | | string | Display a filename header above the code block |
| `jinja` | `false` | `true`, `false` | Enable Jinja processing inside the chunk |
| `tbl-cap` | | string | Table caption (for R table output) |

### R

Persistent `Rscript` subprocess. Plots captured automatically. Numeric inline results formatted with 3 significant digits and comma separators.

Table packages: `tinytable`, `gt`, `knitr::kable` all work. Use `tbl-` label prefix and `tbl-cap` for cross-referenceable tables.

### Python

Persistent `python3` subprocess. Matplotlib figures captured automatically. Shared globals dictionary persists across chunks.

### Shell

Persistent `/bin/sh` subprocess. Variables, working directory, and environment persist across chunks.


## Inline Code

Syntax: `` `{r} expr` ``, `` `{python} expr` ``, `` `{sh} expr` ``

Variables defined in code chunks are available to inline expressions. Numeric R results get 3 significant digits and comma separators.


## Figures

Code chunks that produce plots generate figures automatically. Add `fig-cap` for a caption and a label in the chunk header for cross-referencing.

````
```{r, fig-scatter}
#| fig-cap = "A scatter plot"
#| out-width = "80%"
plot(1:10, (1:10)^2)
```
````

Reference with `@fig-scatter`.

### Figure Options

| Option | Default | Description |
|--------|---------|-------------|
| `out-width` | `70%` | Display width in the document |
| `fig-asp` | `0.618` | Aspect ratio (height / width) |
| `fig-width` | `6` | Graphics device width in inches (auto-scaled from `out-width`) |
| `fig-height` | | Derived from `fig-width * fig-asp` |
| `fig-cap` | | Caption text |
| `fig-alt` | | Alt text |
| `fig-align` | `center` | Alignment: `left`, `center`, `right` |
| `dev` | `png` | Graphics device: `png`, `pdf`, `svg` |

Auto-scaling: Calepin scales `fig-width` to match `out-width`, so text inside figures stays consistent regardless of display size. Setting `fig-width` explicitly disables auto-scaling.

### Figure Layouts

Arrange figures in grids using `layout-ncol`:

````
::: {#fig-panels layout-ncol=3 fig-cap="Three plots compared."}
```{r, line}
#| fig-cap = "Linear"
plot(1:10, 1:10, type = "l")
```

```{r, quad}
#| fig-cap = "Quadratic"
plot(1:10, (1:10)^2, type = "l")
```
:::
````

The `layout` attribute supports custom grids: `layout="[[1,1],[1]]"` with negative values for spacing gaps.


## Tables

### Pipe Tables (GFM)

```
| Name  | Score | Grade |
|-------|------:|:-----:|
| Alice |    95 | A     |
| Bob   |    87 | B     |
```

Column alignment: `:---` left (default), `---:` right, `:---:` center.

### Captions and Cross-references

Wrap in a fenced div with `#tbl-` id. The last paragraph in the div becomes the caption:

```
::: {#tbl-grades}
| Name  | Score | Grade |
|-------|------:|:-----:|
| Alice |    95 | A     |

Student grades for Fall 2026.
:::
```

Reference with `@tbl-grades`.

### R Tables

Use `tinytable`, `gt`, or `knitr::kable` with `tbl-` chunk labels and `tbl-cap` option:

````
```{r, tbl-cars}
#| tbl-cap = Motor Trend car data
library(tinytable)
tt(mtcars[1:5, 1:4])
```
````


## Math

### Inline and Display

- Inline: `$a^2 + b^2 = c^2$`
- Display: `$$...$$` on their own lines
- Literal dollar sign: `\$`
- Multi-line: use `\begin{aligned}...\end{aligned}` inside `$$`

Equation labels for cross-referencing:

```
$$
E = mc^2
$$ {#eq-einstein}
```

Reference with `@eq-einstein`.

HTML uses KaTeX (optionally MathJax). LaTeX and Typst render math natively.

### Theorem Environments

Fenced divs with theorem-type classes get auto-numbering:

```
::: {.theorem #thm-pythagoras}
In a right triangle, $a^2 + b^2 = c^2$.
:::

::: {.proof}
By constructing squares on each side...
:::
```

Reference with `@thm-pythagoras`.

Available types: `theorem`, `lemma`, `corollary`, `proposition`, `conjecture`, `definition`, `example`, `exercise`, `solution`, `remark`, `algorithm`.

### Theorem Cross-reference Prefixes

| Class | Prefix | Example |
|-------|--------|---------|
| `theorem` | `thm` | `@thm-pythagoras` |
| `lemma` | `lem` | `@lem-helper` |
| `definition` | `def` | `@def-group` |
| `example` | `exm` | `@exm-basic` |


## Citations and Bibliography

### Front Matter

```toml
bibliography = "references.bib"
csl = "apa"
```

Multiple files: `bibliography = ["refs_theory.bib", "refs_empirical.bib"]`

### Citation Syntax

| Syntax | Output | Description |
|--------|--------|-------------|
| `@key` | Author et al. (Year) | Narrative citation |
| `[@key]` | (Author et al. Year) | Parenthetical citation |
| `[-@key]` | Year | Suppress author |
| `[@key1; @key2]` | (Author1 Year1; Author2 Year2) | Grouped citations |

### CSL Styles

Default: `chicago-author-date`. Set `csl` in front matter or `{stem}_calepin/config.toml`.

Resolution: file path > built-in name > default. Run `calepin extra csl` to list all built-in styles.


## Cross-references

| Target | Prefix | Syntax |
|--------|--------|--------|
| Figures | `fig-` | `@fig-scatter` |
| Tables | `tbl-` | `@tbl-grades` |
| Equations | `eq-` | `@eq-einstein` |
| Theorems | `thm-` | `@thm-pythagoras` |
| Lemmas | `lem-` | `@lem-helper` |
| Definitions | `def-` | `@def-group` |
| Examples | `exm-` | `@exm-basic` |
| Sections | `sec-` | `@sec-figures` |

Suppress type prefix with `[-@fig-scatter]`.


## Configuration

### Merge Order (last wins)

1. Built-in defaults
2. Project config: `{stem}_calepin/config.toml`
3. Sidecar config: `{stem}_calepin/config.toml` (per-document)
4. Document front matter (TOML between `---`)
5. CLI overrides: `-s key=value`

### Shared Defaults

```toml
csl = "chicago-author-date"
math = "katex"
lang = "en"
dpi = 150.0

[highlight]
light = "github"
dark = "nord"

[figure]
fig_width = 6.0
fig_asp = 0.618
out_width = 0.70
device = "png"
alignment = "center"

[execute]
cache = true
eval = true
echo = true
include = true
warning = true
message = true
results = "markup"

[toc]
enabled = false
depth = 3
title = "Contents"
```

### Document Targets

| Target | Writer | Extension |
|--------|--------|-----------|
| `html` | html | `.html` |
| `latex` | latex | `.tex` |
| `typst` | typst | `.typ` |
| `pdf` | typst | `.pdf` |
| `markdown` | markdown | `.md` |

### Syntax Highlighting

Set theme: `highlight-style = "solarized-dark"` in front matter. List themes: `calepin extra themes`.


## Project Structure

### Key Directories

| Path | Purpose |
|------|---------|
| `{stem}_calepin/config.toml` | Project configuration |
| `{stem}_calepin/templates/` | Template overrides |
| `{stem}_calepin/assets/` | Static assets (CSS, images) copied to output |
| `{stem}_calepin/extensions/` | Installed extensions |
| `{stem}_calepin/` | Per-document sidecar (config, cache, files) |
| `_calepin/output/` | Default website output directory |

### Sidecar Directory

Created with `calepin init sidecar paper.qmd`. Structure:

```
paper_calepin/
  config.toml
  templates/
  cache/
  files/
```


## Templates

### Jinja2 Syntax

Templates use Jinja2 (MiniJinja):

```
{{ variable }}
{{ title | upper }}
{% if caption %}<figcaption>{{ caption }}</figcaption>{% endif %}
{% for item in keywords %}\keyword{ {{- item -}} }{% endfor %}
```

### Template Organization

```
{stem}_calepin/templates/
  html/         # HTML element and page templates
  latex/        # LaTeX templates
  typst/        # Typst templates
  website/      # Website layout templates
  common/       # Format-agnostic (.jinja)
```

### Template Resolution Order (first match wins)

1. Document sidecar: `{stem}_calepin/templates/{target|writer|common}/`
2. Project overrides: `{stem}_calepin/templates/{target|writer|common}/`
3. Active extension's templates
4. Parent extension's templates (walking inheritance chain)
5. Built-in templates (embedded in binary)

Override only the files you need to change; everything else falls through to the built-in defaults.

### Key Element Templates

| Template | Used by |
|----------|---------|
| `figure` | Figures, images, and figure divs |
| `table` | Table div wrappers |
| `code_source` | Code chunks (source) |
| `code_output` | Code chunks (output) |
| `callout` | All callout types |
| `theorem_italic` | theorem, lemma, corollary, proposition, conjecture |
| `theorem_normal` | definition, example, exercise, solution, remark, algorithm |
| `page` | Full page wrapper |

### Key Template Variables

**Div**: `children`, `classes`, `id`, `label`, `format`, plus user `key="value"` attributes.

**Figure**: `src`, `image`, `alt`, `caption`, `label`, `number`, `width_attr`, `height_attr`, `align`, `cap_location`, `link`.

**Code**: `code`, `lang`, `label`, `highlighted`, `filename`.

**Page**: `body`, `toc`, `preamble`, `title_plain`, `title`, `authors`, `date`, `lang`, `css`, `js`, `math`.


## Jinja Body Processing

The `.qmd` body is processed as a Jinja template during evaluation. Code blocks and inline code are protected from evaluation.

### Available Variables

| Variable | Description |
|----------|-------------|
| `meta.title`, `meta.author`, `meta.date` | Front matter fields |
| `var.key`, `var.key.subkey` | Custom front matter fields (nested) |
| `env.HOME`, `env.USER`, ... | System environment variables |
| `writer` | Output format: `html`, `latex`, `typst`, `markdown` |
| `target` | Target name |

### Custom Variables

```toml
[var]
sample_size = 10

[var.urls]
docs = "https://example.com"
```

Access: `{{ var.sample_size }}`, `{{ var.urls.docs }}`.

### Conditional Content

```
{% if writer == "html" %}
HTML-only content.
{% endif %}
```

Also via fenced divs:

```
::: {.content-visible when-format="html"}
This only appears in HTML.
:::
```

### Includes

Include `.qmd` files (path relative to project root):

```
{% include "common/disclaimer.qmd" %}
```

Include templates (no extension, resolved from `{stem}_calepin/templates/`):

```
{% include "source_tip" %}
```


## Built-in Spans

- `[]{.pagebreak}` -- format-specific page break
- `[]{.video url="https://..." width="640" height="480"}` -- video embed
- `[]{.lorem paragraphs=3}` -- placeholder text (also `sentences`, `words`)
- `[]{.placeholder width=300 height=200 text="Banner" color="#e0e0ff"}` -- placeholder image


## Callouts

Five types: `callout-note`, `callout-warning`, `callout-tip`, `callout-caution`, `callout-important`.

```
::: {.callout-note}
Supplementary information.
:::
```


## Tabsets

```
::: {.panel-tabset}

## Tab 1
Content for tab 1.

## Tab 2
Content for tab 2.

:::
```


## Raw Blocks

Format-specific markup, passed through when format matches:

````
```{=html}
<details><summary>Click</summary><p>HTML only.</p></details>
```

```{=latex}
\newpage
```
````


## Websites

### Manifest (`{stem}_calepin/config.toml`)

```toml
title = "My Site"
target = "website"
output = "output"

[[contents]]
pages = ["index.qmd", "about.qmd"]

[[contents]]
title = "Guide"
pages = ["guide/*.qmd"]

[[navbar.left]]
text = "My Site"
href = "/index.html"

[[navbar.right]]
icon = "github"
href = "https://github.com/user/repo"

[[post]]
command = "pagefind --site {output}"
```

### Contents

`[[contents]]` defines sidebar navigation and page ordering:

```toml
[[contents]]
include = ["pages/installation.qmd", "pages/getting_started.qmd"]

[[contents]]
text = "Guide"
include = "guide"
```

Standalone pages (rendered but excluded from sidebar/prev-next):

```toml
[[contents]]
standalone = true
include = ["index.qmd", "404.qmd"]
```


## Extensions

### Structure

```
my-extension/
  extension.toml      # manifest (required)
  templates/           # template overrides
  assets/              # CSS, JS, images
  scripts/             # external module executables
```

### Manifest

```toml
name = "tufte"
description = "Tufte-style HTML"
inherits = "html"

[target]
writer = "html"
extension = "html"
modules = ["highlight", "append_footnotes"]

[assets]
css = ["tufte.css"]
```

### Activation

Target-based: `target = "tufte"` in config or front matter.

Side-loading (modules/assets only):

```toml
[calepin]
extensions = ["lightbox"]
```

Installation: copy to `{stem}_calepin/extensions/{name}/`.

Scaffolding: `calepin init extension myext --inherits html`.
