#import "../core/target.typ": _is-query
#import "defaults.typ": _base-options, _call-extra-defaults, _setup-defaults

// Validate that document variables only contain JSON-serializable leaves
// (none, bool, int, float, str) nested in arrays/dictionaries. Anything else
// (content, functions, lengths, colors, ...) fails fast with the offending path.
#let _validate-vars(value, path) = {
  let t = type(value)
  if value == none or t == bool or t == int or t == float or t == str {
    // supported scalar leaf
  } else if t == array {
    for (i, item) in value.enumerate() {
      _validate-vars(item, path + "[" + str(i) + "]")
    }
  } else if t == dictionary {
    for (k, v) in value.pairs() {
      _validate-vars(v, if path == "" { k } else { path + "." + k })
    }
  } else {
    panic(
      "calepin.setup: unsupported variable `" + path + "`: values of type " + str(t)
        + " cannot be passed as variables; use none, a boolean, a number, a string, "
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
  fig-link: _base-options.at("fig-link"),
  fig-caption: _base-options.at("fig-caption"),
  fig-cap-location: _base-options.at("fig-cap-location"),
  fig-alt-text: _base-options.at("fig-alt-text"),
  fig-subcaptions: _base-options.at("fig-subcaptions"),
  fig-layout-columns: _base-options.at("fig-layout-columns"),
  fig-layout-rows: _base-options.at("fig-layout-rows"),
  kind: _base-options.at("kind"),
  fenced-chunks: true,
  fallback-warning: true,
  theme: none,
  vars: (:),
  ) = {
  _validate-vars(vars, "")
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
    "fig-link": fig-link,
    "fig-caption": fig-caption,
    "fig-cap-location": fig-cap-location,
    "fig-alt-text": fig-alt-text,
    "fig-subcaptions": fig-subcaptions,
    "fig-layout-columns": fig-layout-columns,
    "fig-layout-rows": fig-layout-rows,
    kind: kind,
    "fenced-chunks": fenced-chunks,
    "fallback-warning": fallback-warning,
    theme: theme,
    vars: vars,
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

#let _resolve-options-for(args) = {
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

// Keep the engine argument for themes that import the historical helper.
#let _resolve-options(engine, args) = _resolve-options-for(args)
