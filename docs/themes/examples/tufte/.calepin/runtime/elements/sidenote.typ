#import "../core/target.typ": _is-html, _is-query
#import "margin.typ": _margin-impl

#let _sidenote-counter = counter("calepin-sidenote")

// A margin note. Numbered by default with an in-text marker; `numbering: none`
// produces an unnumbered margin note. In paged output the placement defers to
// the installed margin implementation (see `margin.typ`).
#let sidenote(body, numbering: auto, side: auto) = {
  if _is-query() {
    return body
  }

  if _is-html() {
    let note_class = if numbering == none {
      "calepin-sidenote calepin-sidenote-unnumbered"
    } else {
      "calepin-sidenote calepin-sidenote-numbered"
    }
    if numbering == none {
      return html.elem("span", attrs: (class: note_class))[#body]
    }
    return {
      _sidenote-counter.step()
      context {
        let n = _sidenote-counter.get().first()
        let id = "sn-" + str(n)
        html.elem(
          "a",
          attrs: (role: "doc-noteref", href: "#" + id, id: id + "ref"),
        )[#str(n)]
        html.elem(
          "span",
          attrs: (class: note_class, id: id, "data-sidenote-number": str(n)),
        )[#body]
      }
    }
  }

  context {
    let impl = _margin-impl.get()
    (impl.note)(body, numbering: numbering, side: side)
  }
}
