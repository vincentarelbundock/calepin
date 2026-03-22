# Calepin: Project and Format Specification

This document specifies the *Calepin* project structure and TOML-based output configuration.

### Terminology

Three distinct concepts:

- **Base** -- the rendering engine family: `html`, `latex`, `typst`, or `markdown`. Determines the AST walker, element rendering, and output syntax.
- **Target** -- a named project output profile defined in `calepin.toml`. Each target selects a base and may customize templates, components, compilation, and variables.
- **Compile** -- an optional second stage that transforms the rendered output into a final artifact (e.g., `.tex` to `.pdf`).

## 1. Project Root

A *Calepin* project is any directory containing a file named `calepin.toml`. Recommended structure:

```
my-project/
├── calepin.toml       # Project configuration (required)
├── content/           # Source documents (.qmd)
├── templates/         # Document layouts (full-page wrappers)
├── components/        # Reusable rendering units (figure, callout, code, ...)
├── assets/            # Shared assets (images, bib, fonts, CSL styles)
├── static/            # Files copied verbatim to output (CSS, JS, favicon)
├── .calepin/          # Cache and build metadata
└── output/            # Generated artifacts
```

Only `calepin.toml` is required at the root. For simple projects with a single `.qmd` file, the source document may live in the root directory alongside `calepin.toml` instead of in `content/`.

### Mirroring principle

The project directory structure mirrors the built-in template and component tree that *Calepin* ships embedded at compile time. Every built-in template lives at a path like `templates/common/macros.jinja` or `components/common/figure.jinja` inside the binary. The user's project directories (`templates/`, `components/`) use the exact same layout. To override any built-in, place a file at the same relative path in the project. The user's file always wins.

This means there is a single mental model: the project tree and the built-in tree are the same tree, with the project layer on top.

## 2. Standard Directories

All paths are defaults and may be overridden in `calepin.toml`.

```
my-project/
├── calepin.toml                  # Project configuration (required)
├── content/                      # Source documents (.qmd)
│   ├── index.qmd
│   ├── about.qmd
│   ├── images/                   # Images live alongside documents
│   │   └── diagram.png
│   └── book/
│       ├── chapter1.qmd
│       ├── chapter2.qmd
│       └── fig/
│           └── plot.svg
├── templates/                    # Document layouts (full-page wrappers)
│   ├── common/
│   │   └── macros.jinja
│   ├── html/
│   │   ├── base.html
│   │   ├── page.html
│   │   └── home.html
│   ├── latex/
│   │   ├── article.tex
│   │   └── book.tex
│   └── typst/
│       └── article.typ
├── components/                   # Reusable rendering units (figure, callout, code, ...)
│   ├── common/
│   │   ├── figure.jinja
│   │   ├── callout.jinja
│   │   ├── code_source.jinja
│   │   ├── code_output.jinja
│   │   ├── theorem_normal.jinja
│   │   ├── author_block.jinja
│   │   ├── title_block.jinja
│   │   ├── abstract_block.jinja
│   │   ├── div.jinja
│   │   └── ...
│   ├── html/
│   │   └── figure.html
│   └── latex/
│       └── figure.tex
├── assets/                       # Shared project assets
│   ├── images/                   # Shared logos, diagrams, reusable media
│   ├── bib/
│   ├── fonts/
│   └── csl/
├── static/                       # Files copied verbatim to output (CSS, JS, favicon, ...)
│   └── html/
│       ├── css/
│       ├── js/
│       └── favicon.ico
├── .calepin/                     # Cache and build metadata (not committed)
│   └── cache/
└── output/                       # Generated artifacts, partitioned by target
    ├── web/                      # output/<target>/mirrors content/ structure
    │   ├── basics.html
    │   └── book/
    │       └── chapter1.html
    ├── article/
    │   ├── basics.tex
    │   └── basics.pdf
    └── print/
        ├── basics.typ
        └── basics.pdf
```

