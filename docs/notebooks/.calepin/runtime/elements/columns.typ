#import "../core/target.typ": _is-html, _is-query

#let columns(
  html-tag: "div",
  html-class: "grid",
  columns: 2,
  wrap: true,
  html-attrs: ( : ),
  ..blocks,
) = {
  if type(columns) != int or columns < 1 {
    panic("calepin.elements.columns: columns must be a positive integer")
  }

  let classes = html-attrs.at("class", default: "")
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
    )
    return std.html.elem(html-tag, attrs: attrs)[
      #for item in items {
        if wrap {
          std.html.elem("div")[#item]
        } else {
          item
        }
      }
    ]
  }

  grid(
    columns: columns,
    gutter: 1em,
    ..items,
  )
}
