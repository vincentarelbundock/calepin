#import "../00_syntax-theme.typ": *
#import "../core/assets.typ": _resolve-asset-href
#import "../core/css.typ": *
#import "../core/state.typ": _artifact-path, _attach-label, _attach-labels
#import "../core/state.typ": _crossref-labels-for, _select-representation
#import "../core/target.typ": _is-html

#let _figure-caption(fig-caption, fig-cap-location) = {
  if fig-caption == none {
    none
  } else if fig-cap-location == auto or fig-cap-location == none {
    fig-caption
  } else {
    figure.caption(position: fig-cap-location)[#fig-caption]
  }
}

#let _normalize-display-align(fig-align) = {
  if fig-align == "left" {
    left
  } else if fig-align == "start" {
    start
  } else if fig-align == "right" {
    right
  } else if fig-align == "end" {
    end
  } else if fig-align == "center" {
    center
  } else {
    fig-align
  }
}

#let _html-image-align-style(fig-align) = {
  let fig-align = _normalize-display-align(fig-align)
  if fig-align == left or fig-align == start {
    "margin-inline: 0 auto;"
  } else if fig-align == right or fig-align == end {
    "margin-inline: auto 0;"
  } else {
    "margin-inline: auto;"
  }
}

#let _html-block-align-style(fig-align) = {
  let fig-align = _normalize-display-align(fig-align)
  if fig-align == left or fig-align == start {
    "text-align: left;"
  } else if fig-align == right or fig-align == end {
    "text-align: right;"
  } else if fig-align == center {
    "text-align: center;"
  } else {
    ""
  }
}

#let _html-image-style(width, height, responsive, fig-align) = {
  let base = _append-css("display: block;", _html-image-align-style(fig-align))
  let with-width = _append-css(base, _css-decl("width", width))
  let with-height = _append-css(with-width, _css-decl("height", height))
  if responsive == true {
    _append-css(with-height, "max-width: 100%;")
  } else {
    with-height
  }
}

#let _html-image(path, width, height, responsive, fig-align, alt) = {
  let style = _html-image-style(width, height, responsive, fig-align)
  if style == "" {
    std.html.elem("img", attrs: (src: path, alt: alt))
  } else {
    std.html.elem("img", attrs: (src: path, alt: alt, class: "calepin-figure-width", style: style))
  }
}

#let _html-captioned-image(path, height, alt) = {
  let style = _append-css(_append-css("display: block;", "width: 100%;"), _css-decl("height", height))
  std.html.elem("img", attrs: (src: path, alt: alt, style: style))
}

#let _html-figure-style(width, responsive, fig-align) = {
  let with-width = _css-decl("width", width)
  let with-responsive = if responsive == true {
    _append-css(with-width, "max-width: 100%;")
  } else {
    with-width
  }
  _append-css(with-responsive, _html-image-align-style(fig-align))
}

#let _html-figure-width-attrs(style) = {
  let attrs = (class: "calepin-figure-width")
  attrs.insert("style", style)
  attrs
}

// A labeled figure must stay a native `figure` so `@label` cross-references
// resolve, and a native figure cannot carry the display-width style itself.
// Wrap it in a styled block that applies the same width/responsive/alignment as
// an unlabeled captioned figure, so both honor `fig-width`.
#let _wrap-html-figure-width(content, width, responsive, fig-align) = {
  let style = _html-figure-style(width, responsive, fig-align)
  if style == "" {
    content
  } else {
    std.html.elem("div", attrs: _html-figure-width-attrs(style))[#content]
  }
}