| Directory | Purpose |
|-----------|---------|
| `content/` | Source `.qmd` documents and their local images. Relative paths resolve from the document directory. Paths starting with `/` resolve from the project root (e.g., `/assets/images/logo.svg`). |
| `templates/` | Document layouts that wrap the full rendered body. Per-base subdirectories (`html/`, `latex/`, `typst/`), plus `common/` for shared `.jinja` macros. |
| `components/` | Reusable rendering units (one per element type). Per-base subdirectories, plus `common/` for format-agnostic `.jinja` components. |
| `assets/` | Shared project assets: bibliography, fonts, CSL styles, and reusable media (logos, diagrams). Referenced via `/assets/...` paths from any `.qmd`. |
| `static/` | Files copied verbatim to output, especially for HTML (CSS, JS, favicon). |
| `.calepin/` | Cache and internal build metadata. Not committed to version control. |
| `output/` | Generated artifacts, partitioned by target name. Never written into source directories. See section 4 for output path rules. |

## 3. Target Configuration

Each key under `[targets]` in `calepin.toml` defines a named output profile. Every target selects a base rendering engine and may customize templates, components, compilation, and variables.

### Minimal example

```toml
# calepin.toml

[targets.web]
base = "html"
```

Each key becomes a target name usable via `calepin doc.qmd -f web` or `target: web` in front matter.

### Full schema

```toml
# calepin.toml

# ---------------------------------------------------------------
# Each key under [targets] defines a named output profile.
# ---------------------------------------------------------------

[targets.web]

# Required: base rendering engine.
# One of "html", "latex", "typst", "markdown".
base = "html"

# Document template to use (looked up in the template tree).
# Defaults to "calepin" (i.e., templates/{base}/calepin.{ext}
# or templates/common/calepin.jinja).
template = "page"

# Output file extension (without leading dot).
# Defaults to the base's extension (html, tex, typ, md).
extension = "html"

# Default figure/image extension produced by code chunks.
# Defaults to the base's default (png for html, pdf for
# latex/typst, png for markdown).
fig-extension = "svg"

# Compilation step (optional second stage).
# Only runs when the user passes --compile.
[targets.web.compile]
command = "typst compile {input} {output}"
extension = "pdf"
auto = false     # If true, --compile triggers this automatically.

# Arbitrary key-value pairs passed to templates and components
# as `target_vars.*`.
[targets.web.vars]
toc = true
code-copy = true
code-overflow = "scroll"
```

## 4. Multi-Target Example

A project that renders the *Calepin* website to HTML, and also produces PDF articles via LaTeX and Typst from the same `.qmd` sources:

```toml
# calepin.toml

# ---------------------------------------------------------------
# Website HTML output
# ---------------------------------------------------------------

[targets.web]
base = "html"

[targets.web.vars]
toc = true
highlight-style = { light = "github", dark = "nord" }
code-copy = true
code-overflow = "scroll"

# ---------------------------------------------------------------
# LaTeX article (renders .tex, compiles to PDF via tectonic)
# ---------------------------------------------------------------

[targets.article]
base = "latex"
template = "article"

[targets.article.compile]
command = "tectonic {input}"
extension = "pdf"
auto = true

[targets.article.vars]
documentclass = "article"
fontsize = "11pt"
geometry = "margin=1in"
toc = false

# ---------------------------------------------------------------
# Typst article (renders .typ, compiles to PDF)
# ---------------------------------------------------------------

[targets.print]
base = "typst"

[targets.print.compile]
command = "typst compile {input} {output}"
extension = "pdf"
auto = true

[targets.print.vars]
font = "Libertinus Serif"
fontsize = "11pt"
margin = { top = "1in", bottom = "1in", left = "1in", right = "1in" }
toc = false

# ---------------------------------------------------------------
# Plain markdown (for README, syndication, etc.)
# ---------------------------------------------------------------

[targets.readme]
base = "markdown"
```

Usage:

```bash
calepin content/basics.qmd -f web        # output/web/basics.html
calepin content/basics.qmd -f article    # output/article/basics.tex + basics.pdf
calepin content/basics.qmd -f print      # output/print/basics.typ + basics.pdf
calepin content/basics.qmd -f readme     # output/readme/basics.md
```

Or in front matter:

```yaml
---
title: Basics
target: article
---
```

### Output path rules

All output goes under `output/<target>/`, mirroring the source directory structure relative to `content/`. This guarantees that targets never collide, even when multiple targets compile to the same extension (e.g., both `article` and `print` produce `.pdf`).

