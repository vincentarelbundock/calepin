# Module system spec

## Overview

Two extension mechanisms: **partials** customize how elements look, **modules** customize what the pipeline does.

## Partials

A partial is a Jinja template that controls how an element renders. No manifest, no config. Just a file in the right place.

### Location

`_calepin/partials/{engine}/{name}.{ext}`

### Resolution

When rendering a div with class `.foo`, the renderer looks for a partial named `foo`:

1. `_calepin/partials/{target}/{name}.{ext}` (target-specific)
2. `_calepin/partials/{engine}/{name}.{ext}` (engine-specific)
3. `_calepin/partials/common/{name}.jinja` (format-agnostic)
4. Built-in partials

### Variables

`{{ children }}`, `{{ classes }}`, `{{ id }}`, `{{ base }}`, plus all div attributes as `{{ key }}`.

### Example

Override the default callout rendering:

```
_calepin/partials/html/callout.html
```

```html
<div class="my-callout {{ type }}">
  {% if title %}<strong>{{ title }}</strong>{% endif %}
  {{ children }}
</div>
```

No manifest. The filename matches the template name.

## Modules

A module is a directory with a `module.toml` manifest. Modules either provide element partials with explicit match rules, or run scripts at pipeline stages, or both.

### Location

`_calepin/modules/{name}/module.toml`

### Activation

Explicit opt-in via the target's `modules` list:

```toml
# _calepin.toml
[targets.html]
modules = ["highlight", "card", "minify"]
```

A module that isn't listed doesn't run. Order matters for body and document transforms.

### module.toml

```toml
name = "card"

# --- Element transform (optional) ---
# Provides a partial with explicit matching.
# The partial {name}.{engine} lives in this directory.

[element]
match.classes = ["card"]           # CSS classes (OR'd)
# match.attrs = ["data-card"]     # attribute names (OR'd)
# match.id_prefix = "card-"       # ID prefix
# match.formats = ["html"]        # restrict to specific formats (default: all)

# --- Body transform (optional) ---
# Receives rendered body on stdin, writes transformed body to stdout.
# Runs after element rendering, before cross-reference resolution.

[body]
run = "body.sh"

# --- Document transform (optional) ---
# Receives full assembled document on stdin, writes transformed document to stdout.
# Runs after page template wrapping.

[document]
run = "postprocess.sh"
```

All sections are optional. A module can provide any combination.

### Element transform

An element transform provides a partial with an explicit match rule. The partial file lives in the module directory, named `{module_name}.{ext}` (e.g., `card.html`, `card.tex`).

The partial receives the same variables as standalone partials: `{{ children }}`, `{{ classes }}`, `{{ id }}`, `{{ base }}`, plus div attributes.

#### Example

A card module:

```
_calepin/modules/card/
  module.toml
  card.html
  card.tex
```

```toml
name = "card"

[element]
match.classes = ["card"]
```

```html
<!-- card.html -->
<div class="card">
  {{ children }}
  {% if footer %}<footer>{{ footer }}</footer>{% endif %}
</div>
```

```latex
%% card.tex
\begin{tcolorbox}
{{ children }}
\end{tcolorbox}
```

In the document:

```markdown
::: {.card footer="Source: Wikipedia"}
Content here.
:::
```

#### How element transforms differ from standalone partials

| | Standalone partial | Module element transform |
|---|---|---|
| Location | `_calepin/partials/{engine}/` | `_calepin/modules/{name}/` |
| Matching | Implicit (filename = class name) | Explicit (`match.classes`, `match.attrs`, etc.) |
| Multi-engine | One file per engine directory | All engine files co-located |
| Bundled with scripts | No | Yes (can combine with body/document transforms) |
| Distribution | Copy individual files | Copy one directory |

A module element transform takes priority over a standalone partial with the same name.

### Body transform

A script that transforms the rendered body string. Runs after all elements are rendered, before cross-reference resolution.

#### Protocol

