#let _calepin-document-element = document
#import "/.calepin/calepin.typ": *
#let document = _calepin-document-element

#let _calepin-expected-generation = "93827c1d0082c32e-1349cde127705c16"
#let _calepin-verify-generation() = {
  let path = sys.inputs.at("calepin-results", default: none)
  if path != none and path != "" {
    let actual = json(path).at("generation", default: "")
    if actual != _calepin-expected-generation {
      panic("Calepin results changed while this render was starting; Typst will retry with the completed build")
    }
  }
}
#_calepin-verify-generation()



#let _raw-chunk-langs = ("python", "r", "mermaid", "dot", "tikz", "d2", "html", "sh")
#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("d2", it) }
#show raw.where(block: true, lang: "html", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("html", it) }
#show raw.where(block: true, lang: "sh", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("sh", it) }

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
#import "/.calepin/calepin.typ": _html-themed-raw-block, _is-query, chunk_from_raw_plain

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
  } else if it.lang != none and (_is-query() or _raw-chunk-langs.contains(it.lang)) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

#context _calepin-body-size.update(text.size)

#import "/.calepin/calepin.typ" as calepin_runtime
#set document(title: [Templating])
#import "/.calepin/calepin.typ" as calepin
#metadata((tags: ("themes", "templates"))) <website-metadata>
#title()

Theme layouts are #link("https://docs.rs/minijinja/latest/minijinja/syntax/index.html")[MiniJinja] templates. The HTML layouts (`layouts/site.html` and `layouts/document.html`) and the paged layout (`layouts/pdf.typ`) all use the same engine, so the syntax on this page applies to both. The #link("html_templates.html")[HTML templates] and #link("pdf_templates.html")[PDF templates] pages document the values each layout receives.

= Syntax

MiniJinja has two kinds of tags.

Interpolation prints a value:

```html
<title>{{ doc.title }}</title>
```

Statements control flow. Loops repeat a block:

```html
{% for file in css %}
<style>
{{ file.content }}
</style>
{% endfor %}
```

Conditionals show a block only when a value is set:

```html
{% if site.logo %}
<img src="{{ site.logo }}" alt="{{ site.logo_alt }}">
{% endif %}
```

Includes pull in another template file, which is how themes share partials:

```html
{% include "partials/site-footer.html" %}
```

The #link("https://docs.rs/minijinja/latest/minijinja/syntax/index.html")[MiniJinja syntax reference] covers the rest: filters, tests, macros, and more.

= Context

Each layout receives a context: a set of named values you reference with `{{ }}`. The available names depend on the target.

- HTML layouts receive `site`, `css`, `js`, `doc`, `theme`, `target`, `store`, and more. See #link("html_templates.html")[HTML templates].
- The paged layout receives `doc`, `theme`, `target`, and `store`. See #link("pdf_templates.html")[PDF templates].

Both targets receive `theme`, `target`, and `store`.

= Document store

Pass project-specific template values with `[store]` or `--set store.<name>=...`:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin compile notebook.typ --set store.course=\"Econ 101\" --set store.semester=\"Fall 2026\"\n", block: true, lang: "sh"))

The completed document store is available as top-level `store` in both HTML and paged layouts. It includes project values, document initializers declared with `calepin.store.set()`, and values committed by successful `store-set` chunks. CLI `--set store.<name>=...` values take precedence over project and document initializers. In HTML templates, `store` sits at the top level, not under `site`:

```html
<p>{{ store.course }}, {{ store.semester }}</p>
```

In a `layouts/pdf.typ` paged layout, read the same values and emit Typst:

```typ
#let course = "{{ store.course }}"
```
