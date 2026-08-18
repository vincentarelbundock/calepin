#import "../core/target.typ": _is-html, _is-query
#import "margin.typ": _margin-impl

// A figure placed in the margin. In paged output the placement defers to the
// installed margin implementation (see `margin.typ`); the default renders an
// ordinary inline figure.
#let sidefigure(body, caption: none, side: auto, alt: none) = {
  if _is-query() {
    return body
  }

  if _is-html() {
    return html.elem("figure", attrs: (class: "calepin-sidefigure"))[
      #body
      #if caption != none {
        html.elem("figcaption")[#caption]
      }
    ]
  }

  context {
    let impl = _margin-impl.get()
    (impl.figure)(body, caption: caption, side: side, alt: alt)
  }
}
