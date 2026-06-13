#let html(
  title: none,
  lang: "en",
  body,
) = {
  let calepin-target = sys.inputs.at("calepin-target", default: "paged")
  if calepin-target == "html" {
    std.html.elem("html", attrs: (lang: lang))[
      #std.html.elem("head")[
        #std.html.elem("meta", attrs: (charset: "utf-8"))
        #std.html.elem("meta", attrs: (
          name: "viewport",
          content: "width=device-width, initial-scale=1",
        ))

        #if title != none {
          std.html.elem("title")[#title]
        }
      ]

      #std.html.elem("body")[
        #std.html.elem("main", attrs: (class: "container"))[
          #body
        ]
      ]
    ]
  } else {
    body
  }
}
#let _input-syntax-theme = bytes((
  "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
  "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">",
  "<plist version=\"1.0\">",
  "<dict>",
  "  <key>name</key>",
  "  <string>Calepin Code Input Chunk</string>",
  "  <key>settings</key>",
  "  <array>",
  "    <dict>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#003b4f</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Function calls</string>",
  "      <key>scope</key>",
  "      <string>entity.name.function, support.function, variable.function</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4759ab</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Numeric literals</string>",
  "      <key>scope</key>",
  "      <string>constant.numeric</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#ad0000</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Operators and special characters</string>",
  "      <key>scope</key>",
  "      <string>keyword.operator, punctuation.definition, punctuation.separator</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#5e5e5e</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Assignments</string>",
  "      <key>scope</key>",
  "      <string>keyword.operator.assignment</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#003b4f</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Named arguments</string>",
  "      <key>scope</key>",
  "      <string>variable.parameter, entity.other.attribute-name, support.variable.parameter</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#667321</string>",
  "      </dict>",
  "    </dict>",
  "  </array>",
  "</dict>",
  "</plist>",
).join("\n"))

#let _output-syntax-theme = bytes((
  "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
  "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">",
  "<plist version=\"1.0\">",
  "<dict>",
  "  <key>name</key>",
  "  <string>Calepin Code Output Chunk</string>",
  "  <key>settings</key>",
  "  <array>",
  "    <dict>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#1f2933</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Function calls</string>",
  "      <key>scope</key>",
  "      <string>entity.name.function, support.function, variable.function</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#4759ab</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Numeric literals</string>",
  "      <key>scope</key>",
  "      <string>constant.numeric</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>foreground</key>",
  "        <string>#ad0000</string>",
  "      </dict>",
  "    </dict>",
  "    <dict>",
  "      <key>name</key>",
  "      <string>Output emphasis</string>",
  "      <key>scope</key>",
  "      <string>markup.strong</string>",
  "      <key>settings</key>",
  "      <dict>",
  "        <key>fontStyle</key>",
  "        <string>bold</string>",
  "      </dict>",
  "    </dict>",
  "  </array>",
  "</dict>",
  "</plist>",
).join("\n"))
#let _mode = sys.inputs.at("calepin-mode", default: "render")
#let _auto-label-index = state("calepin-auto-label-index", 1)
#let _auto-inline-label-index = state("calepin-auto-inline-label-index", 1)

// Website pages index, provided by `calepin compile` during website builds.
#let _pages-index-path = sys.inputs.at("calepin-pages", default: "")
#let _current-page-href = sys.inputs.at("calepin-current-href", default: "")

// Relative URL prefix from the current page back to the site root.
#let _site-root-prefix() = {
  let depth = _current-page-href.split("/").filter(part => part != "").len() - 1
  if depth <= 0 { "" } else { "../" * depth }
}