- stdin: rendered body (HTML, LaTeX, Typst, or Markdown depending on engine)
- stdout: transformed body
- Exit code 0: success. Non-zero: warning printed, original body preserved.

#### Environment variables

- `CALEPIN_FORMAT`: current engine name (`html`, `latex`, `typst`, `markdown`)
- `CALEPIN_ROOT`: project root directory
- `CALEPIN_INPUT`: path to the source `.qmd` file

#### Example

Auto-number figures:

```toml
name = "autonumber"

[body]
run = "number.py"
```

```python
#!/usr/bin/env python3
import sys, re

body = sys.stdin.read()
counter = [0]

def replace(m):
    counter[0] += 1
    return f'<figcaption>Figure {counter[0]}. {m.group(1)}</figcaption>'

body = re.sub(r'<figcaption>(.*?)</figcaption>', replace, body)
sys.stdout.write(body)
```

### Document transform

A script that transforms the complete document after page template wrapping. Use this for operations that need the full page context (CSS injection, HTML minification, link rewriting).

#### Protocol

Same as body transform (stdin/stdout), but receives the full document including the page template wrapper (`<html>`, `\documentclass`, etc.).

#### Example

Minify HTML output:

```toml
name = "minify"

[document]
run = "minify.sh"
```

```bash
#!/bin/sh
if [ "$CALEPIN_FORMAT" = "html" ]; then
    python3 -c "
import sys
from htmlmin import minify
sys.stdout.write(minify(sys.stdin.read()))
"
else
    cat
fi
```

### Module with multiple stages

A module can combine element partials with scripts:

```toml
name = "mermaid"

[element]
match.classes = ["mermaid"]

[document]
run = "inject_mermaid_js.sh"
```

```html
<!-- mermaid.html -->
<pre class="mermaid">{{ children }}</pre>
```

```bash
#!/bin/sh
# inject_mermaid_js.sh: append Mermaid JS before </body>
sed 's|</body>|<script src="https://cdn.jsdelivr.net/npm/mermaid/dist/mermaid.min.js"></script><script>mermaid.initialize({startOnLoad:true});</script></body>|'
```

The element partial renders `.mermaid` divs as `<pre class="mermaid">`. The document transform injects the Mermaid JS library once into the page.

## Built-in modules

Built-in modules are compiled into the binary. They use the same pipeline stages but implement Rust traits instead of scripts. Users cannot override built-in module behavior, but they can:

- Override built-in partials by placing files in `_calepin/partials/`
- Disable built-in modules by not listing them in `modules`

Built-in modules have access to additional stages not available to user scripts:

| Stage | Built-in only | Reason |
|---|---|---|
| `TransformElementRaw` | tabset, layout, convert_svg_pdf | Needs raw element tree + render closure |
| `TransformPage` | Highlight CSS injection | Needs efficient template var injection |

These stages require either mutable access to the element tree or direct interaction with the template engine, neither of which is expressible through stdin/stdout.

## Resolution order

When multiple sources provide the same element template:

1. Module element transform (from `_calepin/modules/{name}/`)
2. Standalone partial (from `_calepin/partials/{engine}/`)
3. Built-in partial (embedded in binary)

Modules win because they have explicit match rules. Standalone partials win over built-ins because they are user-provided.

## Target configuration

```toml
# _calepin.toml

[targets.html]
engine = "html"
modules = ["highlight", "append_footnotes_html", "embed_images_html", "mermaid"]
crossref = "html"

[targets.latex]
engine = "latex"
modules = ["highlight", "convert_svg_pdf", "inject_color_defs_latex"]
crossref = "latex"
```

Modules run in listed order within each stage. A module can be listed for one target but not another.

## Summary

| Mechanism | Config | Match | Stages | Language |
|---|---|---|---|---|
| Partial | None (file convention) | Implicit (filename) | Element only | Jinja |
| Module | `module.toml` | Explicit (match rule) | Element + body + document | Jinja + any (scripts) |
| Built-in | Compiled | Explicit (registry) | All 5 stages | Rust |