| Rule | Example |
|------|---------|
| Rendered output | `content/book/chapter1.qmd` with `-f web` produces `output/web/book/chapter1.html` |
| Compiled artifact | Same directory as rendered output: `output/article/basics.pdf` alongside `output/article/basics.tex` |
| Single-file mode | `calepin doc.qmd -f web` with no `content/` directory produces `output/web/doc.html` |
| `-o` flag | Overrides the output path entirely: `calepin doc.qmd -f web -o build/doc.html` |
| Generated figures | `output/<target>/<stem>_files/` alongside the output file |

The target name is always part of the output path (unless `-o` overrides it). This makes multi-target builds safe by construction.

## 5. Field Reference

### Per-target fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `base` | string | yes | -- | Rendering engine. One of `html`, `latex`, `typst`, `markdown`. |
| `template` | string | no | `"calepin"` | Document template name. Looked up in the template tree (see section 7). |
| `extension` | string | no | base default | Output file extension (no dot). |
| `fig-extension` | string | no | base default | Default extension for generated figures. |

### `[targets.{name}.compile]` table

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `command` | string | no | -- | Shell command to compile output. `{input}` and `{output}` are replaced with file paths. |
| `extension` | string | no | -- | Extension of the compiled artifact. |
| `auto` | boolean | no | `false` | Whether `--compile` triggers this automatically. |

### `[targets.{name}.vars]` table

Arbitrary key-value pairs. Values may be strings, integers, floats, booleans, arrays, or inline tables. Available in templates and components as `target_vars.{key}`.

## 6. Precedence

All lookups follow the same three-layer model. The first match wins.

| Priority | Layer | Root path |
|----------|-------|-----------|
| 1 | Project | `.` (project root) |
| 2 | User | `~/.config/calepin/` |
| 3 | Built-in | embedded in binary |

Within each layer, base-specific files are checked before format-agnostic `.jinja`:

| Priority | What is looked up | Lookup kind |
|----------|-------------------|-------------|
| **Targets** | | |
| 1 | `[targets.{name}]` in project `calepin.toml` | TOML key |
| 2 | `[targets.{name}]` in user `calepin.toml` | TOML key |
| 3 | Implicit target if name matches a base or alias (`html`, `tex`, `pdf`, ...) | Fallback |
| **Templates** (e.g., `template = "article"`, `base = "latex"`) | | |
| 1 | `templates/latex/article.tex` (project) | base-specific |
| 2 | `templates/common/article.jinja` (project) | generic |
| 3 | `templates/latex/article.tex` (user) | base-specific |
| 4 | `templates/common/article.jinja` (user) | generic |
| 5 | `templates/latex/article.tex` (built-in) | base-specific |
| 6 | `templates/common/article.jinja` (built-in) | generic |
| **Components** (e.g., `figure`, `base = "html"`) | | |
| 1 | `components/html/figure.html` (project) | base-specific |
| 2 | `components/common/figure.jinja` (project) | generic |
| 3 | `components/html/figure.html` (user) | base-specific |
| 4 | `components/common/figure.jinja` (user) | generic |
| 5 | `components/html/figure.html` (built-in) | base-specific |
| 6 | `components/common/figure.jinja` (built-in) | generic |
| **Assets** (e.g., CSL style) | | |
| 1 | `assets/csl/style.csl` (project) | |
| 2 | `assets/csl/style.csl` (user) | |
| 3 | `assets/csl/default.csl` (built-in) | |
| **Paths in `.qmd` files** | | |
| 1 | Relative path resolves from document directory | `fig/plot.png` |
| 2 | `/`-prefixed path resolves from project root | `/assets/images/logo.svg` |

No other lookup paths exist.

## 7. Template and Component Details

### Templates

A target's `template` field (default: `"calepin"`) names the template file. The precedence matrix in section 6 defines the full lookup. To use a different template for a specific target:

```toml
[targets.article]
base = "latex"
template = "article"       # looks up templates/latex/article.tex

[targets.book]
base = "latex"
template = "book"          # looks up templates/latex/book.tex
```

### Components

Components are not selected per-target. All targets sharing the same base use the same components. To override a component for a specific base, place a file in `components/{base}/`.

### Shared partials

Templates and components can include shared partials via Jinja's `{% include %}`:

```html
{# templates/html/page.html #}
<!DOCTYPE html>
<html>
{% include "common/head.jinja" %}
<body>
  {% include "common/header.jinja" %}
  {{ body }}
  {% include "common/footer.jinja" %}
</body>
</html>
```

