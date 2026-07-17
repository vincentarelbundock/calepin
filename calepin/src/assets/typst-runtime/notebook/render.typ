#import "../core/assets.typ": _resolve-asset-href
#import "../core/config.typ": _runtime-config
#import "../core/results.typ": _result-chunk, _results-document
#import "../core/css.typ": _append-css, _css-decl
#import "../core/target.typ": _is-html
#import "code.typ" as codemod
#import "result-support.typ": _artifact-path, _attach-label, _attach-labels
#import "result-support.typ": _crossref-labels-for, _select-representation

#let code-block = codemod.code-block
#let _html-themed-raw-block = codemod._html-themed-raw-block
#let _input-block = codemod._input-block
#let _output-block = codemod._output-block

#let _figure-caption(fig-caption, fig-cap-location) = {
  if fig-caption == none {
    none
  } else if fig-cap-location == auto or fig-cap-location == none {
    fig-caption
  } else {
    figure.caption(position: fig-cap-location)[#fig-caption]
  }
}

#let _label-figure(content, label, fig-labels, anchor) = {
  if fig-labels.len() > 0 {
    _attach-labels(content, fig-labels)
  } else if anchor {
    _attach-label(content, label)
  } else {
    content
  }
}

#let _figure-or-content(content, label, fig-labels, caption, caption-location, anchor) = {
  if caption == none and fig-labels.len() == 0 {
    content
  } else {
    _label-figure(
      figure(content, caption: _figure-caption(caption, caption-location)),
      label,
      fig-labels,
      anchor,
    )
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

#let _results-hidden(mode) = mode in ("hide", "hidden")

#let _figure-options(opts) = (
  width: opts.at("fig-width"),
  height: opts.at("fig-height"),
  align: opts.at("fig-align"),
  responsive: opts.at("fig-responsive"),
  link: opts.at("fig-link"),
  caption: opts.at("fig-caption"),
  "caption-location": opts.at("fig-cap-location"),
  alt: opts.at("fig-alt-text"),
  subcaptions: opts.at("fig-subcaptions"),
  columns: opts.at("fig-layout-columns"),
  rows: opts.at("fig-layout-rows"),
)

#let _finalize-figure-content(content, label, fig-labels, figure-opts, anchor) = {
  let rendered = _figure-or-content(
    content,
    label,
    fig-labels,
    figure-opts.caption,
    figure-opts.at("caption-location"),
    anchor,
  )
  _finalize-figure-display(rendered, figure-opts.align, figure-opts.link)
}

#let _display-selection(item) = {
  let data = item.at("data", default: (:))
  _select-representation(data)
}

#let _is-image-mime(mime) = mime == "image/svg+xml" or mime == "image/png"

#let _is-image-display-item(item) = {
  let item-type = item.at("type", default: "")
  if item-type != "display" and item-type != "result" {
    return false
  }
  let selected = _display-selection(item)
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

#let _grid-image(item, figure-opts) = {
  let selected = _display-selection(item)
  let value = selected.value
  let artifact-path = _artifact-path(value)
  let html-path = _resolve-asset-href(artifact-path)
  let alt = if figure-opts.alt == none { "" } else { figure-opts.alt }
  if _is-html() {
    _html-image(html-path, 100%, figure-opts.height, figure-opts.responsive, center, alt)
  } else {
    image(artifact-path, width: 100%, height: figure-opts.height, alt: alt)
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
  let figure-opts = _figure-options(opts)

  let cells = ()
  for (index, item) in items.enumerate() {
    cells.push(
      _grid-cell(
        _grid-image(item, figure-opts),
        _caption-for-index(figure-opts.subcaptions, index),
      ),
    )
  }

  let columns = _grid-columns(items.len(), figure-opts.columns, figure-opts.rows)
  let content = _wrap-grid-display(
    _grid-content(columns, figure-opts.rows, cells),
    figure-opts.width,
    figure-opts.responsive,
    figure-opts.align,
  )
  _finalize-figure-content(content, label, fig-labels, figure-opts, anchor)
}

#let _render-display-item(item, label, opts, fig-labels, anchor: true) = {
  let figure-opts = _figure-options(opts)
  let selected = _display-selection(item)
  if selected == none {
    return none
  }
  let mime = selected.mime
  let value = selected.value
  if _is-image-mime(mime) {
    let artifact-path = _artifact-path(value)
    let html-path = _resolve-asset-href(artifact-path)
    let display-width = if figure-opts.width == auto and figure-opts.responsive == true {
      100%
    } else {
      figure-opts.width
    }
    let alt = if figure-opts.alt == none { "" } else { figure-opts.alt }
    if _is-html() and figure-opts.caption != none {
      let img = _html-captioned-image(html-path, figure-opts.height, alt)
      let fig = if fig-labels.len() > 0 {
        figure(
          img,
          caption: _figure-caption(figure-opts.caption, figure-opts.at("caption-location")),
        )
      } else {
        _html-captioned-figure(
          img,
          display-width,
          figure-opts.responsive,
          figure-opts.align,
          figure-opts.caption,
          figure-opts.at("caption-location"),
        )
      }
      let rendered = _label-figure(fig, label, fig-labels, anchor)
      let rendered = if fig-labels.len() > 0 {
        _wrap-html-figure-width(
          rendered,
          display-width,
          figure-opts.responsive,
          figure-opts.align,
        )
      } else {
        rendered
      }
      return _finalize-figure-display(rendered, none, figure-opts.link)
    }
    let img = if _is-html() {
      _html-image(
        html-path,
        display-width,
        figure-opts.height,
        figure-opts.responsive,
        figure-opts.align,
        alt,
      )
    } else {
      image(
        artifact-path,
        width: display-width,
        height: figure-opts.height,
        alt: alt,
      )
    }
    _finalize-figure-content(img, label, fig-labels, figure-opts, anchor)
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
      _output-block(text)
    }
  } else if item-type == "diagnostic" {
    let level = item.at("level", default: "")
    if (level == "warning" and warning != true) or (level == "message" and message != true) {
      none
    } else {
      _output-block(
        item.at("text", default: ""),
        stream: if level == "warning" { "stderr" } else { "stdout" },
      )
    }
  } else if item-type == "error" {
    _output-block(item.at("message", default: ""), stream: "stderr")
  } else if item-type == "display" or item-type == "result" {
    _render-display-item(item, label, opts, fig-labels, anchor: anchor)
  }
}

#let _render-image-group(items, label, opts, fig-labels, anchor) = {
  if items.len() == 0 {
    none
  } else if items.len() == 1 {
    _render-item(items.first(), label, opts, fig-labels, anchor: anchor)
  } else {
    _render-image-grid(items, label, opts, fig-labels, anchor: anchor)
  }
}

// `anchor` controls whether cross-reference labels (and the chunk's internal-id
// label) are attached. The inline render owns the anchor; a relocated copy that
// does not own it passes `anchor: false` so the same output can appear more than
// once without defining a Typst label twice.
#let _render-results(label, opts, anchor: true, config: none) = {
  let runtime-config = _runtime-config(bound: config)
  let results-path = runtime-config.at("results", default: none)
  if results-path == none or results-path == "" {
    return none
  }
  let chunk = _result-chunk(_results-document(config: runtime-config), label)
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
    if _is-image-display-item(result-item) {
      image-group.push(result-item)
    } else {
      if image-group.len() > 0 {
        _render-image-group(image-group, label, opts, fig-labels, anchor)
        image-group = ()
      }
      _render-item(result-item, label, opts, fig-labels, anchor: anchor)
    }
  }
  _render-image-group(image-group, label, opts, fig-labels, anchor)
}
