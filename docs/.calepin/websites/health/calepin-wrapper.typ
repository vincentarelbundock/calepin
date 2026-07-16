#let _calepin-document-element = document
#import "/.calepin/calepin.typ": *
#let document = _calepin-document-element



#let _raw-chunk-langs = ("python", "r", "mermaid", "dot", "tikz", "d2", "text")
#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("d2", it) }
#show raw.where(block: true, lang: "text", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("text", it) }

#show raw.where(block: true, theme: auto): it => {
  if _is-query() {
    it
  } else if _disable-raw-chunk-transforms.get() {
    _html-themed-raw-block(it)
  } else if it.has("lang") and it.lang != none and _raw-chunk-langs.contains(it.lang) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

#show heading: it => {
  if _is-html() and "label" in it.fields() {
    std.html.elem("calepin-heading-anchor", attrs: (data-id: str(it.label)))
  }
  it
}

// Notebook theme
#import "/.calepin/calepin.typ": _html-themed-raw-block, chunk_from_raw_plain

// Body text size, captured below at document-body level. Code blocks are sized
// relative to this rather than to `1em`, which would compound: a literal
// ```typ block is rendered by replacing its source `raw` element, so it renders
// inside Typst's already-reduced raw text context, whereas executed chunks are
// emitted as ordinary calls at body size. Anchoring to the captured body size
// gives both paths a single, matching reduction instead of shrinking twice.
#let _calepin-body-size = std.state("calepin-body-size", 11pt)

#show raw.where(block: true): it => {
  if it.theme != auto {
    context {
      set text(size: _calepin-body-size.get() * 0.8)
      it
    }
  } else if it.lang != none and _raw-chunk-langs.contains(it.lang) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

#context _calepin-body-size.update(text.size)

#import "/.calepin/calepin.typ" as calepin
#set document(title: [Health checks])
#metadata((title: "Health", tags: ("websites", "CLI", "validation"))) <website-metadata>

#title()

`calepin health` runs quick diagnostic checks on the current project (notebook or website) and reports anything that could break rendering or execution.

It checks the following by default:

- executable availability for configured engines and diagram tools
- Python/Jupyter kernel metadata consistency
- local literal links inside `.typ` files
- local literal images inside `.typ` files
- missing alt text on literal Typst images
- duplicate explicit page routes from `slug` and `url` page metadata

```text
calepin health [OPTIONS]
```

Use `--json` for machine-readable output.

= Link checking

`health` scans `#link("...")` literals in Typst source files and verifies targets.

- A bad local target (for example `#link("missing.html")`) is reported as an **error**.
- External `http://` and `https://` links are ignored by default.
- Pass `--check-external-links` to validate HTTP(S) links too.

By default, `health` walks directories recursively from the current directory,
ignoring `.calepin`, `.git`, `target`, `node_modules`, and `.venv`.
You can limit recursion depth with `--depth`:

```text
calepin health --depth 2
```

With recursive links that may refer to generated outputs, run `calepin health` after a
build so referenced pages exist.

= Image checking

`health` scans literal `#image("...")` calls in Typst source files. Missing local image files are reported as **errors**. Images without non-empty `alt:` text are reported as **warnings**.

Relative image paths are resolved from the source file that contains the image call. Root-relative paths are resolved from the project root. External and special targets such as `http://`, `https://`, `data:`, `mailto:`, and fragment-only targets are ignored.

Alt text is considered present when the image call has an `alt:` argument with a non-empty string or a computed value. These calls pass the accessibility check:

```typ
#image("assets/chart.png", alt: "Line chart of quarterly revenue")
#image("assets/logo.svg", alt: figure_alt)
```

These calls produce warnings:

```typ
#image("assets/chart.png")
#image("assets/chart.png", alt: "")
#image("assets/chart.png", alt: none)
```

= Page route checking

`health` scans `<website-metadata>` for literal `slug` and `url` values and reports duplicate or unsafe output routes as **errors**. The check compares the rendered HTML route, so pages in different directories can use the same slug as long as they do not write the same output path.

For example, these two pages collide because they both render to `same.html`:

```typ
#metadata((slug: "same")) <website-metadata>
#metadata((url: "same.html")) <website-metadata>
```

Routes that escape the output directory are also errors:

```typ
#metadata((slug: "../outside")) <website-metadata>
#metadata((url: "/absolute/path")) <website-metadata>
```

= Source scanning

Raw Typst code spans and blocks are ignored by source checks, so documentation examples do not need to point at real files.

The source checks are intentionally conservative: they only inspect literal `#link("...")`, `#image("...")`, `slug: "..."`, and `url: "..."` values. Dynamic expressions are left to the normal build process.

`--strict` turns warnings into failures (in addition to errors).

```text
calepin health --strict --check-external-links
```
