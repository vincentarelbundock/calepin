# Extension System Specification

## Concepts

- **Extension** -- A directory with an `extension.toml` manifest. The single unit of distribution and customization. Bundles a target definition, partials, modules, and assets. Built-in targets (html, latex, typst, slides, website, book) are also extensions.
- **Target** -- An output profile: which writer to use, which modules to run, which file extension to produce. Defined in the `[target]` section of an extension manifest or in `_calepin/config.toml`.
- **Writer** -- One of the four rendering engines: `html`, `latex`, `typst`, `markdown`. Every target has exactly one writer. Writers are not user-extensible.
- **Partial** -- A Jinja template that renders a specific element or page component (e.g., `figure.html`, `page.tex`). Partials are resolved by name through a layered chain. Adding a partial is enough to create a custom div style -- no module needed.
- **Module** -- A declared transform in the extension manifest. Only needed when partials alone are insufficient: structural transforms that rewrite children, match rules beyond class names, auto-numbering, or external code execution.
- **Asset** -- A CSS, JS, or static file declared in the manifest's `[assets]` section. CSS/JS are injected into the page template; static files are copied to the output directory.
- **Vars** -- Key-value pairs declared in `[vars]` in the extension manifest. Namespaced by extension name: `{{ vars.tufte.sidenote_style }}`. Extensions provide defaults; users override in front matter under `[vars.extension_name]`.

## Overview

Both built-in targets and user-created targets are extensions -- the internal architecture mirrors the user-facing one exactly.

## Extension directory layout

```
my-extension/
  extension.toml        # Manifest (required) -- complete index of everything
  partials/             # Template overrides
    html/
      page.html
      figure.html
    common/
      note.jinja
  assets/               # CSS, JS, images, fonts
    tufte.css
    tufte.js
  scripts/              # External module executables (scripts, WASM)
    postprocess.sh
    transform.wasm
```

All subdirectories are optional. An extension can be as small as an `extension.toml` plus one CSS file.

All module declarations live in `extension.toml` itself. There are no separate `module.toml` files. Module templates are resolved from `partials/` like any other partial (matched by class name). External executables live in `scripts/`.

## Manifest: `extension.toml`

The manifest is the complete index of everything the extension provides. Nothing is discovered by directory scanning.

```toml
name = "tufte"
description = "Tufte-style HTML with sidenotes and margin notes"
version = "1.0.0"
author = "Jane Doe"
license = "MIT"

# Single-chain inheritance: exactly one parent.
# The parent must be a built-in target or another installed extension.
inherits = "html"

# ---------------------------------------------------------------------------
# Target definition
# ---------------------------------------------------------------------------

# Full target definition (same fields as [targets.X] in config.toml).
# These override the parent's values after inheritance resolution.
[target]
writer = "html"                     # inherited if omitted
extension = "html"                  # inherited if omitted
template = "page"                   # inherited if omitted
fig_extension = "svg"               # inherited if omitted
preview = "serve"                   # inherited if omitted
embed_resources = true
crossref = "html"                   # inherited if omitted
toc_headings = true                 # inherited if omitted

# Modules: full ordered list. Replaces parent's module list entirely.
# Omit to inherit parent's modules unchanged.
modules = ["highlight", "append_footnotes", "sidenotes", "embed_images"]

# Post-processing commands. Run in order after rendering.
# {input} = rendered file, {output} = final output path.
# post = ["typst compile {input}", "pagefind --site {output}"]

# Extension variables: namespaced under the extension name.
# Available in templates as {{ vars.tufte.key }}.
# Users can override in front matter: [vars.tufte] sidenote_style = "popup"
[vars]
sidenote_style = "margin"
cdn = "https://cdn.example.com/tufte"

# Extra page template variables.
# [target.page_vars]
# base = "html"

# Preferred figure formats, in priority order.
# fig_formats = ["svg", "png", "jpg"]

# ---------------------------------------------------------------------------
# Assets
# ---------------------------------------------------------------------------

# Paths are relative to the extension directory.
# CSS files are injected into the page template via {{ css }}.
# JS files are injected via {{ js }}.
# Static files are copied to assets/ in the output directory.
[assets]
css = ["tufte.css"]
js = ["tufte.js"]
static = ["fonts/"]

# ---------------------------------------------------------------------------
# Modules
# ---------------------------------------------------------------------------

# Each [[modules]] entry declares a module this extension provides.
# The manifest is the complete index -- no separate module.toml files.

# Most extensions don't need [[modules]] at all. If you just want a custom
# div style, add a partial to partials/{writer}/ and use ::: {.my-class}
# -- the partial is resolved by class name automatically.
#
# Declare a [[modules]] entry only when you need capabilities beyond what
# a partial provides:
#   - Match by attribute, id prefix, or writer (not just class)
#   - Structural transforms (rewrite children before rendering)
#   - Auto-numbering (number = true)
#   - External code (run = "scripts/foo.sh" or "scripts/foo.wasm")

# --- Structural transform ---
[[modules]]
name = "sidenotes"
description = "Convert footnotes to margin sidenotes"
kind = "element_children"
contexts = ["div"]

[modules.match]
classes = ["sidenote", "margin-note"]
# attrs = ["sidenote-ref"]          # attribute names (OR'd with classes)
# id_prefix = "sn-"                 # ID prefix trigger
# writers = ["html"]                # restrict to writers (omit = all)
# number = true                     # auto-number matching divs

# --- External module (script or WASM) ---
# [[modules]]
# name = "rewrite-links"
# description = "Rewrite external links to open in new tab"
# kind = "document"
# run = "scripts/rewrite-links.sh"  # .wasm for WASM runtime
```

