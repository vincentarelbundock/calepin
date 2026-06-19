#import "/.calepin/calepin.typ": *



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

// Notebook theme
#import "/.calepin/calepin.typ": _html-themed-raw-block, chunk_from_raw_plain
#import "/.calepin/calepin.typ" as calepin
#import "@preview/marginalia:0.3.1" as marginalia

#let _superscript-numbering(pattern, ..i) = super(numbering(pattern, ..i))

// Place margin elements with marginalia in paged output. Page geometry stays
// under author control; add `marginalia.setup` in the document or a local theme
// when you want to reserve margin space.
#show raw.where(block: true): set text(size: .8em)

#calepin.elements.set-margin-impl(
  note: (body, numbering: auto, side: auto) => {
    let note-numbering = if numbering == auto { "1" } else { numbering }
    let marker-numbering = if type(note-numbering) == str {
      (..i) => _superscript-numbering(note-numbering, ..i)
    } else if type(note-numbering) == function {
      (..i) => super(note-numbering(..i))
    } else {
      note-numbering
    }
    let args = (numbering: marker-numbering)
    if marker-numbering != none {
      args.insert("anchor-numbering", marker-numbering)
    }
    if side != auto { args.insert("side", side) }
    marginalia.note(body, ..args)
  },
  figure: (body, caption: none, side: auto) => {
    let args = (:)
    if side != auto { args.insert("side", side) }
    marginalia.notefigure(body, caption: caption, ..args)
  },
)

#show raw.where(block: true): it => {
  if it.theme != auto {
    it
  } else if it.lang != none and _raw-chunk-langs.contains(it.lang) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

#include "/.calepin/tufte/source.typ"