A partial can branch on the current base when needed:

```jinja
{# templates/common/header.jinja #}
{% if base == "html" %}
<header>{{ title }}</header>
{% elif base == "latex" %}
\maketitle
{% endif %}
```

## 8. Template Variables

All templates and components receive variables through a Jinja context. The available variables depend on the template type.

### Templates (document layouts)

Templates wrap the full rendered document. They receive:

| Variable | Type | Description |
|----------|------|-------------|
| `body` | string | Rendered main content |
| `base` | string | Base rendering engine (`html`, `latex`, `typst`, `markdown`) |
| `target` | string | Target name (e.g., `web`, `article`, `print`) |
| `title` | string | Rendered title (markdown converted to output format) |
| `plain_title` | string | Title without formatting (for `<title>`, PDF metadata) |
| `author` | string | Comma-separated author names |
| `date` | string | Document date |
| `toc` | string | Table of contents (base-specific) |
| `css` | string | Embedded CSS (HTML only) |
| `js` | string | JavaScript (HTML only) |
| `preamble` | string | Extra head content (HTML) or preamble (LaTeX) |
| `math_block` | string | Math rendering setup (KaTeX/MathJax include) |
| `html_math_method` | string | Math method name (`katex`, `mathjax`, `none`) |
| `generator` | string | Generator metadata string |
| `bib_preamble` | string | Bibliography preamble (LaTeX only) |
| `bib_end` | string | Bibliography postamble (LaTeX only) |
| `_lb`, `_rb` | string | Literal `{` and `}` (for LaTeX/Jinja escaping) |

**Rendered sub-blocks** (each produced by rendering the corresponding component):

| Variable | Description |
|----------|-------------|
| `title_block` | Title heading |
| `subtitle_block` | Subtitle |
| `author_block` | Authors with affiliations, ORCID, corresponding markers |
| `date_block` | Formatted date |
| `abstract_block` | Abstract |
| `keywords_block` | Keywords |
| `bibliography_block` | Bibliography |
| `appendix_block` | Appendix sections (license, copyright, funding, citation) |

**Brand variables** (when `brand:` is set in front matter):

| Variable | Description |
|----------|-------------|
| `brand-{name}` | Color value for semantic color name (e.g., `brand-primary`) |
| `brand-{name}-light`, `brand-{name}-dark` | Themed color variants |
| `brand_logo_light`, `brand_logo_dark` | Logo image paths |
| `brand_logo_alt` | Logo alt text |
| `brand_css` | Compiled brand CSS |

### Components

Each component type receives its own set of variables. All include `base` and `target`.

**Figure** (`figure.jinja`):

| Variable | Description |
|----------|-------------|
| `path` | Resolved image path |
| `image` | Formatted image tag |
| `alt` | Alt text |
| `caption` | Figure caption |
| `label` | Figure label (e.g., `fig-example`) |
| `number` | Auto-assigned figure number |
| `width_attr`, `height_attr` | Dimension attributes |
| `align` | Alignment (`left`, `center`, `right`) |
| `cap_location` | Caption position (`top`, `bottom`, `margin`) |
| `fig_env`, `fig_begin`, `fig_end`, `fig_pos` | LaTeX figure environment |
| `short_caption`, `caption_cmd` | LaTeX caption commands |

**Code source** (`code_source.jinja`):

| Variable | Description |
|----------|-------------|
| `code` | Source code (escaped) |
| `lang` | Language identifier |
| `label` | Chunk label |
| `highlighted` | Syntax-highlighted code |

**Code output** (`code_output.jinja`):

| Variable | Description |
|----------|-------------|
| `output` | Output text (escaped) |

**Code diagnostic** (`code_diagnostic.jinja`):

| Variable | Description |
|----------|-------------|
| `text` | Diagnostic text |
| `diagnostic_class` | `warning`, `message`, or `error` |

**Div** (`div.jinja`):

| Variable | Description |
|----------|-------------|
| `children` | Rendered child elements |
| `classes` | Space-separated CSS classes |
| `id` | Element ID |
| `id_attr` | Formatted ID attribute (e.g., ` id="..."`) |
| `label` | Format-specific label anchor |
| *(any attribute)* | HTML attributes from the div are passed through |

**Callout** (`callout.jinja`):

