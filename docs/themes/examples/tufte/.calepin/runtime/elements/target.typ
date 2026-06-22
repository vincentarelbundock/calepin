#import "../core/target.typ": _is-html

#let _realize(value) = {
  if value == none {
    none
  } else if type(value) == function {
    value()
  } else {
    value
  }
}

#let target(html: none, paged: none, fallback: none) = {
  let selected = if _is-html() { html } else { paged }
  let content = _realize(selected)
  if content == none {
    _realize(fallback)
  } else {
    content
  }
}