### Module kinds

| `kind` | When it runs | What it does |
|---|---|---|
| `span` | During render, per span | Renders matching bracketed spans |
| `element_children` | During render, per div | Rewrites div children (structural transforms) |
| `element` | Pre-render | Mutates individual elements before rendering |
| `document` | Post-assembly | Transforms the full document string |

### Module implementations

A module is a declaration in the manifest. How it runs depends on whether it has a `run` field:

Most div/span customization does not require a module at all. Adding a partial to `partials/{writer}/` is enough -- the renderer resolves partials by class name, so `::: {.my-class}` automatically uses `partials/html/my_class.html`.

A `[[modules]]` declaration is needed only when you require:

- **Match rules** beyond class name (attributes, id prefix, writer restrictions)
- **Structural transforms** that rewrite children before rendering (like tabset, layout, figure)
- **Auto-numbering** (`number = true` injects `{{ number }}` and `{{ type_class }}` into the template context)
- **External code** that runs a script or WASM binary

Modules with a `run` field execute an external program. The runtime is determined by the file extension:

- `.wasm` -- executed in the Extism WASM runtime (sandboxed, portable)
- Anything else (`.sh`, `.py`, `.rb`, etc.) -- executed as a subprocess

### External module protocol

External modules (both scripts and WASM) communicate via JSON on stdin/stdout.

**Element modules** (`span`, `element_children`, `element`) receive a JSON object describing the matched element and return a JSON result:

```json
// stdin
{
  "name": "sidenote",
  "kind": "element_children",
  "writer": "html",
  "context": "div",
  "content": "<p>The rendered children...</p>",
  "classes": ["sidenote"],
  "id": "sn-1",
  "attrs": {"ref": "fn1"},
  "number": 3,
  "vars": {"sidenote_style": "margin"}
}
```

```json
// stdout
{
  "action": "rendered",
  "output": "<aside class=\"sidenote\">...</aside>"
}
```

The `action` field controls what happens next:

| `action` | Meaning |
|---|---|
| `"rendered"` | Use `output` as the final rendering. Stop further dispatch. |
| `"pass"` | This module declines. Continue to next matching module or fallback template. |

**Document modules** (`document`) receive the full document:

```json
// stdin
{
  "name": "rewrite-links",
  "kind": "document",
  "writer": "html",
  "body": "<!DOCTYPE html>...",
  "vars": {}
}
```

```json
// stdout
{
  "action": "rendered",
  "output": "<!DOCTYPE html>..."
}
```

Document modules can also use plain text protocol: raw document on stdin, transformed document on stdout (no JSON wrapping). The manifest can declare this explicitly:

```toml
[[modules]]
name = "rewrite-links"
kind = "document"
run = "scripts/rewrite-links.sh"
protocol = "text"                    # "json" (default) or "text"
```

### Partials vs. modules

**Partials alone** handle most customization. Add `partials/html/note.html` to your extension and `::: {.note}` renders using that template. No module declaration needed. The partial receives standard variables (content, classes, attrs, id, etc.) and is overridable at every level of the resolution chain.

**Modules** add behavior that partials cannot express: structural transforms, match rules beyond class names, auto-numbering, and external code execution. A module that matches an element still resolves its template via the partial chain (by class name), unless it returns rendered output directly (external modules with `"action": "rendered"`).

### Inheritance rules

- `inherits` names exactly one parent: a built-in base target (`html`, `latex`, `typst`, `markdown`), a built-in composed target (`slides`, `website`, `book`), or an installed extension.
- Resolution is recursive with cycle detection (same algorithm as current target inheritance).
- Field merge: child values override parent values. `Option` fields use child-or-parent. `Vec` fields (modules, post, fig_formats) use all-or-nothing: if the child specifies the field, it replaces the parent's list entirely; if omitted, the parent's list is inherited.
- Partials, assets, and modules compose additively (see resolution order below).

## Installation

Extensions live in `_calepin/extensions/{name}/`:

```
project/
  _calepin/
    config.toml
    extensions/
      tufte/
        extension.toml
        partials/...
        assets/...
      lightbox/
        extension.toml
        assets/...
```

A future `calepin install` command could fetch extensions from a registry or git URL. For now, users copy or clone the directory manually.

## Activation

An extension that defines a target is activated by using it as the target name:

```toml
# _calepin/config.toml or document front matter
target = "tufte"
```

An extension that only provides modules or assets (no custom target) is activated by listing it in the document or project config:

```toml
[calepin]
extensions = ["lightbox"]
```

When `extensions` is set, those extensions' assets are included and their modules are available (but only run if listed in the active target's `modules` list or if they match by class/attr/id like any other module).

## Resolution order

### Partials

First match wins:

1. Document sidecar: `{stem}_calepin/partials/{target|writer|common}/`
2. Project overrides: `_calepin/partials/{target|writer|common}/`
3. Active extension's partials: `_calepin/extensions/{name}/partials/{writer|common}/`
4. Parent extension's partials (walking up the inheritance chain)
5. Built-in partials (embedded in binary)

At each level, the lookup order within the partials directory is:

1. `{target}/{name}.{ext}` (target-specific, e.g., `tufte/page.html`)
2. `{writer}/{name}.{ext}` (writer-specific, e.g., `html/page.html`)
3. `common/{name}.jinja` (writer-agnostic)

### Assets

Assets compose additively (all levels contribute, no shadowing):

1. Project assets: `_calepin/assets/` (always included)
2. Active extension's assets (CSS/JS declared in manifest)
3. Parent extension's assets (walking up the inheritance chain)
4. Built-in assets (embedded in binary, fill gaps)

CSS and JS from multiple extensions are concatenated in inheritance order (parent first, child last), so the child's styles override the parent's.

Static files from extensions are copied to `assets/` in the output directory. If two extensions provide a file at the same path, the child's file wins.

### Modules

Modules are resolved from:

1. Active extension's `[[modules]]` entries (declared in its `extension.toml`)
2. Parent extension's `[[modules]]` entries (walking up the inheritance chain)
3. Built-in modules (embedded in binary)

Module templates are resolved via the standard partial resolution chain (see Partials above). A module's matched class name determines the partial lookup, so a `sidenote` module's template is found at `partials/html/sidenote.html` in whichever level of the chain provides it first.

A module must be listed in the active target's `modules` field to run as a document/element transform. Modules that match by class/attr/id (span transforms, element children transforms) run whenever a matching element is encountered, regardless of the `modules` list.

## Built-in targets as extensions

The built-in targets are structured internally as extensions. Each has the same shape:

```
(embedded at compile time)

html/
  extension.toml        # writer = "html", modules = [...]
  partials/
    html/
      page.html
      code_source.html
      figure.html
      ...

latex/
  extension.toml
  partials/
    latex/
      page.tex
      ...

slides/
  extension.toml        # inherits = "html"
  partials/
    slides/
      page.html         # overrides html/page.html

website/
  extension.toml        # inherits = "html"
  partials/
    website/
      base.html
      navbar.html
      sidebar.html
  assets/
    calepin.css
    css/...
    fontawesome/...
```

The current `config/document.toml` target entries become the `[target]` section of each extension's `extension.toml`. The current `partials/{writer}/` directories map directly to `{name}/partials/{writer}/`. Built-in modules remain in Rust code; they are registered by name in `modules.toml` as today.

This means:

- `slides` inherits from `html`, overriding `page.html` and adding `split_slides` to modules.
- `website` inherits from `html`, adding navbar/sidebar partials, CSS assets, and `document_listing` support.
- A user extension inherits from any of these and follows the same rules.

## `calepin init extension`

Scaffolds a new extension directory:

```
$ calepin init extension sidenotes --inherits html
```

Creates:

```
sidenotes/
  extension.toml
  partials/
    html/
      .gitkeep
  assets/
    .gitkeep
```

With a starter `extension.toml`:

```toml
name = "sidenotes"
description = ""
version = "0.1.0"
inherits = "html"

[target]
# modules = ["highlight", "append_footnotes", "embed_images"]

[assets]
# css = []
# js = []

# Declare [[modules]] only if you need structural transforms,
# match rules beyond class names, auto-numbering, or external code.
# For simple div styles, just add a partial -- no module needed.
```

The user can then:

1. Add partials to `partials/html/` to override page templates or add module templates (e.g., `sidenote.html`).
2. Declare modules in `extension.toml` under `[[modules]]`.
3. Add CSS/JS to `assets/` and declare them in `extension.toml`.
4. Copy the directory into a project's `_calepin/extensions/` to install it.

## Examples

### Example 1: Visual theme (CSS-only)

```
dark-academic/
  extension.toml
  assets/
    dark-academic.css
```

```toml
# extension.toml
name = "dark-academic"
description = "Dark color scheme with serif typography"
version = "1.0.0"
inherits = "html"

[assets]
css = ["dark-academic.css"]
```

No partials, no modules, no target overrides. The extension only adds a stylesheet. Activated with `target = "dark-academic"` or `calepin.extensions = ["dark-academic"]`.

### Example 2: Presentation target

```
slidev/
  extension.toml
  partials/
    html/
      page.html
  assets/
    slidev.css
    slidev.js
  scripts/
    slidev.wasm
```

```toml
name = "slidev"
description = "Slidev-compatible HTML presentations"
version = "0.2.0"
inherits = "slides"

[target]
modules = ["highlight", "append_footnotes", "split_slides"]
embed_resources = false

[vars]
transition = "slide"    # {{ vars.slidev.transition }}

[assets]
css = ["slidev.css"]
js = ["slidev.js"]
```

Inherits from `slides` (which inherits from `html`). Overrides `page.html` for its own slide shell. The inheritance chain is `slidev -> slides -> html`.

### Example 3: Academic journal target

```
jss/
  extension.toml
  partials/
    latex/
      page.tex
      code_source.tex
  assets/
    jss.cls
    jss.bst
```

```toml
name = "jss"
description = "Journal of Statistical Software article format"
version = "1.0.0"
inherits = "latex"

[target]
modules = ["highlight", "convert_svg_pdf"]
fig_extension = "pdf"

[vars]
documentclass = "jss"   # {{ vars.jss.documentclass }}

[assets]
static = ["jss.cls", "jss.bst"]
```

Overrides the LaTeX page template and code rendering. Copies `.cls` and `.bst` files alongside the output.

### Example 4: PDF via compile step

```
pdf/
  extension.toml
```

```toml
name = "pdf"
description = "PDF output via Typst compilation"
version = "1.0.0"
inherits = "typst"

[target]
extension = "pdf"
post = ["typst compile {input}"]
```

No partials, no assets, no modules. Just inherits everything from `typst` and adds a post-processing step. A user who prefers LaTeX-based PDF would write their own extension with `inherits = "latex"` and `post = ["tectonic {input}"]` instead.

