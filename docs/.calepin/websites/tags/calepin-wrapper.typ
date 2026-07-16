#let _calepin-document-element = document
#import "/.calepin/calepin.typ": *
#let document = _calepin-document-element



#let _raw-chunk-langs = ("python", "r", "mermaid", "dot", "tikz", "d2")
#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("d2", it) }

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

#set document(title: [Tags and taxonomies])
#metadata((title: "Tags", tags: ("websites", "metadata", "navigation"))) <website-metadata>

#title()

A taxonomy groups pages by shared metadata such as tags, categories, authors, or topics. You can build one without special configuration: add an array to each page's `<website-metadata>`, then filter the entries returned by `calepin.pages()`.

For example, documentation pages on this site declare tags alongside their other metadata:

```typ
#metadata((
  title: "Pages",
  tags: ("websites", "metadata"),
)) <website-metadata>
```

Create a regular page for the taxonomy index. The following generic function collects the values of any array-valued metadata field, then lists the matching pages under each value:

```typ
#import "/.calepin/calepin.typ" as calepin

#let taxonomy(field) = {
  let pages = calepin.pages()
    .filter(page => page.meta.at(field, default: ()) != ())
  let terms = pages
    .map(page => page.meta.at(field))
    .flatten()
    .dedup()
    .sorted(key: term => lower(term))

  for term in terms {
    heading(term, level: 2)
    for page in pages.filter(
      page => term in page.meta.at(field)
    ) [
      - #link(page.href)[#page.title]
    ]
  }
}

#taxonomy("tags")
```

This approach keeps taxonomies ordinary Typst code. You control the page's route and presentation, and the same function works for fields such as `categories` or `authors`. Add sorting, counts, summaries, or pagination using the usual Typst array methods.

= Tags used in this documentation

#let taxonomy(field) = {
  let pages = calepin.pages()
    .filter(page => page.meta.at(field, default: ()) != ())
  let terms = pages
    .map(page => page.meta.at(field))
    .flatten()
    .dedup()
    .sorted(key: term => lower(term))

  for term in terms {
    heading(term, level: 2)
    for page in pages.filter(
      page => term in page.meta.at(field)
    ) [
      - #link(page.href)[#page.title]
    ]
  }
}

#taxonomy("tags")
