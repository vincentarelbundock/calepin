#import "../../active.typ": config as active-config

#let _empty-config = (
  enabled: false,
  results: none,
  store: none,
  "image-meta": none,
  "source-dir": "",
  source: none,
  "raw-langs": ("python", "r", "mermaid", "dot", "tikz", "d2"),
)

// Notebook identity is an all-or-nothing bundle. Calepin always supplies a
// results input during its managed query/render passes, so its presence is the
// selector for the complete sys.inputs-backed configuration. External Typst
// frontends supply no such input and therefore fall back to active.typ.
#let _input-config() = (
  enabled: true,
  results: sys.inputs.at("calepin-results"),
  store: sys.inputs.at("calepin-store", default: none),
  "image-meta": sys.inputs.at("calepin-image-meta", default: none),
  "source-dir": sys.inputs.at("calepin-source-dir", default: ""),
  source: none,
  "raw-langs": _empty-config.at("raw-langs"),
)

#let _runtime-config(bound: none) = {
  if bound != none {
    bound
  } else if sys.inputs.at("calepin-results", default: none) != none {
    _input-config()
  } else if active-config.at("enabled", default: false) {
    active-config
  } else {
    _empty-config
  }
}
