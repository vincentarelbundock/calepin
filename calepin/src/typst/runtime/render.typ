#let _raw-block(value, lang: none, theme: auto) = {
  show raw.where(block: true): set text(size: 1em)
  raw(value, block: true, lang: lang, theme: theme)
}

#let _resolve-input-theme() = {
  let theme = sys.inputs.at("calepin-raw-theme", default: "")
  if theme == "" {
    _input-syntax-theme
  } else {
    theme
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

#let _input-block(code, lang: none) = {
  if sys.inputs.at("calepin-target", default: "paged") == "html" {
    std.html.elem("div", attrs: (
      class: "sourceCode",
      "data-lang": _block-lang-label(lang),
    ))[
      #_raw-block(code, lang: lang, theme: _resolve-input-theme())
    ]
  } else {
    block(
      width: 100%,
      fill: rgb("#f7f7f5"),
      stroke: 0.5pt + rgb("#d8d8d2"),
      radius: 2pt,
      inset: (x: 0.65em, y: 0.45em),
    )[
      #text(fill: rgb("#1f2933"))[
        #_raw-block(code, lang: lang, theme: _input-syntax-theme)
      ]
    ]
  }
}

#let _output-block(output, stream: "stdout") = {
  if sys.inputs.at("calepin-target", default: "paged") == "html" {
    let class = if stream == "stderr" {
      "cell-output cell-output-stderr"
    } else {
      "cell-output cell-output-stdout"
    }
    std.html.elem("div", attrs: (class: class))[
      #_raw-block(output, theme: _output-syntax-theme)
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
    block(
      width: 100%,
      fill: fill,
      stroke: stroke,
      radius: 2pt,
      inset: (x: 0.65em, y: 0.4em),
    )[
      #if stream == "stderr" {
        text(fill: rgb("#5f3328"))[
          #_raw-block(output, theme: _output-syntax-theme)
        ]
      } else {
        _raw-block(output, theme: _output-syntax-theme)
      }
    ]
  }
}

#let _figure-caption(fig-caption, fig-caption-position) = {
  if fig-caption == none {
    none
  } else if fig-caption-position == auto or fig-caption-position == none {
    fig-caption
  } else {
    figure.caption(position: fig-caption-position)[#fig-caption]
  }
}

#let _css-size(value) = {
  if value == none or value == auto {
    none
  } else if type(value) == str {
    value
  } else {
    repr(value)
  }
}

#let _css-decl(property, value) = {
  let size = _css-size(value)
  if size == none or size == "" {
    ""
  } else {
    property + ": " + size + ";"
  }
}

#let _append-css(base, next) = {
  if next == "" {
    base
  } else if base == "" {
    next
  } else {
    base + " " + next
  }
}

#let _html-image-align-style(fig-display-align) = {
  if fig-display-align == left or fig-display-align == start {
    "margin-inline: 0 auto;"
  } else if fig-display-align == right or fig-display-align == end {
    "margin-inline: auto 0;"
  } else {
    "margin-inline: auto;"
  }
}

#let _html-block-align-style(fig-display-align) = {
  if fig-display-align == left or fig-display-align == start {
    "text-align: left;"
  } else if fig-display-align == right or fig-display-align == end {
    "text-align: right;"
  } else if fig-display-align == center {
    "text-align: center;"
  } else {
    ""
  }
}

#let _html-image-style(width, height, responsive, fig-display-align) = {
  let base = _append-css("display: block;", _html-image-align-style(fig-display-align))
  let with-width = _append-css(base, _css-decl("width", width))
  let with-height = _append-css(with-width, _css-decl("height", height))
  if responsive == true {
    _append-css(with-height, "max-width: 100%;")
  } else {
    with-height
  }
}

#let _html-image(path, width, height, responsive, fig-display-align, alt) = {
  let style = _html-image-style(width, height, responsive, fig-display-align)
  if style == "" {
    std.html.elem("img", attrs: (src: path, alt: alt))
  } else {
    std.html.elem("img", attrs: (src: path, alt: alt, style: style))
  }
}

#let _html-captioned-image(path, height, alt) = {
  let style = _append-css(_append-css("display: block;", "width: 100%;"), _css-decl("height", height))
  std.html.elem("img", attrs: (src: path, alt: alt, style: style))
}

#let _html-figure-style(width, responsive, fig-display-align) = {
  let with-width = _css-decl("width", width)
  let with-responsive = if responsive == true {
    _append-css(with-width, "max-width: 100%;")
  } else {
    with-width
  }
  _append-css(with-responsive, _html-image-align-style(fig-display-align))
}

