#import "../00_syntax-theme.typ": _input-syntax-theme, _output-syntax-theme, _paged-syntax-theme
#import "../core/target.typ": _is-html

// Chunk chrome is emitted as *labeled, unstyled* carriers. Every visual
// treatment is applied later by show rules (see `chrome.typ`), so a document
// author can restyle or strip it with plain Typst:
//
//   #show <calepin-input>: it => it.body            // strip Calepin's box
//   #show <calepin-input>: it => my-frame(it.body)  // restyle
//
// Overrides must reconstruct from `it.body` rather than re-display `it`:
// re-displaying re-fires the default rule, so `it => it` is a no-op and
// `it => my-frame(it)` nests around the default chrome instead of replacing it.
// That is why the carrier is a `block` (one field access) and not the `raw`.

#let _block-lang-label(lang) = {
  if lang == none {
    ""
  } else if lang == "r" {
    "R"
  } else {
    lang
  }
}

#let _raw-block(value, lang: none, theme: auto) = {
  raw(value, block: true, lang: lang, theme: theme)
}

// Default chrome for echoed source. Kept exported: it is the body of the
// default `<calepin-input>` show rule, not something call sites invoke.
#let code-block(
  body,
  fill: rgb("#f7f7f5"),
  stroke: 0.5pt + rgb("#d8d8d2"),
  radius: 2pt,
  inset: (x: 0.65em, y: 0.45em),
  text-fill: rgb("#1f2933"),
  plain: false,
) = {
  let content = if plain {
    body
  } else {
    text(fill: text-fill)[#body]
  }
  block(
    width: 100%,
    fill: fill,
    stroke: stroke,
    radius: radius,
    inset: inset,
  )[
    #content
  ]
}

// Program output is not source code. On the paged target it is set as plain
// monospaced text rather than a `raw` element, so a document-wide `show raw:`
// rule from a package such as codly styles the code a chunk contains without
// also reaching what the chunk printed. Spaces become non-breaking so column
// alignment in printed tables survives; `raw` gave us that for free.
#let _mono-text(value) = {
  let lines = value.trim("\n", at: end).split("\n")
  set text(font: "DejaVu Sans Mono")
  lines.map(line => line.replace(" ", "\u{00A0}")).join(linebreak())
}

// Default chrome for the four output kinds; the body of the default show rules.
#let _output-chrome(body, kind: "stdout") = {
  let erroring = kind == "error" or kind == "warning"
  let fill = if erroring { rgb("#fffaf7") } else { rgb("#fbfbfa") }
  let stroke = if erroring {
    (rest: 0.5pt + rgb("#e2c7ba"), left: 1.5pt + rgb("#c48672"))
  } else {
    (rest: 0.5pt + rgb("#ddddda"), left: 1.5pt + rgb("#cfcfc8"))
  }
  code-block(
    fill: fill,
    stroke: stroke,
    radius: 2pt,
    inset: (x: 0.65em, y: 0.4em),
    plain: true,
  )[
    #if erroring {
      text(fill: rgb("#5f3328"))[#body]
    } else {
      body
    }
  ]
}

#let _input-carrier(body) = [#block(body) <calepin-input>]

#let _output-carrier(body, kind: "stdout") = {
  if kind == "result" {
    [#block(body) <calepin-result>]
  } else if kind == "warning" {
    [#block(body) <calepin-warning>]
  } else if kind == "error" {
    [#block(body) <calepin-error>]
  } else {
    [#block(body) <calepin-output>]
  }
}

// The carrier body always carries an explicit syntax `theme:`, which serves
// double duty: highlighting survives a user strip rule, and the fenced-chunk
// show rules (which select `theme: auto`) never match Calepin-emitted raw, so
// the sentinel holds even after a user reconstructs `it.body` elsewhere. A
// document-level `set raw(theme: ..)` does not set the field on the element, so
// chunk detection keeps matching the user's own fences either way.
//
// The two targets need different themes. HTML gets the sentinel, whose colors
// are rewritten into CSS classes after export; that rewrite is what drives
// light/dark. Paged gets the real palette, since nothing rewrites it later.
//
// A document's own `set raw(theme: ..)` therefore does not reach chunk source.
// Forwarding it is not merely unimplemented: the value arrives as the path the
// user wrote, and Typst resolves a relative path against the file holding the
// expression, which here is this module under `.calepin/runtime/`. Only bytes
// and root-absolute paths would survive the trip.
#let _source-block(code, lang: none) = {
  if _is-html() {
    std.html.elem("div", attrs: (
      class: "sourceCode",
      "data-lang": _block-lang-label(lang),
    ))[
      #_input-carrier(_raw-block(code, lang: lang, theme: _input-syntax-theme))
    ]
  } else {
    _input-carrier(_raw-block(code, lang: lang, theme: _paged-syntax-theme))
  }
}

#let _html-themed-raw-block(it) = {
  let lang = if it.has("lang") { it.lang } else { none }
  _source-block(it.text, lang: lang)
}

#let _input-block(code, lang: none) = {
  _source-block(code, lang: lang)
}

#let _output-block(output, kind: "stdout") = {
  // HTML keeps a real `raw`, which exports as `<pre><code>`: whitespace is
  // preserved by the browser and the markup stays semantic.
  let body = if _is-html() {
    _raw-block(output, theme: _output-syntax-theme)
  } else {
    _mono-text(output)
  }
  let carrier = _output-carrier(body, kind: kind)
  if _is-html() {
    let class = if kind == "error" or kind == "warning" {
      "cell-output cell-output-stderr"
    } else {
      "cell-output cell-output-stdout"
    }
    std.html.elem("div", attrs: (class: class))[
      #carrier
    ]
  } else {
    carrier
  }
}
