#import "/.calepin/calepin.typ": *



#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => it)
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => it)
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { it } else { chunk-from-raw-plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { it } else { chunk-from-raw-plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { it } else { chunk-from-raw-plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { it } else { chunk-from-raw-plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { it } else { chunk-from-raw-plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { it } else { chunk-from-raw-plain("d2", it) }

// Paged theme
#let code-block(
  body,
  fill: rgb("#f7f7f5"),
  stroke: 0.5pt + rgb("#d8d8d2"),
  radius: 2pt,
  inset: (x: 0.65em, y: 0.45em),
  text-fill: rgb("#1f2933"),
) = {
  block(
    width: 100%,
    fill: fill,
    stroke: stroke,
    radius: radius,
    inset: inset,
  )[
    #text(fill: text-fill)[#body]
  ]
}

#show raw.where(block: true): it => {
  if sys.inputs.at("calepin-target", default: "paged") == "html" {
    it
  } else if it.theme != auto {
    it
  } else if it.lang != none and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    it
  } else {
    code-block(it)
  }
}

#include "/.calepin/websites/config/source.typ"