#let _html-captioned-figure(
  img,
  width,
  responsive,
  fig-display-align,
  fig-caption,
  fig-caption-position,
) = {
  let style = _html-figure-style(width, responsive, fig-display-align)
  let attrs = if style == "" { (:) } else { (style: style) }
  let caption = std.html.elem("figcaption")[#context [Figure #counter(figure).display(): #fig-caption]]
  let content = if fig-caption-position == top {
    [#caption #img]
  } else {
    [#img #caption]
  }
  [
    #counter(figure).step()
    #std.html.elem("figure", attrs: attrs)[#content]
  ]
}

#let _finalize-figure-display(content, fig-display-align, fig-display-link) = {
  let linked = if fig-display-link == none or fig-display-link == auto {
    content
  } else {
    link(fig-display-link)[#content]
  }
  if _html-target() {
    let style = _html-block-align-style(fig-display-align)
    if style == "" {
      return linked
    }
    return std.html.elem("div", attrs: (style: style))[#linked]
  }
  if fig-display-align == none or fig-display-align == auto {
    linked
  } else {
    align(fig-display-align)[#linked]
  }
}

#let _render-display-item(item, label, opts) = {
  let format = opts.at("format")
  let fig-display-width = opts.at("fig-display-width")
  let fig-display-height = opts.at("fig-display-height")
  let fig-display-align = opts.at("fig-display-align")
  let fig-display-responsive = opts.at("fig-display-responsive")
  let fig-display-link = opts.at("fig-display-link")
  let fig-caption = opts.at("fig-caption")
  let fig-caption-position = opts.at("fig-caption-position")
  let fig-alt-text = opts.at("fig-alt-text")
  let kind = opts.at("kind")

  let data = item.at("data", default: (:))
  let selected = _select-representation(data, format)
  if selected == none {
    return none
  }
  let mime = selected.mime
  let value = selected.value
  if mime == "image/svg+xml" or mime == "image/png" {
    let artifact-path = _artifact-path(value)
    let html-path = _resolve-asset-href(artifact-path)
    let display-width = if fig-display-width == auto and fig-display-responsive == true { 100% } else { fig-display-width }
    let alt = if fig-alt-text == none { "" } else { fig-alt-text }
    if _html-target() and fig-caption != none {
      let img = _html-captioned-image(html-path, fig-display-height, alt)
      let rendered = _attach-label(
        _html-captioned-figure(img, display-width, fig-display-responsive, fig-display-align, fig-caption, fig-caption-position),
        label,
      )
      return _finalize-figure-display(rendered, none, fig-display-link)
    }
    let img = if _html-target() {
      _html-image(html-path, display-width, fig-display-height, fig-display-responsive, fig-display-align, alt)
    } else {
      image(
        artifact-path,
        width: display-width,
        height: fig-display-height,
        alt: alt,
      )
    }
    let rendered = if fig-caption != none {
      _attach-label(
        figure(img, caption: _figure-caption(fig-caption, fig-caption-position)),
        label,
      )
    } else {
      img
    }
    _finalize-figure-display(rendered, fig-display-align, fig-display-link)
  } else if mime == "text/x-typst" {
    let rendered = eval(value, mode: "markup")
    if fig-caption != none or kind == "table" or label.starts-with("tbl-") {
      _finalize-figure-display(
        _attach-label(
          figure(kind: table, caption: _figure-caption(fig-caption, fig-caption-position))[
            #rendered
          ],
          label,
        ),
        fig-display-align,
        fig-display-link,
      )
    } else {
      rendered
    }
  } else if mime == "application/json" {
    _output-block(repr(value))
  } else {
    _output-block(str(value))
  }
}

#let _render-item(item, label, opts) = {
  let results-mode = opts.at("results")
  let inline-output = opts.at("inline-output")
  let warning = opts.at("warning")
  let message = opts.at("message")

  let item-type = item.at("type", default: "")
  if item-type == "stream" {
    let text = item.at("text", default: "")
    if results-mode == "hide" {
      none
    } else if inline-output and results-mode == "asis" {
      eval(text, mode: "markup")
    } else if inline-output {
      text
    } else if results-mode == "asis" {
      eval(text, mode: "markup")
    } else {
      _output-block(text)
    }
  } else if item-type == "diagnostic" {
    let level = item.at("level", default: "")
    if (level == "warning" and warning != true) or (level == "message" and message != true) {
      none
    } else {
      _output-block(item.at("text", default: ""), stream: if level == "warning" { "stderr" } else { "stdout" })
    }
  } else if item-type == "error" {
    _output-block(item.at("message", default: ""), stream: "stderr")
  } else if item-type == "display" or item-type == "result" {
    _render-display-item(item, label, opts)
  }
}

#let _render-results(label, opts) = {
  let results-path = sys.inputs.at("calepin-results", default: "")
  if results-path == "" {
    return none
  }
  let item = opts.at("item")
  let results-doc = json(results-path)
  let chunk = results-doc.at("chunks", default: (:)).at(label, default: none)
  if chunk == none {
    panic("calepin results do not contain label `" + label + "`")
  }
  let items = chunk.at("items", default: ())
  if item == "first" {
    if items.len() > 0 {
      return _render-item(items.first(), label, opts)
    }
    return none
  }
  if item == "last" {
    if items.len() > 0 {
      return _render-item(items.last(), label, opts)
    }
    return none
  }
  if type(item) == int {
    let idx = if item < 0 { items.len() + item } else { item }
    if idx >= 0 and idx < items.len() {
      return _render-item(items.at(idx), label, opts)
    }
    return none
  }
  for result-item in items {
    _render-item(result-item, label, opts)
  }
}
