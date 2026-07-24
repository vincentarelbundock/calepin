#import "config.typ": _runtime-config

// Load the results contract through one helper so callers share the same
// defaults and schema navigation. Typst caches file reads during compilation.
#let _results-document(config: none) = {
  let results-path = _runtime-config(bound: config).at("results", default: none)
  if results-path == none or results-path == "" {
    none
  } else {
    let document = json(results-path)
    let schema = document.at("schema", default: 1)
    if schema not in (1, 2) {
      panic("unsupported Calepin results schema " + str(schema) + "; this runtime supports schemas 1 and 2")
    }
    document
  }
}

#let _result-chunk(document, label, default: none) = {
  if document == none {
    default
  } else {
    document.at("chunks", default: (:)).at(label, default: default)
  }
}
