
// Per-option defaults come from `_base-options` so there is a single source of
// truth; `setup` only adds the document-level options that have no per-chunk
// equivalent (`lang`).
#let setup(
  echo: _base-options.at("echo"),
  eval: _base-options.at("eval"),
  output: _base-options.at("output"),
  results: _base-options.at("results"),
  warning: _base-options.at("warning"),
  message: _base-options.at("message"),
  error: _base-options.at("error"),
  format: _base-options.at("format"),
  item: _base-options.at("item"),
  placeholder: _base-options.at("placeholder"),
  fig-device-format: _base-options.at("fig-device-format"),
  fig-device-dpi: _base-options.at("fig-device-dpi"),
  fig-device-width: _base-options.at("fig-device-width"),
  fig-device-height: _base-options.at("fig-device-height"),
  fig-device-aspect: _base-options.at("fig-device-aspect"),
  fig-display-width: _base-options.at("fig-display-width"),
  fig-display-height: _base-options.at("fig-display-height"),
  fig-display-align: _base-options.at("fig-display-align"),
  fig-display-responsive: _base-options.at("fig-display-responsive"),
  raw-chunks: true,
  lang: none,
  ) = {
  let setup-lang = if lang == none { none } else { if lang == "bash" { "sh" } else { lang } }
  let setup-opts = (
    echo: echo,
    eval: eval,
    output: output,
    results: results,
    warning: warning,
    message: message,
    error: error,
    format: format,
    item: item,
    placeholder: placeholder,
    "fig-device-format": fig-device-format,
    "fig-device-dpi": fig-device-dpi,
    "fig-device-width": fig-device-width,
    "fig-device-height": fig-device-height,
    "fig-device-aspect": fig-device-aspect,
    "fig-display-width": fig-display-width,
    "fig-display-height": fig-display-height,
    "fig-display-align": fig-display-align,
    "fig-display-responsive": fig-display-responsive,
    "raw-chunks": raw-chunks,
  )
  _setup-defaults.update(state => {
    let defaults = if lang == none { _base-options + setup-opts } else { state.at("default") }
    if setup-lang == none {
      (default: defaults, langs: state.at("langs"))
    } else {
      let langs = state.at("langs")
      langs.insert(setup-lang, _base-options + setup-opts)
      (default: defaults, langs: langs)
    }
  })
  if _mode == "query" {
    let setup-metadata = setup-opts + (
      "lang": setup-lang,
    )
    [#metadata(setup-metadata) <calepin-config>]
  }
}

#let _coalesce-auto(value, fallback) = {
  if value == auto {
    fallback
  } else {
    value
  }
}

#let _resolve-options(engine, args) = {
  let engine = if engine == none { none } else { if engine == "bash" { "sh" } else { engine } }
  let setup-defaults = _setup-defaults.get()
  let defaults = if engine == none {
    setup-defaults.at("default")
  } else {
    let langs = setup-defaults.at("langs")
    let selected = langs.at(engine, default: none)
    if selected == none {
      setup-defaults.at("default")
    } else {
      setup-defaults.at("default") + selected
    }
  }
  let out = (:)
  for key in _base-options.keys() {
    out.insert(key, _coalesce-auto(args.at(key), defaults.at(key)))
  }
  for key in _call-extra-defaults.keys() {
    out.insert(key, args.at(key))
  }
  out
}