#let _html-captioned-figure(
  img,
  width,
  responsive,
  fig-align,
  fig-caption,
  fig-cap-location,
) = {
  let style = _html-figure-style(width, responsive, fig-align)
  let attrs = if style == "" { (:) } else { _html-figure-width-attrs(style) }
  let caption = std.html.elem("figcaption")[#context [Figure #counter(figure).display(): #fig-caption]]
  let content = if fig-cap-location == top {
    [#caption #img]
  } else {
    [#img #caption]
  }
  [
    #counter(figure).step()
    #std.html.elem("figure", attrs: attrs)[#content]
  ]
}

#let _finalize-figure-display(content, fig-align, fig-link) = {
  let fig-align = _normalize-display-align(fig-align)
  let linked = if fig-link == none or fig-link == auto {
    content
  } else {
    link(fig-link)[#content]
  }
  if _is-html() {
    let style = _html-block-align-style(fig-align)
    if style == "" {
      return linked
    }
    return std.html.elem("div", attrs: (style: style))[#linked]
  }
  if fig-align == none or fig-align == auto {
    linked
  } else {
    align(fig-align)[#linked]
  }
}

#let _paged-layout-size(value) = {
  if type(value) != str {
    return value
  }
  let value = value.trim()
  if value.ends-with("%") {
    return float(value.slice(0, value.len() - 1).trim()) * 1%
  }
  if value.ends-with("pt") {
    return float(value.slice(0, value.len() - 2).trim()) * 1pt
  }
  if value.ends-with("em") {
    return float(value.slice(0, value.len() - 2).trim()) * 1em
  }
  if value.ends-with("cm") {
    return float(value.slice(0, value.len() - 2).trim()) * 1cm
  }
  if value.ends-with("mm") {
    return float(value.slice(0, value.len() - 2).trim()) * 1mm
  }
  if value.ends-with("in") {
    return float(value.slice(0, value.len() - 2).trim()) * 1in
  }
  value
}

#let _paged-result-options(options) = {
  let out = (:)
  if "fig-width" in options and options.at("fig-width") != none {
    out.insert("fig-width", _paged-layout-size(options.at("fig-width")))
  }
  if "fig-height" in options and options.at("fig-height") != none {
    out.insert("fig-height", _paged-layout-size(options.at("fig-height")))
  }
  if "fig-align" in options and options.at("fig-align") != none {
    out.insert("fig-align", options.at("fig-align"))
  }
  if "code-size" in options and options.at("code-size") != none {
    out.insert("code-size", options.at("code-size"))
  }
  out
}

#let _merge-result-options(opts, chunk) = {
  let options = chunk.at("options", default: (:))
  if _is-html() {
    opts + options
  } else {
    opts + _paged-result-options(options)
  }
}

#let _block-lang-label(lang) = {
  if lang == none {
    ""
  } else if lang == "r" {
    "R"
  } else {
    lang
  }
}

#let _results-hidden(mode) = mode in ("hide", "hidden")

#let _raw-block(value, lang: none, theme: auto, code-size: none) = {
  let raw-cell = raw(value, block: true, lang: lang, theme: theme)
  if code-size == none {
    raw-cell
  } else {
    text(size: code-size)[#raw-cell]
  }
}

#let code-block(
  body,
  fill: rgb("#f7f7f5"),
  stroke: 0.5pt + rgb("#d8d8d2"),
  radius: 2pt,
  inset: (x: 0.65em, y: 0.45em),
  text-fill: rgb("#1f2933"),
  plain: false,
) = {
  let content = if plain {
    body
  } else {
    text(fill: text-fill)[#body]
  }
  block(
    width: 100%,
    fill: fill,
    stroke: stroke,
    radius: radius,
    inset: inset,
  )[
    #content
  ]
}

#let _paged-input-code-block(code, lang: none, code-size: none) = {
  code-block(
    fill: rgb("#f7f7f5"),
    stroke: 0.5pt + rgb("#d8d8d2"),
    radius: 2pt,
    inset: (x: 0.65em, y: 0.45em),
  )[
    #_raw-block(code, lang: lang, theme: _paged-syntax-theme, code-size: code-size)
  ]
}

#let _html-themed-raw-block(it) = {
  let lang = if it.has("lang") { it.lang } else { none }
  if _is-html() {
    _raw-block(it.text, lang: lang, theme: _input-syntax-theme)
  } else {
    _paged-input-code-block(it.text, lang: lang)
  }
}

#let _input-block(code, lang: none, code-size: none) = {
  if _is-html() {
    std.html.elem("div", attrs: (
      class: "sourceCode",
      "data-lang": _block-lang-label(lang),
    ))[
      #_raw-block(code, lang: lang, theme: _input-syntax-theme, code-size: code-size)
    ]
  } else {
    _paged-input-code-block(code, lang: lang, code-size: code-size)
  }
}

