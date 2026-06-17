#import "../core/target.typ": _is-html, _is-query

#let columns(
  html-tag: "div",
  html-class: "grid",
  columns: 2,
  gutter: 1em,
  html-item-tag: "div",
  html-item-attrs: ( : ),
  html-attrs: ( : ),
  ..blocks,
) = {
  let _css-track(value) = {
    if type(value) == int {
      str(value) + "fr"
    } else if type(value) == str {
      value
    } else {
      repr(value)
    }
  }

  let column-css = {
    if type(columns) == int {
      "repeat(" + str(columns) + ", 1fr)"
    } else if type(columns) == array {
      let tracks = ()
      for column in columns {
        tracks.push(_css-track(column))
      }
      tracks.join(" ")
    } else {
      "repeat(2, minmax(0, 1fr))"
    }
  }

  let gutter-style = if gutter == none {
    "1em"
  } else if type(gutter) == str {
    gutter
  } else {
    repr(gutter)
  }

  let style-base = "display: grid; grid-template-columns: " + column-css + "; gap: " + gutter-style + ";"
  let extra-style = html-attrs.at("style", default: "")
  let classes = html-attrs.at("class", default: "")
  let attrs-style = if extra-style == "" {
    style-base
  } else {
    style-base + " " + extra-style
  }
  let attrs-class = if classes == "" {
    html-class
  } else {
    html-class + " " + classes
  }

  let items = blocks.pos()
  if _is-query() {
    return items
  }

  if _is-html() {
    let attrs = (
      ..html-attrs,
      class: attrs-class,
      style: attrs-style,
    )
    let item-attrs = html-item-attrs
    return std.html.elem(html-tag, attrs: attrs)[
      #for item in items {
        std.html.elem(html-item-tag, attrs: item-attrs)[
          #item
        ]
      }
    ]
  }

  grid(
    columns: columns,
    gutter: gutter,
    ..items,
  )
}
