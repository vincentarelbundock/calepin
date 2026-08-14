#import "../core/target.typ": _is-html, _is-query

// Paged colors mirror the public CSS tokens in
// themes/shared/css/20_theme.css (--calepin-callout-*-color).
#let _kinds = (
  note: (label: "Note", color: rgb("#1c7ed6")),
  tip: (label: "Tip", color: rgb("#2e7d32")),
  warning: (label: "Warning", color: rgb("#b26a00")),
  caution: (label: "Caution", color: rgb("#b3261e")),
  important: (label: "Important", color: rgb("#7c3aed")),
)

#let _kind-config(kind) = {
  if type(kind) != str {
    panic("calepin.elements.callout: kind must be a string")
  }

  let normalized = lower(kind.trim())
  let config = _kinds.at(normalized, default: none)
  if config == none {
    panic("calepin.elements.callout: unknown kind `" + normalized + "`")
  }

  (kind: normalized, ..config)
}

#let callout(kind: "note", title: auto, body) = {
  let config = _kind-config(kind)
  let callout-title = if title == auto {
    config.label
  } else {
    title
  }

  if _is-query() {
    return body
  }

  if _is-html() {
    return html.elem(
      "div",
      attrs: (class: "calepin-callout calepin-callout-" + config.kind),
    )[
      #if callout-title != none {
        html.elem("div", attrs: (class: "calepin-callout-title"))[#callout-title]
      }
      #html.elem("div", attrs: (class: "calepin-callout-body"))[#body]
    ]
  }

  block(
    width: 100%,
    breakable: true,
    inset: (x: 0.9em, y: 0.75em),
    radius: 4pt,
    stroke: (
      left: 3pt + config.color,
      rest: 0.5pt + config.color.lighten(65%),
    ),
    fill: config.color.lighten(90%),
  )[
    #if callout-title != none {
      [
        #text(fill: config.color, weight: "bold")[#callout-title]
        #v(0.35em)
      ]
    }
    #body
  ]
}
