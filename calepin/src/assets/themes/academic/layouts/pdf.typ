#import "/.calepin/calepin.typ": _html-themed-raw-block, chunk_from_raw_plain
#import "/.calepin/calepin.typ" as calepin
#import "@preview/marginalia:0.3.1" as marginalia

#let _superscript-numbering(pattern, ..i) = super(numbering(pattern, ..i))

// Place margin elements with marginalia in paged output. Page geometry stays
// under author control; add `marginalia.setup` in the document or a local theme
// when you want to reserve margin space.
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

{{ doc.body }}