// Returns one entry per page of the website: a dictionary with `path` (source
// file), `href` (link to the page, relative to the current page), `title`
// (resolved display title), `pdf` (link to the PDF twin, or none), and `meta`
// (the page's raw `<website-metadata>` dictionary, verbatim). Returns an
// empty array outside website builds.
#let pages() = {
  if _pages-index-path == "" { return () }
  let prefix = _site-root-prefix()
  json(_pages-index-path).map(entry => {
    let entry = entry
    if type(entry.at("href", default: none)) == str {
      entry.insert("href", prefix + entry.href)
    }
    if type(entry.at("pdf", default: none)) == str {
      entry.insert("pdf", prefix + entry.pdf)
    }
    entry
  })
}

#let _base-options = (
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
  auto-label-prefix: "chunk",
  auto-label-state: _auto-label-index,
)

#let _auto-call-defaults(defaults) = {
  let out = (:)
  for key in defaults.keys() {
    out.insert(key, auto)
  }
  out.insert("fig-link", none)
  out.insert("fig-caption", none)
  out.insert("fig-alt-text", none)
  out.insert("fig-subcaptions", none)
  out + _call-extra-defaults
}

#let _call-defaults = _auto-call-defaults(_base-options)

#let _disable-raw-chunk-transforms = state("calepin-disable-raw-chunk-transforms", false)

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

#let _sync-auto-label-counter(auto-label-state, label) = {
  if label.starts-with("chunk-") {
    let suffix = label.slice(6)
    let is-int = suffix.matches(regex("^[0-9]+$")) != ()
    if is-int {
      let next = int(suffix) + 1
      auto-label-state.update(n => if next > n { next } else { n })
    }
  }
}

// Accept `label` as none | str | array of str. Returns the internal id (used
// for results lookup + artifact filenames) and the raw label-name list.
#let _derive-label(label-opt, generated-prefix, counter-value) = {
  if label-opt == none {
    (id: generated-prefix + "-" + str(counter-value), names: (), generated: true)
  } else if type(label-opt) == str {
    (id: label-opt, names: (label-opt,), generated: false)
  } else if type(label-opt) == array {
    if label-opt.len() == 0 { panic("calepin.chunk: label list must not be empty") }
    for entry in label-opt {
      if type(entry) != str { panic("calepin.chunk: label entries must be strings") }
    }
    (id: label-opt.first(), names: label-opt, generated: false)
  } else {
    panic("calepin.chunk: label must be a string or an array of strings")
  }
}

#let _select-representation(data) = {
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

#let _resolve-asset-href(path) = {
  let base = sys.inputs.at("calepin-assets", default: "")
  if base != "" and path.starts-with("/") {
    base + path
  } else {
    path
  }
}

#let _html-target() = sys.inputs.at("calepin-target", default: "paged") == "html"

#let _attach-label(content, id) = [
  #content #label(id)
]

#let _attach-labels(content, ids) = {
  let out = content
  for id in ids {
    out = [#out #label(id)]
  }
  out
}

#let _crossref-labels-for(chunk, kind) = {
  let labels = ()
  for entry in chunk.at("crossref-labels", default: ()) {
    if entry.at("kind", default: "") == kind {
      labels.push(entry.at("name"))
    }
  }
  labels
}
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