### Example 5: Structural module (no target)

```
lightbox/
  extension.toml
  partials/
    html/
      lightbox.html
  assets/
    lightbox.css
    lightbox.js
```

```toml
name = "lightbox"
description = "Click-to-zoom image lightbox for HTML figures"
version = "1.0.0"
inherits = "html"

[assets]
css = ["lightbox.css"]
js = ["lightbox.js"]

[[modules]]
name = "lightbox"
description = "Wrap figure images in click-to-zoom lightbox links"
kind = "element_children"
contexts = ["div"]

[modules.match]
classes = ["lightbox"]
writers = ["html"]
```

Everything is declared in the manifest. The template at `partials/html/lightbox.html` is resolved by class name through the standard partial chain. Activated with `calepin.extensions = ["lightbox"]`; the module runs whenever a matching div is encountered.

## Terminology

Three user-facing concepts, one implementation detail:

| Term | What it is |
|---|---|
| **Extension** | A directory with `extension.toml` that bundles targets, partials, modules, and assets. The unit of distribution. |
| **Module** | A declared transform in `extension.toml`. Matches elements by class/attr/id and runs during the render pipeline. |
| **Partial** | A Jinja template that renders a specific element or page component. |
| **WASM** | An implementation detail. A module whose `run` field points to a `.wasm` file executes in the Extism runtime instead of as a subprocess. Users do not need to know or care about this distinction. |

The word "plugin" is retired. What was previously called a plugin is now a module with `run = "scripts/foo.wasm"`.

## Migration from current architecture

### What changes

1. **`config/document.toml` target entries** become `extension.toml` manifests for each built-in target. The TOML structure is nearly identical; `[targets.html]` becomes `[target]` inside `html/extension.toml`.

2. **`config/modules.toml` entries** move into the appropriate extension's `[[modules]]` declarations. Built-in Rust implementations remain in code, but their declarations live in the extension manifest instead of a separate file.

3. **`partials/{writer}/` directories** move under each extension: `html/partials/html/`, `latex/partials/latex/`, etc. The embedding strategy (`include_dir!`) stays the same but scoped per extension.

4. **`themes.rs` and `themes/` directory** are replaced by the extension system. The current `Theme` struct maps directly to an extension with only `partials/` and no target overrides. The `minimal` theme becomes a `minimal` extension that inherits `website`.

5. **`modules/manifest.rs`** (`ModuleManifest`, `module.toml` parsing) is replaced by `ExtensionManifest` parsing. Module declarations move from per-module `module.toml` files into the extension manifest's `[[modules]]` array.

6. **WASM plugins** (`plugins/` directory, Extism PDK) become modules with `run = "scripts/foo.wasm"`. The `calepin.plugins` config key is replaced by `calepin.extensions`. The WASM runtime stays the same; only the packaging changes.

7. **Partial resolution** gains one new level: extension partials sit between project overrides and built-in partials. The current dual-mode behavior (filesystem-only or built-in-only) is replaced by the layered chain.

8. **Asset handling** gains explicit declaration. Instead of implicitly copying everything from `_calepin/assets/`, extensions declare which CSS/JS files they contribute. The pipeline concatenates CSS/JS from the inheritance chain and injects them into the page template.

9. **`calepin init website`** becomes `calepin init website --extension default` (or just `calepin init website`, using the default extension). The scaffold copies extension files into `_calepin/extensions/` or uses the built-in extension directly.

### What stays the same

- The `Target` struct and its fields.
- The render pipeline stages (parse, evaluate, render, crossref, assemble, transform).
- Module matching logic (`MatchRule.matches()`) and dispatch by kind.
- Sidecar directories for per-document overrides.
- Front matter config and merge order.
- The Extism WASM runtime (used for modules with `.wasm` executables).
- The `FormatPipeline` struct and its construction from a resolved `Target`.

### Internal refactoring sequence