| Variable | Description |
|----------|-------------|
| `callout_type` | `note`, `tip`, `warning`, `caution`, `important` |
| `title` | Callout title |
| `icon` | Unicode emoji icon |
| `header` | Icon + title combined |
| `collapse` | Whether collapsible (`true`/`false`, HTML only) |
| `appearance` | Appearance style |

**Theorem** (`theorem_normal.jinja`, `theorem_italic.jinja`):

| Variable | Description |
|----------|-------------|
| `number` | Auto-incremented theorem number |
| `type_name` | Capitalized name (Theorem, Lemma, Definition, ...) |
| `type_class` | CSS class (theorem, lemma, definition, ...) |

**Author sub-components** (`author_item.jinja`, `affiliation_item.jinja`):

| Variable | Description |
|----------|-------------|
| `name` | Author name |
| `superscripts` | Affiliation indices |
| `corresponding` | Corresponding author marker |
| `orcid_link` | ORCID icon/link |
| `number` | Affiliation number (affiliation_item) |
| `display` | Full affiliation string (affiliation_item) |

### Body context variables

Inside the `.qmd` body (processed as Jinja before parsing), these variables are available:

| Variable | Description |
|----------|-------------|
| `base` | Base rendering engine |
| `target` | Target name |
| `meta.title` | Document title |
| `meta.subtitle` | Document subtitle |
| `meta.author` | Authors (comma-separated) |
| `meta.date` | Document date |
| `meta.abstract` | Document abstract |
| `meta.keywords` | Keywords (comma-separated) |
| `var.{key}` | Custom front matter fields (supports nesting: `var.key.subkey`) |
| `env.{NAME}` | System environment variables |

## 9. Front Matter

Document metadata is specified in YAML front matter (between `---` fences):

| Field | Type | Description |
|-------|------|-------------|
| `title` | string | Document title |
| `subtitle` | string | Document subtitle |
| `author` / `authors` | string or list | Author names or rich author objects |
| `date` | string | Publication date |
| `abstract` | string | Document abstract |
| `keywords` | list | Keyword strings |
| `target` | string | Output target name |
| `bibliography` | string or list | Bibliography file paths |
| `csl` | string | Citation style file |
| `number-sections` | boolean | Enable section numbering |
| `toc` | boolean | Enable table of contents |
| `toc-depth` | integer | TOC depth |
| `toc-title` | string | Custom TOC title |
| `date-format` | string | Date format specification |
| `html-math-method` | string | Math rendering method |
| `appendix-style` | string | Appendix rendering style |
| `copyright` | object | Copyright metadata |
| `license` | object | License information |
| `citation` | object | Citation metadata |
| `funding` | object | Funding information |
| `brand` | object | Brand configuration (colors, typography, logos) |
| `target-vars` | object | Per-document overrides for `[targets.{name}.vars]` |

Rich author objects support: `name` (or `name.given`/`name.family`), `email`, `url`, `orcid`, `note`, `corresponding`, `affiliations`, and `attributes` (`equal-contributor`, `deceased`).

Front matter `target-vars` merge with (and override) the TOML `[targets.{name}.vars]` defaults. The merge is shallow: top-level keys in front matter replace the corresponding TOML keys entirely.

## 10. Compilation Pipeline

When `[targets.{name}.compile]` is present and the user passes `--compile` (or `auto = true`):

1. *Calepin* renders the document using the base engine (e.g., LaTeX produces `.tex`).
2. The compile `command` is executed with `{input}` replaced by the rendered file path and `{output}` replaced by the artifact path (same stem, compile `extension`).
3. If the command exits non-zero, *Calepin* reports the error and exits.

## 11. Target Variables in Templates

The `[targets.{name}.vars]` table is passed to all templates and components under the `target_vars` namespace. In a Jinja template:

```latex
\documentclass[{{ target_vars.fontsize }}]{{ "{" }}{{ target_vars.documentclass }}{{ "}" }}
\usepackage[{{ target_vars.geometry }}]{geometry}
{% if target_vars.toc %}
\tableofcontents
{% endif %}
```

Or in an HTML template:

```html
{% if target_vars.toc %}
<nav class="toc">{{ toc }}</nav>
{% endif %}
{% if target_vars.code_copy %}
<script src="/static/html/js/copy-code.js"></script>
{% endif %}
```

Variables support TOML's full type system:

