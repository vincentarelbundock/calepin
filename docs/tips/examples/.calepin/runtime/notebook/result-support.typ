#let _select-representation(data) = {
  for mime in ("image/svg+xml", "image/png", "text/x-typst", "text/plain", "application/json") {
    let value = data.at(mime, default: none)
    if value != none {
      return (mime: mime, value: value)
    }
  }
  none
}

#let _artifact-path(value) = {
  if type(value) == dictionary {
    value.at("path")
  } else {
    value
  }
}

#let _attach-label(content, id) = [
  #content #label(id)
]

#let _attach-labels(content, ids) = {
  let out = content
  for id in ids {
    out = [#out #label(id)]
  }
  out
}

#let _crossref-labels-for(chunk, kind) = {
  let labels = ()
  for entry in chunk.at("crossref-labels", default: ()) {
    if entry.at("kind", default: "") == kind {
      labels.push(entry.at("name"))
    }
  }
  labels
}
