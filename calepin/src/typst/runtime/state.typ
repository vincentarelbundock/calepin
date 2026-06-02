#let _mode = sys.inputs.at("calepin-mode", default: "render")
#let _auto-label-index = state("calepin-auto-label-index", 1)
#let _auto-inline-label-index = state("calepin-auto-inline-label-index", 1)

#let _base-options = (
  echo: true,
  eval: true,
  output: true,
  results: "verbatim",
  warning: true,
  message: true,
  error: false,
  format: auto,
  item: "all",
  placeholder: auto,
  "fig-device-format": "svg",
  "fig-device-dpi": 150,
  "fig-device-width": 6,
  "fig-device-height": auto,
  "fig-device-aspect": 0.618,
  "fig-display-width": 70%,
  "fig-display-height": auto,
  "fig-display-align": center,
  "fig-display-responsive": true,
  "fig-display-link": auto,
  "fig-caption": none,
  "fig-caption-position": auto,
  "fig-alt-text": none,
  "fig-subcaptions": none,
  "fig-layout-columns": auto,
  "fig-layout-rows": auto,
  "fig-layout-design": auto,
  kind: auto,
  "raw-chunks": false,
)
#let _setup-defaults = state("calepin-setup-defaults", (default: _base-options, langs: (:)))

#let _call-extra-defaults = (
  label: none,
  source-lang: auto,
  inline-output: false,
  auto-label-prefix: "chunk",
  auto-label-state: _auto-label-index,
)

#let _auto-call-defaults(defaults) = {
  let out = (:)
  for key in defaults.keys() {
    out.insert(key, auto)
  }
  out.insert("fig-display-link", none)
  out.insert("fig-caption", none)
  out.insert("fig-alt-text", none)
  out.insert("fig-subcaptions", none)
  out + _call-extra-defaults
}

#let _call-defaults = _auto-call-defaults(_base-options)

#let _disable-raw-chunk-transforms = state("calepin-disable-raw-chunk-transforms", false)

#let _raw-node(body) = {
  if body.has("text") {
    return body
  }
  if body.has("children") {
    let candidates = body.children.filter(child => child.has("text"))
    if candidates.len() == 1 {
      return candidates.at(0)
    }
  }
  panic("calepin chunks must contain exactly one raw code element")
}

#let _raw-text(body) = _raw-node(body).text

#let _sync-auto-label-counter(auto-label-state, label) = {
  if label.starts-with("chunk-") {
    let suffix = label.slice(6)
    let is-int = suffix.matches(regex("^[0-9]+$")) != ()
    if is-int {
      let next = int(suffix) + 1
      auto-label-state.update(n => if next > n { next } else { n })
    }
  }
}

#let _select-representation(data, format) = {
  if format != auto {
    return data.at(format, default: none)
  }
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

#let _resolve-asset-href(path) = {
  let base = sys.inputs.at("calepin-assets", default: "")
  if base != "" and path.starts-with("/") {
    base + path
  } else {
    path
  }
}

#let _html-target() = sys.inputs.at("calepin-target", default: "paged") == "html"

#let _attach-label(content, id) = [
  #content #label(id)
]