```toml
[targets.print.vars]
margin = { top = "1in", bottom = "1in", left = "1in", right = "1in" }
fonts = ["Libertinus Serif", "IBM Plex Mono"]
```

Accessed in templates as `target_vars.margin.top` and `target_vars.fonts` (iterable).

## 12. Validation

*Calepin* validates `calepin.toml` at load time using the `validator` crate. The TOML is deserialized into typed Rust structs with `serde`, then validated with `#[derive(Validate)]`:

```rust
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use validator::Validate;

/// Top-level: map of target name to target config.
type TargetsConfig = HashMap<String, Target>;

#[derive(Deserialize, Validate)]
#[serde(deny_unknown_fields)]
struct Target {
    /// Must be one of: html, latex, typst, markdown.
    #[validate(custom(function = "validate_base"))]
    base: String,

    /// Lowercase alphanumeric, no dot.
    #[validate(regex(path = "RE_EXTENSION"))]
    extension: Option<String>,

    #[validate(regex(path = "RE_EXTENSION"))]
    #[serde(rename = "fig-extension")]
    fig_extension: Option<String>,

    /// Document template name (default: "calepin").
    template: Option<String>,

    #[validate(nested)]
    compile: Option<CompileConfig>,

    /// Arbitrary; no validation beyond TOML type system.
    vars: Option<toml::Value>,
}

#[derive(Deserialize, Validate)]
#[serde(deny_unknown_fields)]
struct CompileConfig {
    /// Must contain {input} placeholder.
    #[validate(contains(pattern = "{input}"))]
    command: Option<String>,

    #[validate(regex(path = "RE_EXTENSION"))]
    extension: Option<String>,

    auto: Option<bool>,
}
```

Validation rules:

| Field | Constraint |
|-------|-----------|
| `base` | One of `html`, `latex`, `typst`, `markdown`. |
| `extension`, `fig-extension`, `compile.extension` | Lowercase alphanumeric (`^[a-z0-9]+$`). |
| `template` | Alphanumeric + hyphens/underscores. Resolved via the three-layer lookup. |
| `compile.command` | Contains `{input}` placeholder. |
| Unknown fields | Rejected by `#[serde(deny_unknown_fields)]`. |

On validation failure, *Calepin* exits with an error message naming the target, the field, and the constraint that failed.

---

## Implementation Instructions

The following is a self-contained prompt to pass to an LLM for implementing this spec in the *Calepin* Rust codebase.