#let _output-block(output, stream: "stdout", code-size: none) = {
  if _is-html() {
    let class = if stream == "stderr" {
      "cell-output cell-output-stderr"
    } else {
      "cell-output cell-output-stdout"
    }
    std.html.elem("div", attrs: (class: class))[
      #_raw-block(output, theme: _output-syntax-theme, code-size: code-size)
    ]
  } else {
    let fill = if stream == "stderr" {
      rgb("#fffaf7")
    } else {
      rgb("#fbfbfa")
    }
    let stroke = if stream == "stderr" {
      (
        rest: 0.5pt + rgb("#e2c7ba"),
        left: 1.5pt + rgb("#c48672"),
      )
    } else {
      (
        rest: 0.5pt + rgb("#ddddda"),
        left: 1.5pt + rgb("#cfcfc8"),
      )
    }
    code-block(
      fill: fill,
      stroke: stroke,
      radius: 2pt,
      inset: (x: 0.65em, y: 0.4em),
      plain: true,
    )[
      #if stream == "stderr" {
        text(fill: rgb("#5f3328"))[
          #_raw-block(output, theme: _paged-syntax-theme, code-size: code-size)
        ]
      } else {
        _raw-block(output, theme: _paged-syntax-theme, code-size: code-size)
      }
    ]
  }
}

#let _display-selection(item, opts) = {
  let data = item.at("data", default: (:))
  _select-representation(data)
}

#let _is-image-mime(mime) = mime == "image/svg+xml" or mime == "image/png"

#let _is-image-display-item(item, opts) = {
  let item-type = item.at("type", default: "")
  if item-type != "display" and item-type != "result" {
    return false
  }
  let selected = _display-selection(item, opts)
  selected != none and _is-image-mime(selected.mime)
}

#let _fr-tracks(count) = {
  let tracks = ()
  for _ in range(count) {
    tracks.push(1fr)
  }
  tracks
}

#let _track-list(value) = {
  if value == auto or value == none {
    auto
  } else if type(value) == int {
    _fr-tracks(value)
  } else {
    value
  }
}

#let _auto-grid-columns(count, fig-layout-rows) = {
  if type(fig-layout-rows) == int and fig-layout-rows > 0 {
    return _fr-tracks(calc.ceil(count / fig-layout-rows))
  }
  if count <= 1 {
    (1fr,)
  } else if count <= 4 {
    (1fr, 1fr)
  } else {
    (1fr, 1fr, 1fr)
  }
}

#let _grid-columns(count, fig-layout-columns, fig-layout-rows) = {
  let columns = _track-list(fig-layout-columns)
  if columns == auto {
    _auto-grid-columns(count, fig-layout-rows)
  } else {
    columns
  }
}

#let _css-track(value) = {
  if value == auto {
    "auto"
  } else if type(value) == str {
    value
  } else {
    repr(value)
  }
}

#let _css-track-template(value) = {
  if value == auto or value == none {
    none
  } else if type(value) == array {
    let tracks = ()
    for track in value {
      tracks.push(_css-track(track))
    }
    tracks.join(" ")
  } else {
    _css-track(value)
  }
}

#let _html-grid-style(columns, rows) = {
  let style = "display: grid; gap: 1em;"
  let column-template = _css-track-template(columns)
  if column-template != none {
    style = _append-css(style, "grid-template-columns: " + column-template + ";")
  }
  let row-template = _css-track-template(rows)
  if row-template != none {
    style = _append-css(style, "grid-template-rows: " + row-template + ";")
  }
  style
}

#let _html-grid-content(columns, rows, cells) = {
  let body = []
  for cell in cells {
    body += cell
  }
  std.html.elem("div", attrs: (
    class: "calepin-figure-grid",
    style: _html-grid-style(columns, rows),
  ))[#body]
}

#let _grid-content(columns, rows, cells) = {
  let rows = _track-list(rows)
  if _is-html() {
    _html-grid-content(columns, rows, cells)
  } else if rows == auto {
    grid(columns: columns, gutter: 1em, ..cells)
  } else {
    grid(columns: columns, rows: rows, gutter: 1em, ..cells)
  }
}

#let _caption-for-index(captions, index) = {
  if captions == none or captions == auto {
    none
  } else if type(captions) == array and index < captions.len() {
    captions.at(index)
  } else {
    none
  }
}