#let _figure-caption(fig-caption, fig-cap-location) = {
  if fig-caption == none {
    none
  } else if fig-cap-location == auto or fig-cap-location == none {
    fig-caption
  } else {
    figure.caption(position: fig-cap-location)[#fig-caption]
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
    std.html.elem("img", attrs: (src: path, alt: alt, style: style))
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

// A labeled figure must stay a native `figure` so `@label` cross-references
// resolve, and a native figure cannot carry the display-width style itself.
// Wrap it in a styled block that applies the same width/responsive/alignment as
// an unlabeled captioned figure, so both honor `fig-width`.
#let _wrap-html-figure-width(content, width, responsive, fig-align) = {
  let style = _html-figure-style(width, responsive, fig-align)
  if style == "" {
    content
  } else {
    std.html.elem("div", attrs: (style: style))[#content]
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
  let attrs = if style == "" { (:) } else { (style: style) }
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
  if _html-target() {
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

#let _paged-result-options(options) = {
  let out = (:)
  if "fig-align" in options {
    out.insert("fig-align", options.at("fig-align"))
  }
  out
}

#let _merge-result-options(opts, chunk) = {
  let options = chunk.at("options", default: (:))
  if _html-target() {
    opts + options
  } else {
    opts + _paged-result-options(options)
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
  if _html-target() {
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
  if _html-target() {
    _html-image(html-path, 100%, fig-height, fig-responsive, center, alt)
  } else {
    image(artifact-path, width: 100%, height: fig-height, alt: alt)
  }
}

#let _grid-cell(content, caption) = {
  if _html-target() and caption != none {
    std.html.elem("div", attrs: (style: "min-width: 0;"))[
      #content
      #std.html.elem("div", attrs: (style: "font-size: 0.85em; margin-top: 0.35em;"))[#caption]
    ]
  } else if _html-target() {
    std.html.elem("div", attrs: (style: "min-width: 0;"))[#content]
  } else if caption == none {
    content
  } else {
    stack(spacing: 0.35em, content, text(size: 0.85em)[#caption])
  }
}

#let _wrap-grid-display(content, width, responsive, align) = {
  if _html-target() {
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

#let _render-image-grid(items, label, opts, fig-labels) = {
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
    } else {
      _attach-label(fig, label)
    }
  } else {
    content
  }
  _finalize-figure-display(rendered, fig-align, fig-link)
}

#let _render-display-item(item, label, opts, fig-labels) = {
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
    if _html-target() and fig-caption != none {
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
        _attach-label(fig, label)
      }
      return _finalize-figure-display(rendered, none, fig-link)
    }
    let img = if _html-target() {
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
        _attach-label(fig, label)
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

#let _render-item(item, label, opts, fig-labels) = {
  let results-mode = opts.at("results")
  let inline-output = opts.at("inline-output")
  let warning = opts.at("warning")
  let message = opts.at("message")

  let item-type = item.at("type", default: "")
  if item-type == "stream" {
    let text = item.at("text", default: "")
    if results-mode == "hide" {
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
      _output-block(item.at("text", default: ""), stream: if level == "warning" { "stderr" } else { "stdout" })
    }
  } else if item-type == "error" {
    _output-block(item.at("message", default: ""), stream: "stderr")
  } else if item-type == "display" or item-type == "result" {
    _render-display-item(item, label, opts, fig-labels)
  }
}

#let _render-results(label, opts) = {
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
  let fig-labels = _crossref-labels-for(chunk, "fig")
  let items = chunk.at("items", default: ())
  let image-group = ()
  for result-item in items {
    if _is-image-display-item(result-item, opts) {
      image-group.push(result-item)
    } else {
      if image-group.len() > 0 {
        if image-group.len() == 1 {
          _render-item(image-group.first(), label, opts, fig-labels)
        } else {
          _render-image-grid(image-group, label, opts, fig-labels)
        }
        image-group = ()
      }
      _render-item(result-item, label, opts, fig-labels)
    }
  }
  if image-group.len() > 0 {
    if image-group.len() == 1 {
      _render-item(image-group.first(), label, opts, fig-labels)
    } else {
      _render-image-grid(image-group, label, opts, fig-labels)
    }
  }
}
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
  if _mode == "query" {
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

#let _chunk-spec(body, engine, label, crossref-labels, options) = {
  let out = (
    body: body,
    engine: engine,
    label: label,
    "crossref-labels": crossref-labels,
  )
  for key in _base-options.keys() {
    if key != "fenced-chunks" {
      out.insert(key, options.at(key))
    }
  }
  out
}

#let _query-crossref-placeholders(crossref-labels) = {
  let out = []
  for name in crossref-labels {
    if type(name) == str and name.starts-with("fig-") {
      out += [#figure(box(width: 0pt, height: 0pt), caption: none) #label(name)]
    }
  }
  out
}

#let _strip-qmd-label-quotes(value) = {
  let value = value.trim()
  if value.len() >= 2 and (
    (value.starts-with("\"") and value.ends-with("\"")) or
    (value.starts-with("'") and value.ends-with("'"))
  ) {
    value.slice(1, value.len() - 1)
  } else {
    value
  }
}

#let _parse-qmd-label-value(value) = {
  let value = value.trim()
  if value.starts-with("[") and value.ends-with("]") {
    let inner = value.slice(1, value.len() - 1).trim()
    if inner == "" {
      return ()
    }
    let labels = ()
    for item in inner.split(",") {
      labels.push(_strip-qmd-label-quotes(item))
    }
    labels
  } else {
    _strip-qmd-label-quotes(value)
  }
}

#let _qmd-label-from-body(body) = {
  let code = _raw-text(body)
  let code = if code.starts-with("\n") { code.slice(1) } else { code }
  for line in code.split("\n") {
    let trimmed = line.trim()
    if not trimmed.starts-with("#|") {
      return none
    }
    let directive = trimmed.slice(2).trim()
    let colon = directive.position(":")
    if colon == none {
      continue
    }
    let key = directive.slice(0, colon).trim()
    if key == "label" {
      return _parse-qmd-label-value(directive.slice(colon + 1))
    }
  }
  none
}

