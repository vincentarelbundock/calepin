#let _auto-label-index = state("calepin-auto-label-index", 1)
#let _auto-inline-label-index = state("calepin-auto-inline-label-index", 1)

// The option vocabulary is defined once, in Rust (`typst/option_table.rs`),
// and generated into this dictionary by `build.rs`. Do not hand-edit; add or
// remove options in `OPTION_TABLE` instead.
// calepin:generated-base-options

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
