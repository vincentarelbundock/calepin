#import "../core/state.typ": *
#import "../core/target.typ": _is-query

// Validate that document parameters only contain JSON-serializable leaves
// (none, bool, int, float, str) nested in arrays/dictionaries. Anything else
// (content, functions, lengths, colors, ...) fails fast with the offending path.
#let _validate-params(value, path) = {
  let t = type(value)
  if value == none or t == bool or t == int or t == float or t == str {
    // supported scalar leaf
  } else if t == array {
    for (i, item) in value.enumerate() {
      _validate-params(item, path + "[" + str(i) + "]")
    }
  } else if t == dictionary {
    for (k, v) in value.pairs() {
      _validate-params(v, if path == "" { k } else { path + "." + k })
    }
  } else {
    panic(
      "calepin.setup: unsupported parameter `" + path + "`: values of type " + str(t)
        + " cannot be passed as parameters; use none, a boolean, a number, a string, "
        + "an array, or a dictionary",
    )
  }
}

// Per-option defaults come from `_base-options` so there is a single source of
// truth for all document-level configuration.
#let setup(
  echo: _base-options.at("echo"),
  eval: _base-options.at("eval"),
  results: _base-options.at("results"),
  warning: _base-options.at("warning"),
  message: _base-options.at("message"),
  error: _base-options.at("error"),
  placeholder: _base-options.at("placeholder"),
  fig-device-format: _base-options.at("fig-device-format"),
  fig-device-dpi: _base-options.at("fig-device-dpi"),
  fig-device-width: _base-options.at("fig-device-width"),
  fig-device-height: _base-options.at("fig-device-height"),
  fig-device-aspect: _base-options.at("fig-device-aspect"),
  fig-width: _base-options.at("fig-width"),
  fig-height: _base-options.at("fig-height"),
  fig-align: _base-options.at("fig-align"),
  fig-responsive: _base-options.at("fig-responsive"),
  fenced-chunks: true,
  fallback-warning: true,
  theme: none,
  params: (:),
  ) = {
  _validate-params(params, "")
  let setup-opts = (
    echo: echo,
    eval: eval,
    results: results,
    warning: warning,
    message: message,
    error: error,
    placeholder: placeholder,
    "fig-device-format": fig-device-format,
    "fig-device-dpi": fig-device-dpi,
    "fig-device-width": fig-device-width,
    "fig-device-height": fig-device-height,
    "fig-device-aspect": fig-device-aspect,
    "fig-width": fig-width,
    "fig-height": fig-height,
    "fig-align": fig-align,
    "fig-responsive": fig-responsive,
    "fenced-chunks": fenced-chunks,
    "fallback-warning": fallback-warning,
    theme: theme,
    params: params,
  )
  _setup-defaults.update(defaults => (default: defaults.at("default") + setup-opts))
  if _is-query() {
    [#metadata(setup-opts) <calepin-config>]
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
  let defaults = _setup-defaults.get().at("default")
  let out = (:)
  for key in _base-options.keys() {
    out.insert(key, _coalesce-auto(args.at(key), defaults.at(key)))
  }
  for key in _call-extra-defaults.keys() {
    out.insert(key, args.at(key))
  }
  out
}