#let _label-name(value) = {
  let value = str(value)
  if value.starts-with("<") and value.ends-with(">") and value.len() >= 2 {
    value.slice(1, value.len() - 1)
  } else {
    value
  }
}

#let _metadata-fence-label(node) = {
  if node.has("label") and node.label == <calepin-fence-label> {
    let value = node.value
    if type(value) == dictionary and value.at("label", default: none) != none {
      return _label-name(value.at("label"))
    }
    panic("calepin.chunk: trailing fence label metadata is malformed")
  }
  none
}

#let _fence-label-from-body(body) = {
  let labels = ()
  let raw = _raw-node(body)
  if raw.has("label") {
    labels.push(_label-name(raw.label))
  }
  if body.has("children") {
    for child in body.children {
      let label = _metadata-fence-label(child)
      if label != none {
        labels.push(label)
      }
    }
  }
  if labels.len() > 1 {
    panic("calepin.chunk: label supplied more than once")
  }
  if labels.len() == 1 {
    labels.first()
  } else {
    none
  }
}

#let _strip-qmd-header(code) = {
  let out = ""
  let reading-header = true
  for line in code.split("\n") {
    if reading-header and line.trim().starts-with("#|") {
      continue
    }
    reading-header = false
    if out == "" {
      out = line
    } else {
      out += "\n" + line
    }
  }
  out
}

// Detect and strip a version suffix that Typst's fence parser split from
// the lang identifier.  For example, ```julia-1.2 produces lang="julia-1"
// with ".2\n" prepended to the code text.  This mirrors the
// reattach_version_suffix() logic in query.rs so the echo shows clean code.
#let _strip-lang-version-suffix(engine, code) = {
  let builtin-engines = ("python", "r", "mermaid", "dot", "tikz", "d2")
  if engine in builtin-engines { return code }
  let nl = code.position("\n")
  if nl == none { return code }
  let first-line = code.slice(0, nl)
  if not first-line.starts-with(".") or first-line.len() < 2 { return code }
  let tail = first-line.slice(1)
  let parts = tail.split(".")
  let is-version = parts.all(part =>
    part.len() > 0 and part.match(regex("^[0-9]+$")) != none
  )
  if not is-version { return code }
  code.slice(nl + 1)
}

