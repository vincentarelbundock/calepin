#let _css-size(value) = {
  if value == none or value == auto {
    none
  } else if type(value) == str {
    value
  } else {
    repr(value)
  }
}

#let _css-decl(property, value) = {
  let size = _css-size(value)
  if size == none or size == "" {
    ""
  } else {
    property + ": " + size + ";"
  }
}

#let _append-css(base, next) = {
  if next == "" {
    base
  } else if base == "" {
    next
  } else {
    base + " " + next
  }
}
