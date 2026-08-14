#let _auto-label-index = state("calepin-auto-label-index", 1)
#let _auto-inline-label-index = state("calepin-auto-inline-label-index", 1)

#let _base-options = (
  script: true,
  echo: true,
  eval: true,
  results: "render",
  warning: true,
  message: true,
  error: false,
  placeholder: auto,
  "fig-device-format": "svg",
  "fig-device-dpi": 150,
  "fig-device-width": 6,
  "fig-device-height": auto,
  "fig-device-aspect": 0.618,
  "fig-width": 70%,
  "fig-height": auto,
  "fig-align": center,
  "fig-responsive": true,
  "fig-link": auto,
  "fig-caption": none,
  "fig-cap-location": auto,
  "fig-alt-text": none,
  "fig-subcaptions": none,
  "fig-layout-columns": auto,
  "fig-layout-rows": auto,
  kind: auto,
  "fenced-chunks": true,
)

#let _setup-defaults = state("calepin-setup-defaults", (default: _base-options))

#let _call-extra-defaults = (
  label: none,
  inline-output: false,
  store-get: none,
  store-set: none,
  auto-label-prefix: "chunk",
  auto-label-state: _auto-label-index,
)

#let _auto-call-defaults(defaults) = {
  let out = (:)
  for key in defaults.keys() {
    out.insert(key, auto)
  }
  out + _call-extra-defaults
}

#let _call-defaults = _auto-call-defaults(_base-options)