#let _emit-chunk(engine, body, ..args) = context {
  let options = _call-defaults + args.named()
  let label-opt = options.at("label")
  let qmd-label-opt = _qmd-label-from-body(body)
  let fence-label-opt = _fence-label-from-body(body)
  let label-count = (
    if label-opt != none { 1 } else { 0 }
  ) + (
    if qmd-label-opt != none { 1 } else { 0 }
  ) + (
    if fence-label-opt != none { 1 } else { 0 }
  )
  if label-count > 1 {
    panic("calepin.chunk: label supplied more than once")
  }
  let label-opt = if qmd-label-opt != none {
    qmd-label-opt
  } else if fence-label-opt != none {
    fence-label-opt
  } else {
    label-opt
  }
  let auto-label-state = options.at("auto-label-state")
  let auto-label-prefix = options.at("auto-label-prefix")
  let derived = _derive-label(label-opt, auto-label-prefix, auto-label-state.get())
  let label = derived.id
  let crossref-labels = derived.names
  let generated-label = derived.generated
  let label-step = if generated-label {
    auto-label-state.update(n => n + 1)
  } else {
    _sync-auto-label-counter(auto-label-state, label)
  }
  if _mode == "query" {
    [
      #label-step
      #metadata(_chunk-spec(body, engine, label, crossref-labels, options)) <calepin-chunk>
      #_query-crossref-placeholders(crossref-labels)
    ]
  } else {
    let code = _raw-text(body)
    let code = if code.starts-with("\n") { code.slice(1) } else { code }
    let code = _strip-lang-version-suffix(engine, code)
    let code = _strip-qmd-header(code)
    let options = _resolve-options(engine, options)
    let show-echo = options.at("echo") == true
    let results-path = sys.inputs.at("calepin-results", default: "")
    label-step
    [#metadata((label: label, page: here().page())) <calepin-page>]

    if show-echo {
      _input-block(code, lang: engine)
    } else if results-path == "" {
      _input-block(code, lang: engine)
    }
    if results-path != "" {
      _render-results(label, options)
    }
  }
}

#let _without-raw-chunk-transforms(body) = context {
  let disabled = _disable-raw-chunk-transforms.get()
  _disable-raw-chunk-transforms.update(_ => true)
  let rendered = body()
  _disable-raw-chunk-transforms.update(_ => disabled)
  rendered
}

// `fenced-chunks` is the single switch for auto-running plain fenced blocks:
// `true` (every engine), an engine name, or a list of engine names.
#let _fenced-chunks-runs(engine, setting) = {
  if engine in ("typ", "typst") {
    false
  } else if setting == true {
    true
  } else if type(setting) == str {
    setting == engine
  } else if type(setting) == array {
    setting.contains(engine)
  } else {
    false
  }
}

#let chunk-from-raw-plain(engine, it) = {
  let defaults = _resolve-options(engine, _call-defaults)
  if _fenced-chunks-runs(engine, defaults.at("fenced-chunks")) {
    _emit-chunk(engine, it, ..defaults)
  } else {
    it
  }
}

#let _infer-engine(body) = {
  let node = _raw-node(body)
  if node.has("lang") and node.lang != none {
    node.lang
  } else {
    panic("calepin.chunk: no engine given; add a language to the fence (e.g. ```python) or pass the engine name")
  }
}

// `chunk` accepts either an explicit engine (`chunk("python")[...]`) or just a
// body (`chunk[```python ... ```]`), in which case the engine is read from the
// fenced block's language.
#let chunk(..args) = {
  let positional = args.pos()
  let engine = none
  let body = none
  if positional.len() >= 2 and type(positional.at(0)) == str {
    engine = positional.at(0)
    body = positional.at(1)
  } else if positional.len() >= 1 {
    body = positional.at(0)
    engine = _infer-engine(body)
  } else {
    panic("calepin.chunk: missing code block")
  }
  _without-raw-chunk-transforms(() => _emit-chunk(engine, body, ..args.named()))
}

#let inline(engine, body, ..args) = {
  let opts = args.named()
  if opts.at("label", default: none) != none {
    panic("unexpected argument: label")
  }
  let defaults = (
    echo: false,
    inline-output: true,
    auto-label-prefix: "inline",
    auto-label-state: _auto-inline-label-index,
  )
  chunk(engine, body, ..(defaults + opts))
}