#let _grid-image(item, opts) = {
  let selected = _display-selection(item, opts)
  let value = selected.value
  let artifact-path = _artifact-path(value)
  let html-path = _resolve-asset-href(artifact-path)
  let fig-height = opts.at("fig-height")
  let fig-responsive = opts.at("fig-responsive")
  let fig-alt-text = opts.at("fig-alt-text")
  let alt = if fig-alt-text == none { "" } else { fig-alt-text }
  if _is-html() {
    _html-image(html-path, 100%, fig-height, fig-responsive, center, alt)
  } else {
    image(artifact-path, width: 100%, height: fig-height, alt: alt)
  }
}

#let _grid-cell(content, caption) = {
  if _is-html() and caption != none {
    std.html.elem("div", attrs: (style: "min-width: 0;"))[
      #content
      #std.html.elem("div", attrs: (style: "font-size: 0.85em; margin-top: 0.35em;"))[#caption]
    ]
  } else if _is-html() {
    std.html.elem("div", attrs: (style: "min-width: 0;"))[#content]
  } else if caption == none {
    content
  } else {
    stack(spacing: 0.35em, content, text(size: 0.85em)[#caption])
  }
}

#let _wrap-grid-display(content, width, responsive, align) = {
  if _is-html() {
    let style = _html-figure-style(width, responsive, align)
    if style == "" {
      std.html.elem("div")[#content]
    } else {
      std.html.elem("div", attrs: (style: style))[#content]
    }
  } else if width == none or width == auto {
    content
  } else {
    block(width: width)[#content]
  }
}

#let _render-image-grid(items, label, opts, fig-labels, anchor: true) = {
  let fig-width = opts.at("fig-width")
  let fig-align = opts.at("fig-align")
  let fig-responsive = opts.at("fig-responsive")
  let fig-link = opts.at("fig-link")
  let fig-caption = opts.at("fig-caption")
  let fig-cap-location = opts.at("fig-cap-location")
  let fig-subcaptions = opts.at("fig-subcaptions")
  let fig-layout-columns = opts.at("fig-layout-columns")
  let fig-layout-rows = opts.at("fig-layout-rows")

  let cells = ()
  for (index, item) in items.enumerate() {
    cells.push(_grid-cell(_grid-image(item, opts), _caption-for-index(fig-subcaptions, index)))
  }

  let columns = _grid-columns(items.len(), fig-layout-columns, fig-layout-rows)
  let content = _wrap-grid-display(
    _grid-content(columns, fig-layout-rows, cells),
    fig-width,
    fig-responsive,
    fig-align,
  )
  let rendered = if fig-caption != none or fig-labels.len() > 0 {
    let fig = figure(content, caption: _figure-caption(fig-caption, fig-cap-location))
    if fig-labels.len() > 0 {
      _attach-labels(fig, fig-labels)
    } else if anchor {
      _attach-label(fig, label)
    } else {
      fig
    }
  } else {
    content
  }
  _finalize-figure-display(rendered, fig-align, fig-link)
}

#let _render-display-item(item, label, opts, fig-labels, anchor: true) = {
  let fig-width = opts.at("fig-width")
  let fig-height = opts.at("fig-height")
  let fig-align = opts.at("fig-align")
  let fig-responsive = opts.at("fig-responsive")
  let fig-link = opts.at("fig-link")
  let fig-caption = opts.at("fig-caption")
  let fig-cap-location = opts.at("fig-cap-location")
  let fig-alt-text = opts.at("fig-alt-text")
  let selected = _display-selection(item, opts)
  if selected == none {
    return none
  }
  let mime = selected.mime
  let value = selected.value
  if _is-image-mime(mime) {
    let artifact-path = _artifact-path(value)
    let html-path = _resolve-asset-href(artifact-path)
    let display-width = if fig-width == auto and fig-responsive == true { 100% } else { fig-width }
    let alt = if fig-alt-text == none { "" } else { fig-alt-text }
    if _is-html() and fig-caption != none {
      let img = _html-captioned-image(html-path, fig-height, alt)
      let fig = if fig-labels.len() > 0 {
        figure(img, caption: _figure-caption(fig-caption, fig-cap-location))
      } else {
        _html-captioned-figure(img, display-width, fig-responsive, fig-align, fig-caption, fig-cap-location)
      }
      let rendered = if fig-labels.len() > 0 {
        _wrap-html-figure-width(
          _attach-labels(fig, fig-labels),
          display-width,
          fig-responsive,
          fig-align,
        )
      } else {
        if anchor { _attach-label(fig, label) } else { fig }
      }
      return _finalize-figure-display(rendered, none, fig-link)
    }
    let img = if _is-html() {
      _html-image(html-path, display-width, fig-height, fig-responsive, fig-align, alt)
    } else {
      image(
        artifact-path,
        width: display-width,
        height: fig-height,
        alt: alt,
      )
    }
    let rendered = if fig-caption != none or fig-labels.len() > 0 {
      let fig = figure(img, caption: _figure-caption(fig-caption, fig-cap-location))
      if fig-labels.len() > 0 {
        _attach-labels(fig, fig-labels)
      } else {
        if anchor { _attach-label(fig, label) } else { fig }
      }
    } else {
      img
    }
    _finalize-figure-display(rendered, fig-align, fig-link)
  } else if mime == "text/x-typst" {
    if type(value) == dictionary and value.at("path", default: none) != none {
      eval(read(_artifact-path(value), encoding: "utf8"), mode: "markup")
    } else {
      eval(value, mode: "markup")
    }
  } else if mime == "application/json" {
    _output-block(repr(value))
  } else {
    _output-block(str(value))
  }
}

#let _render-item(item, label, opts, fig-labels, anchor: true) = {
  let results-mode = opts.at("results")
  let inline-output = opts.at("inline-output")
  let warning = opts.at("warning")
  let message = opts.at("message")

  let item-type = item.at("type", default: "")
  if item-type == "stream" {
    let text = item.at("text", default: "")
    if _results-hidden(results-mode) {
      none
    } else if results-mode == "typst" {
      eval(text, mode: "markup")
    } else if inline-output {
      text
    } else {
      _output-block(text, code-size: opts.at("code-size"))
    }
  } else if item-type == "diagnostic" {
    let level = item.at("level", default: "")
    if (level == "warning" and warning != true) or (level == "message" and message != true) {
      none
    } else {
      _output-block(
        item.at("text", default: ""),
        stream: if level == "warning" { "stderr" } else { "stdout" },
        code-size: opts.at("code-size"),
      )
    }
  } else if item-type == "error" {
    _output-block(item.at("message", default: ""), stream: "stderr", code-size: opts.at("code-size"))
  } else if item-type == "display" or item-type == "result" {
    _render-display-item(item, label, opts, fig-labels, anchor: anchor)
  }
}

// `anchor` controls whether cross-reference labels (and the chunk's internal-id
// label) are attached. The inline render owns the anchor; a relocated copy that
// does not own it passes `anchor: false` so the same output can appear more than
// once without defining a Typst label twice.
#let _render-results(label, opts, anchor: true) = {
  let results-path = sys.inputs.at("calepin-results", default: "")
  if results-path == "" {
    return none
  }
  let results-doc = json(results-path)
  let chunk = results-doc.at("chunks", default: (:)).at(label, default: none)
  if chunk == none {
    panic("calepin results do not contain label `" + label + "`")
  }
  let opts = _merge-result-options(opts, chunk)
  // `results: "hide"` only suppresses a chunk's own inline render. Reaching
  // `_render-results` at all (e.g. through a `#calepin.results` relocation)
  // means the output should be shown here.
  if _results-hidden(opts.at("results", default: "render")) {
    opts.insert("results", "render")
  }
  let fig-labels = if anchor { _crossref-labels-for(chunk, "fig") } else { () }
  let items = chunk.at("items", default: ())
  let image-group = ()
  for result-item in items {
    if _is-image-display-item(result-item, opts) {
      image-group.push(result-item)
    } else {
      if image-group.len() > 0 {
        if image-group.len() == 1 {
          _render-item(image-group.first(), label, opts, fig-labels, anchor: anchor)
        } else {
          _render-image-grid(image-group, label, opts, fig-labels, anchor: anchor)
        }
        image-group = ()
      }
      _render-item(result-item, label, opts, fig-labels, anchor: anchor)
    }
  }
  if image-group.len() > 0 {
    if image-group.len() == 1 {
      _render-item(image-group.first(), label, opts, fig-labels, anchor: anchor)
    } else {
      _render-image-grid(image-group, label, opts, fig-labels, anchor: anchor)
    }
  }
}