```
You are implementing a new project structure and TOML-based target configuration
for Calepin, a Rust CLI that renders .qmd documents to HTML, LaTeX, Typst, and
Markdown. Read the spec in spec-toml-formats.md and the architecture in CLAUDE.md
before making any changes.

TERMINOLOGY

There are three distinct concepts. Use them precisely:

  base   = rendering engine family (html, latex, typst, markdown)
  target = named project output profile (web, article, print, readme, ...)
  compile = optional second-stage artifact (e.g., .tex -> .pdf)

The TOML table is [targets], not [formats]. The CLI flag is -f <target-name>.
Front matter uses `target:`, not `format:`. Template variables use `target_vars`,
not `format_vars`. Both `base` and `target` are available in template contexts.

CORE PRINCIPLE: MIRRORING

The built-in template and component tree embedded in the binary (currently at
calepin/src/templates/) must use the EXACT SAME directory layout as the user-facing
project directories. The binary's embedded tree IS the default project tree. The
user overrides any built-in file by placing a file at the same relative path in
their project.

Built-in tree (embedded at compile time):
  templates/common/       -- shared .jinja macros and partials
  templates/html/         -- HTML document layouts
  templates/latex/        -- LaTeX document layouts
  templates/typst/        -- Typst document layouts
  components/common/      -- format-agnostic .jinja components (figure, callout, ...)
  components/html/        -- HTML-specific component overrides
  components/latex/       -- LaTeX-specific component overrides
  components/typst/       -- Typst-specific component overrides
  assets/csl/default.csl  -- default citation style

User project tree (same layout, overrides built-ins):
  templates/              -- mirrors built-in templates/
  components/             -- mirrors built-in components/
  assets/                 -- shared project assets (bib, fonts, CSL, shared images)

Path resolution rules:
  - Relative paths (e.g., fig/plot.png) resolve from the document directory.
    Document-local images should live beside the .qmd file.
  - Paths starting with / (e.g., /assets/images/logo.svg) resolve from the
    project root. Shared reusable media (logos, diagrams) lives under assets/.
  - Do NOT change relative path resolution. ADD project-root resolution for
    /-prefixed paths.

NAMING CHANGES

The current codebase uses different names. Rename consistently:
  - "elements" (calepin/src/templates/elements/) -> "components"
  - "pages" (calepin/src/templates/pages/)       -> "templates"
  - "_calepin/elements/" user directory           -> "components/"
  - "_calepin/templates/" user directory          -> "templates/"
  - "_calepin/" prefix for user dirs              -> project root dirs
  - "format" in config/front matter context       -> "target"
  - "format_vars" in template context             -> "target_vars"
  - "format" template variable                    -> "base" (engine) + "target" (profile name)

Specifically:
  1. Move calepin/src/templates/elements/*.jinja -> calepin/src/templates/components/common/
  2. Move calepin/src/templates/pages/calepin.html -> calepin/src/templates/templates/html/calepin.html
  3. Move calepin/src/templates/pages/calepin.latex -> calepin/src/templates/templates/latex/calepin.latex
  4. Move calepin/src/templates/pages/calepin.typst -> calepin/src/templates/templates/typst/calepin.typst
  5. Move calepin/src/templates/pages/calepin.css -> calepin/src/templates/templates/html/calepin.css
  6. Move calepin/src/templates/misc/default.csl -> calepin/src/templates/assets/csl/default.csl

TARGET CONFIGURATION

Replace the current per-file format YAML/TOML (in _calepin/formats/{name}.toml)
with a [targets] table in calepin.toml:

  [targets.web]
  base = "html"

  [targets.article]
  base = "latex"
  [targets.article.compile]
  command = "tectonic {input}"
  extension = "pdf"
  auto = true

  [targets.print]
  base = "typst"
  [targets.print.compile]
  command = "typst compile {input} {output}"
  extension = "pdf"
  auto = true

Each target supports: base, template, extension, fig-extension,
compile.{command,extension,auto}, and vars.

RESOLUTION MODEL: PURE MIRRORING

There are exactly three layers, checked in order:
  1. Project tree (templates/, components/ in project root)
  2. User tree (~/.config/calepin/templates/, components/)
  3. Built-in tree (embedded in binary)

Each tree has the same layout:
  templates/{base}/        -- base-specific document layouts
  templates/common/        -- shared .jinja macros and partials
  components/{base}/       -- base-specific component overrides
  components/common/       -- format-agnostic .jinja components

NO per-target override directories. NO target-qualified filenames.
Per-target template selection happens through the explicit `template`
field on the target (default: "calepin").

For a target with base="latex" and template="article":
  1. templates/latex/article.tex (project)
  2. templates/common/article.jinja (project)
  3. templates/latex/article.tex (user config)
  4. templates/common/article.jinja (user config)
  5. templates/latex/article.tex (built-in)
  6. templates/common/article.jinja (built-in)

Components are NOT per-target. All targets with the same base share
components. Resolution for component "figure" with base="html":
  1. components/html/figure.html (project)
  2. components/common/figure.jinja (project)
  3-6. Same pattern for user config and built-in.

At each layer, base-specific extension is checked before .jinja.

VALIDATION

Add the `validator` crate. Deserialize calepin.toml into typed structs with
serde + #[derive(Validate)]. Validate base enum, extension regex,
script path existence, directory existence, and {input} in compile commands.
Use #[serde(deny_unknown_fields)] to catch typos.

FILES TO MODIFY

  - calepin/src/templates/ -- restructure the embedded tree
  - calepin/src/render/template.rs -- update template loading and resolution
  - calepin/src/render/elements.rs -- rename element -> component in lookups
  - calepin/src/registry.rs -- update resolution paths
  - calepin/src/formats/mod.rs -- load target config from [targets] table
  - calepin/src/main.rs -- look for calepin.toml, resolve project root
  - calepin/src/paths.rs -- update path resolution to use new directory names
  - calepin/Cargo.toml -- add validator dependency if not present

Do NOT change the rendering pipeline, AST walker, code execution engines, or
filter logic. This is a reorganization of the project structure and config
loading only.

Run `make test` after changes. Run `make build` to verify compilation.
```
