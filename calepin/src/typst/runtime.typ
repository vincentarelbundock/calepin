#let _mode = sys.inputs.at("calepin-mode", default: "render")
#let _auto-label-index = state("calepin-auto-label-index", 1)

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

#let _selected(data, format) = {
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

#let _attach-label(content, id) = [
  #content #label(id)
]

#let _render-rich(item, label, format, out-width, out-height, fig-cap, fig-alt, tbl-cap, kind) = {
  let data = item.at("data", default: (:))
  let selected = _selected(data, format)
  if selected == none {
    return none
  }
  let mime = selected.mime
  let value = selected.value
  if mime == "image/svg+xml" or mime == "image/png" {
    let img = image(_artifact-path(value), width: out-width, height: out-height, alt: if fig-alt == none { "" } else { fig-alt })
    if fig-cap != none {
      _attach-label(figure(img, caption: fig-cap), label)
    } else {
      img
    }
  } else if mime == "text/x-typst" {
    if tbl-cap != none or kind == "table" or label.starts-with("tbl-") {
      _attach-label(
        figure(kind: table, caption: tbl-cap)[
          #eval(value, mode: "markup")
        ],
        label,
      )
    } else {
      eval(value, mode: "markup")
    }
  } else if mime == "application/json" {
    raw(repr(value), block: true)
  } else {
    raw(str(value), block: true)
  }
}

#let _render-item(item, label, format, results-mode, warning, message, out-width, out-height, fig-cap, fig-alt, tbl-cap, kind) = {
  let item-type = item.at("type", default: "")
  if item-type == "stream" {
    let text = item.at("text", default: "")
    if results-mode == "hide" {
      none
    } else if results-mode == "asis" {
      eval(text, mode: "markup")
    } else {
      raw(text, block: true, lang: "text")
    }
  } else if item-type == "diagnostic" {
    let level = item.at("level", default: "")
    if (level == "warning" and warning != true) or (level == "message" and message != true) {
      none
    } else {
      raw(item.at("text", default: ""), block: true, lang: "text")
    }
  } else if item-type == "error" {
    raw(item.at("message", default: ""), block: true, lang: "text")
  } else if item-type == "display" or item-type == "result" {
    _render-rich(item, label, format, out-width, out-height, fig-cap, fig-alt, tbl-cap, kind)
  }
}

#let _render-results(label, format, item, results-mode, warning, message, out-width, out-height, fig-cap, fig-alt, tbl-cap, kind) = {
  let results-path = sys.inputs.at("calepin-results", default: "")
  if results-path == "" {
    return none
  }
  let results-doc = json(results-path)
  let chunk = results-doc.at("chunks", default: (:)).at(label, default: none)
  if chunk == none {
    panic("calepin results do not contain label `" + label + "`")
  }
  let items = chunk.at("items", default: ())
  if item == "first" {
    if items.len() > 0 {
      return _render-item(items.first(), label, format, results-mode, warning, message, out-width, out-height, fig-cap, fig-alt, tbl-cap, kind)
    }
    return none
  }
  if item == "last" {
    if items.len() > 0 {
      return _render-item(items.last(), label, format, results-mode, warning, message, out-width, out-height, fig-cap, fig-alt, tbl-cap, kind)
    }
    return none
  }
  if type(item) == int {
    let idx = if item < 0 { items.len() + item } else { item }
    if idx >= 0 and idx < items.len() {
      return _render-item(items.at(idx), label, format, results-mode, warning, message, out-width, out-height, fig-cap, fig-alt, tbl-cap, kind)
    }
    return none
  }
  for result-item in items {
    _render-item(result-item, label, format, results-mode, warning, message, out-width, out-height, fig-cap, fig-alt, tbl-cap, kind)
  }
}

#let setup(
  cache: true,
  echo: true,
  eval: true,
  include_: true,
  results: "verbatim",
  warning: true,
  message: true,
  error: false,
  format: auto,
  item: "all",
  placeholder: auto,
  dev: "svg",
  dpi: 150,
  fig-width: 6,
  fig-height: auto,
) = {
  if _mode == "query" {
    [#metadata((
      cache: cache,
      echo: echo,
      eval: eval,
      "include": include_,
      results: results,
      warning: warning,
      message: message,
      error: error,
      format: format,
      item: item,
      placeholder: placeholder,
      dev: dev,
      dpi: dpi,
      fig-width: fig-width,
      fig-height: fig-height,
    )) <calepin-config>]
  } else {
    none
  }
}

#let chunk(
  engine,
  body,
  label: none,
  cache: auto,
  echo: auto,
  eval: auto,
  include_: auto,
  results: auto,
  warning: auto,
  message: auto,
  error: auto,
  format: auto,
  item: auto,
  placeholder: auto,
  dev: auto,
  dpi: auto,
  fig-width: auto,
  fig-height: auto,
  out-width: auto,
  out-height: auto,
  fig-cap: none,
  fig-alt: none,
  tbl-cap: none,
  kind: auto,
) = context {
  let generated-label = label == none
  let label = if generated-label { "chunk-" + str(_auto-label-index.get()) } else { label }
  let label-step = if generated-label { _auto-label-index.update(n => n + 1) } else { none }
  let code = _raw-text(body)
  let code = if code.starts-with("\n") { code.slice(1) } else { code }
  if _mode == "query" {
    [#label-step #metadata((
      body: body,
      code: code,
      engine: engine,
      label: label,
      cache: cache,
      echo: echo,
      eval: eval,
      "include": include_,
      results: results,
      warning: warning,
      message: message,
      error: error,
      format: format,
      item: item,
      placeholder: placeholder,
      dev: dev,
      dpi: dpi,
      fig-width: fig-width,
      fig-height: fig-height,
      out-width: out-width,
      out-height: out-height,
      fig-cap: fig-cap,
      fig-alt: fig-alt,
      tbl-cap: tbl-cap,
      kind: kind,
    )) <calepin-chunk>]
  } else {
    let show-echo = echo == true or echo == auto
    let results-path = sys.inputs.at("calepin-results", default: "")

    label-step

    if include_ == false {
      none
    } else {
      if show-echo {
        raw(code, block: true, lang: engine)
      } else if results-path == "" {
        raw(code, block: true, lang: engine)
      }
      if results-path != "" {
        _render-results(
          label,
          format,
          if item == auto { "all" } else { item },
          if results == auto { "verbatim" } else { results },
          warning != false,
          message != false,
          out-width,
          out-height,
          fig-cap,
          fig-alt,
          tbl-cap,
          kind,
        )
      }
    }
  }
}
