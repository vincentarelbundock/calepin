#import "../core/css.typ": _css-size
#import "../core/target.typ": _is-html, _is-query
#import "target.typ": target

#let _base-style = "display: block; margin-block: 1rem; padding: 1rem; overflow: hidden; border: 1px solid var(--pico-muted-border-color); border-radius: 0.8rem; background: var(--pico-background-color); color: var(--pico-color); box-shadow: 0 0.65rem 1.5rem rgba(15, 23, 42, 0.08);"

#let card(
  html: none,
  paged: none,
  class: none,
  style: none,
  width: 100%,
  body,
) = {
  let content = target(html: html, paged: paged, fallback: body)

  if _is-query() {
    return content
  }

  if _is-html() {
    let css-width = _css-size(width)
    let class-attrs = "calepin-elements-card" + if class != none and class != "" { " " + class } else { "" }
    let attrs-style = {
      let out = _base-style
      if style != none {
        if style.ends-with(";") {
          out + " " + style
        } else {
          out + " " + style + ";"
        }
      } else {
        out
      }
    }
    let style-with-width = {
      let width-style = if css-width != none {
        " max-width: " + css-width + ";"
      } else {
        ""
      }
      attrs-style + width-style
    }
    let attrs = (
      class: class-attrs,
      style: style-with-width,
    )
    return std.html.elem("article", attrs: attrs)[#content]
  }

  return block(
    width: width,
    inset: 0.85em,
    radius: 6pt,
    stroke: 0.5pt + luma(75%),
    fill: luma(98%),
  )[#content]
}