1. Define `ExtensionManifest` struct (parsed from `extension.toml`). Includes `[target]`, `[assets]`, and `[[modules]]` sections. Replaces both `ThemeManifest` and `ModuleManifest`.
2. Write `extension.toml` manifests for each built-in target (`html`, `latex`, `typst`, `markdown`, `slides`, `website`, `book`), replacing entries in `document.toml`, `collection.toml`, and `modules.toml`.
3. Restructure embedded partials: group by extension name instead of flat `partials/{writer}/`.
4. Update `resolve_target()` to walk the extension inheritance chain.
5. Update module registry to load `[[modules]]` from extension manifests instead of `modules.toml` + per-module `module.toml` files.
6. Update partial resolution to check extension directories between project overrides and built-in fallbacks.
7. Update asset copying to read `[assets]` declarations from the extension chain.
8. Replace `themes.rs` with the extension resolver.
9. Add `calepin init extension` subcommand.

## Design constraints

- **Single inheritance only.** No diamond inheritance, no multiple parents, no layers. One chain: child -> parent -> grandparent -> ... -> built-in base target.
- **Full module list replacement.** If an extension specifies `modules` in `[target]`, it replaces the parent's list entirely. No `modules_add`, no `modules_remove`, no ordering ambiguity.
- **Explicit everything.** The manifest is the complete index. CSS, JS, static files, and modules must be declared in `extension.toml`. No implicit directory scanning.
- **No config fragments.** An extension does not inject arbitrary front matter keys. It provides a target definition, partials, modules, and assets. Document-level config (title, author, bibliography, execute options) stays in the document or project config.
- **Additive partial chain.** Each level in the chain can override partials from the level below. The user always has final say (project `_calepin/partials/` overrides everything).
- **Modules are for behavior, partials are for presentation.** Adding a partial to `partials/` is enough for custom div/span styles. A `[[modules]]` declaration is only needed for structural transforms, advanced matching, auto-numbering, or external code.

## Open question: partial resolution vs. wholesale copy

The spec above describes a layered partial resolution chain: user overrides > extension > parent extension > built-in. Each level can override individual partials while inheriting the rest.

But the current system does not work this way. Today, `calepin init website` copies *all* built-in partials into `_calepin/partials/`. The user gets a complete, self-contained set of templates. Resolution is then flat: either everything comes from the filesystem, or everything comes from the built-ins. There is no mixing.

This wholesale-copy approach has real advantages:

- **Predictability.** The user sees every template that will be used. No hidden inheritance, no wondering "which level is this partial coming from?"
- **Stability.** When *Calepin* updates its built-in partials, the user's project is unaffected. They opted into a specific version of the templates at scaffold time.
- **Debuggability.** If output looks wrong, the user can inspect and edit the local file directly. There is no need to understand a resolution chain.

But it conflicts with the extension model:

- **An extension that overrides 3 partials must ship all ~50.** Otherwise, the remaining partials have to come from somewhere, which requires the layered chain. If we keep the wholesale-copy model, every extension must be a complete fork of its parent's template set.
- **Updates don't propagate.** If the `html` extension fixes a bug in `code_source.html`, no child extension or user project picks it up unless they manually re-copy.
- **Extension authoring is heavy.** Creating a "tufte" extension that only changes `page.html` and `figure.html` would require copying and maintaining 48 other files verbatim.

The layered chain solves these problems but introduces the "which level did this come from?" question.

Possible approaches:

1. **Layered resolution (as specced above).** Extensions only ship the partials they override. Missing partials fall through to the parent. Accept the resolution complexity.

2. **Wholesale copy with scaffolding.** `calepin init extension --inherits html` copies all of the parent's partials into the new extension. The extension is self-contained. Updates require re-scaffolding. This is simple but heavy.

3. **Hybrid: layered by default, `calepin eject` to flatten.** Normal operation uses the layered chain. A `calepin eject` (or `calepin export-partials`) command copies all resolved partials into the project's `_calepin/partials/`, switching to the flat model. Users who want full control can eject; extensions stay lightweight.

4. **Layered resolution with `calepin info --partials` for debugging.** Keep the layered chain, but provide a command that shows exactly where each partial is resolved from (extension name, file path). Makes the chain inspectable without flattening it.

This decision affects the entire extension architecture. If we keep wholesale copy, extensions are heavier but simpler. If we go layered, extensions are lightweight but the resolution chain is a new concept users must understand.
