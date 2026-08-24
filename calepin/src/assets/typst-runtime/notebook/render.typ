#import "../core/assets.typ": _resolve-asset-href
#import "../core/config.typ": _runtime-config
#import "../core/results.typ": _result-chunk, _results-document
#import "../core/css.typ": _append-css, _css-decl
#import "../core/target.typ": _is-html
#import "code.typ" as codemod
#import "result-support.typ": _artifact-path, _attach-label, _attach-labels
#import "result-support.typ": _crossref-labels-for, _select-representation

#let _html-themed-raw-block = codemod._html-themed-raw-block
#let _input-block = codemod._input-block
#let _output-block = codemod._output-block

// Typst steps a figure's counter for every figure element it sees, captioned or
// not, so a Calepin wrapper placed around output that is already a figure (a
// `tinytable` table, say) costs the kind's counter two steps and numbers the
// inner caption one ahead of the reference. The two helpers below let a chunk
// see that case coming: `_trails-in-figure` recognises evaluated markup whose
// last element is a figure the code produced itself, which the chunk can label
// directly instead of wrapping it again.

#let _is-blank-element(item) = {
  let kind = item.func()
  kind == [ ].func() or kind == parbreak or kind == linebreak
}

// Only the wrappers Typst puts around an evaluated markup body (`sequence`,
// `styled`) are traversed. A figure nested deeper belongs to whatever content
// holds it, not to the chunk.
#let _trails-in-figure(content) = {
  if type(content) != type([]) {
    return false
  }
  if content.func() == figure {
    return true
  }
  let fields = content.fields()
  if "child" in fields {
    return _trails-in-figure(fields.child)
  }
  if "children" in fields {
    let children = fields.children.filter(child => not _is-blank-element(child))
    if children.len() == 0 {
      return false
    }
    return _trails-in-figure(children.last())
  }
  false
}

// Evaluate Typst markup a chunk printed.
//
// `typst-labels` hands the chunk's cross-reference identity to the figure the
// markup itself produced: appending the labels to the source attaches them to
// that figure, where an enclosing Calepin figure would have taken the number
// the reference resolves to. `typst-demote-figures` covers the case where
// Calepin still has to wrap (it owns a caption of its own): nested figures keep
// their caption text but stop consuming counter steps.
#let _eval-typst(source, opts) = {
  let labels = opts.at("typst-labels", default: ())
  if labels.len() > 0 {
    let suffix = labels.map(name => "<" + name + ">").join(" ")
    return eval(source + "\n" + suffix, mode: "markup")
  }
  if opts.at("typst-demote-figures", default: false) {
    set figure(numbering: none)
    eval(source, mode: "markup")
  } else {
    eval(source, mode: "markup")
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

#let _label-figure(content, label, fig-labels, anchor) = {
  if fig-labels.len() > 0 {
    _attach-labels(content, fig-labels)
  } else if anchor {
    _attach-label(content, label)
  } else {
    content
  }
}

#let _figure-or-content(
  content,
  label,
  fig-labels,
  caption,
  caption-location,
  anchor,
  kind: auto,
) = {
  if caption == none and fig-labels.len() == 0 {
    content
  } else {
    _label-figure(
      figure(content, kind: kind, caption: _figure-caption(caption, caption-location)),
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
    // A Typst `figure` ignores an outer `align()` (it centers itself by
    // default), so aligning a captioned/labeled figure requires a show rule.
    // `align(...)` still handles the bare-image (uncaptioned) case.
    [
      #show figure: set align(fig-align)
      #align(fig-align)[#linked]
    ]
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
  if value.ends-with("fr") {
    return float(value.slice(0, value.len() - 2).trim()) * 1fr
  }
  value
}

// Grid track lists serialize as arrays of size strings, so each entry needs
// the same string-to-length conversion a scalar size gets.
#let _paged-layout-tracks(value) = {
  if type(value) == array {
    value.map(_paged-layout-size)
  } else {
    _paged-layout-size(value)
  }
}

// Display options declared in a fenced `#|` chunk header exist only in the
// serialized result options: Typst reads nothing but `label` out of the
// header, so a caption, alt text, or layout written there never reaches the
// call options this render started from. Restore every stored option that was
// actually set (unset ones serialize as `none` and must not clobber call-site
// values); paged output needs size strings converted to lengths, HTML
// consumes them directly.
#let _merge-result-options(opts, chunk) = {
  let out = opts
  for (key, value) in chunk.at("options", default: (:)) {
    if value == none {
      continue
    }
    if not _is-html() and key in ("fig-width", "fig-height") {
      value = _paged-layout-size(value)
    } else if not _is-html() and key in ("fig-layout-columns", "fig-layout-rows") {
      value = _paged-layout-tracks(value)
    }
    out.insert(key, value)
  }
  out
}

// Sizes written by a relocation call go through the same string-to-length
// conversion the serialized chunk options get, so `fig-width: "50%"` and
// `fig-width: 50%` behave alike in paged output.
#let _relocation-override-values(overrides) = {
  if _is-html() {
    return overrides
  }
  let out = overrides
  for key in ("fig-width", "fig-height") {
    if key in out and out.at(key) != none {
      out.insert(key, _paged-layout-size(out.at(key)))
    }
  }
  out
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

#let _finalize-figure-content(content, label, fig-labels, figure-opts, anchor, kind: auto) = {
  let rendered = _figure-or-content(
    content,
    label,
    fig-labels,
    figure-opts.caption,
    figure-opts.at("caption-location"),
    anchor,
    kind: kind,
  )
  _finalize-figure-display(rendered, figure-opts.align, figure-opts.link)
}

#let _display-selection(item) = {
  let data = item.at("data", default: (:))
  _select-representation(data)
}

#let _typst-source-value(value) = {
  if type(value) == dictionary and value.at("path", default: none) != none {
    read(_artifact-path(value), encoding: "utf8")
  } else {
    value
  }
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

// Panels of a multi-plot chunk are figures of their own kind, so they carry a
// counter Typst can number and a label a reference can resolve, without
// disturbing the figure counter their parent uses.
#let _subfigure-kind = "calepin-subfigure"

// `@fig-name-2` names the second panel, 1-based, matching the documented form.
#let _subfigure-label-name(base, index) = base + "-" + str(index + 1)

#let _subfigure-number(location) = numbering(
  "a",
  ..counter(figure.where(kind: _subfigure-kind)).at(location),
)

// Typst would caption a panel "Figure N"; print the letter and the chunk's own
// sub-caption text instead. Installed around the grid rather than in
// `calepin.document`, so a panel is lettered even when the runtime is driven
// directly. Reference formatting still needs the document rule, since a
// reference can sit anywhere in the prose.
#let _subfigure-panel = it => {
  let n = context _subfigure-number(it.location())
  block(breakable: false)[
    #it.body
    #if it.caption != none {
      text(size: 0.85em)[(#n)~#it.caption.body]
    } else {
      text(size: 0.85em)[(#n)]
    }
  ]
}

// A panel becomes a sub-figure when it can be referenced (the chunk carries a
// `fig-` label) or when it has a sub-caption to letter. A plain multi-plot
// chunk keeps its bare grid: no letters, no counter, no labels.
#let _panels-are-subfigures(subcaptions, fig-labels) = {
  fig-labels.len() > 0 or (subcaptions != none and subcaptions != auto)
}

#let _subfigure-cell(content, caption, fig-labels, index, anchor) = {
  // A custom `kind` has no default supplement, and Typst refuses to render one
  // without it. Set it here rather than in a document show rule, so a panel is
  // valid even when the runtime is used outside `calepin.document`. The panel
  // caption prints its letter itself, so the supplement stays empty.
  let panel = figure(content, kind: _subfigure-kind, supplement: none, caption: caption)
  if anchor {
    for base in fig-labels {
      panel = [#panel #std.label(_subfigure-label-name(base, index))]
    }
  }
  if _is-html() {
    std.html.elem("div", attrs: (style: "min-width: 0;"))[#panel]
  } else {
    panel
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
  let subfigures = _panels-are-subfigures(figure-opts.subcaptions, fig-labels)

  let cells = ()
  for (index, item) in items.enumerate() {
    let panel = _grid-image(item, figure-opts)
    let caption = _caption-for-index(figure-opts.subcaptions, index)
    cells.push(
      if subfigures {
        _subfigure-cell(panel, caption, fig-labels, index, anchor)
      } else {
        _grid-cell(panel, caption)
      },
    )
  }

  let columns = _grid-columns(items.len(), figure-opts.columns, figure-opts.rows)
  let grid-content = _grid-content(columns, figure-opts.rows, cells)
  // Panel letters restart inside every parent figure.
  let grid-content = if subfigures {
    [
      #show figure.where(kind: _subfigure-kind): _subfigure-panel
      #counter(figure.where(kind: _subfigure-kind)).update(0)
      #grid-content
    ]
  } else {
    grid-content
  }
  let content = _wrap-grid-display(
    grid-content,
    figure-opts.width,
    figure-opts.responsive,
    figure-opts.align,
  )
  // A panel reference reads its parent's number off the image counter, so the
  // parent must land there rather than in whatever kind Typst infers for a grid.
  _finalize-figure-content(
    content,
    label,
    fig-labels,
    figure-opts,
    anchor,
    kind: if subfigures { image } else { auto },
  )
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
    _eval-typst(_typst-source-value(value), opts)
  } else if mime == "application/json" {
    _output-block(repr(value), kind: "result")
  } else {
    _output-block(str(value), kind: "result")
  }
}

#let _render-item(item, label, opts, fig-labels, anchor: true, source-block: none) = {
  let results-mode = opts.at("results")
  let inline-output = opts.at("inline-output")
  let warning = opts.at("warning")
  let message = opts.at("message")

  let item-type = item.at("type", default: "")
  if item-type == "source" {
    // Source segments reach this point only when the chunk interleaves; every
    // other path strips them before rendering.
    if source-block != none {
      source-block(item.at("text", default: ""))
    }
  } else if item-type == "stream" {
    let text = item.at("text", default: "")
    if _results-hidden(results-mode) {
      none
    } else if results-mode == "typst" {
      _eval-typst(text, opts)
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
        kind: if level == "warning" { "warning" } else { "stdout" },
      )
    }
  } else if item-type == "error" {
    _output-block(item.at("message", default: ""), kind: "error")
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

// The Typst markup an item hands to the document, or `none` when the item is
// not Typst markup: printed output the chunk asked to pass through
// (`results: "typst"`), or a display item the engine tagged as Typst.
#let _typst-item-source(item, opts) = {
  let item-type = item.at("type", default: "")
  if item-type == "stream" {
    if opts.at("results", default: "render") == "typst" {
      item.at("text", default: "")
    } else {
      none
    }
  } else if item-type == "display" or item-type == "result" {
    let selected = _display-selection(item)
    if selected != none and selected.mime == "text/x-typst" {
      _typst-source-value(selected.value)
    } else {
      none
    }
  } else {
    none
  }
}

// How many of the chunk's items print Typst markup that ends in a figure of
// their own.
#let _own-figure-count(items, opts) = {
  let count = 0
  for item in items {
    let source = _typst-item-source(item, opts)
    if source != none and _trails-in-figure(eval(source, mode: "markup")) {
      count += 1
    }
  }
  count
}

// How many runs of consecutive image items the chunk's output contains. Output
// between two plots (R flushes `cat()` between plot calls; Python buffers its
// stdout ahead of them) starts a new run.
#let _image-group-count(items) = {
  let count = 0
  let in-group = false
  for item in items {
    if _is-image-display-item(item) {
      if not in-group {
        count += 1
        in-group = true
      }
    } else {
      in-group = false
    }
  }
  count
}

// Render a chunk's items in order, batching consecutive images into one figure
// each. `fig-labels` and the caption carried by `opts` are attached by every
// batch, so callers pass them only when a single batch will result.
#let _render-item-sequence(items, label, opts, anchor, fig-labels: (), source-block: none) = {
  let image-group = ()
  for result-item in items {
    if _is-image-display-item(result-item) {
      image-group.push(result-item)
    } else {
      if image-group.len() > 0 {
        _render-image-group(image-group, label, opts, fig-labels, anchor)
        image-group = ()
      }
      _render-item(result-item, label, opts, fig-labels, anchor: anchor, source-block: source-block)
    }
  }
  _render-image-group(image-group, label, opts, fig-labels, anchor)
}

// Render a chunk's items, attaching its `fig-` identity exactly once.
//
// A chunk's caption and cross-reference label name one figure. When other
// output splits the images into several batches, letting each batch attach them
// defines the label twice (Typst rejects the reference) and numbers the caption
// twice. Wrap the whole chunk in a single figure instead.
#let _render-figure-sequence(items, label, opts, fig-labels, anchor, source-block: none) = {
  let is-figure = fig-labels.len() > 0 or opts.at("fig-caption", default: none) != none
  if is-figure and _image-group-count(items) > 1 {
    // Batches render without a caption or labels of their own, so none of them
    // becomes a figure and the outer figure is the only one.
    let body = _render-item-sequence(
      items,
      label,
      opts + ("fig-caption": none),
      anchor,
      source-block: source-block,
    )
    _finalize-figure-content(body, label, fig-labels, _figure-options(opts), anchor)
  } else {
    _render-item-sequence(
      items,
      label,
      opts,
      anchor,
      fig-labels: fig-labels,
      source-block: source-block,
    )
  }
}

#let _is-source-item(item) = item.at("type", default: "") == "source"

#let _without-source-items(items) = items.filter(item => not _is-source-item(item))

// Whether the chunk's own render splits its source, showing each segment above
// the output it produced, rather than echoing the source once and following it
// with everything the chunk printed.
//
// `results-location: "chunk"` asks for the latter outright. So do several cases
// where interleaving would break an identity the chunk carries:
//
// - fewer than two source segments: the engine did not split the source (Julia,
//   `sh`, and the diagram engines never do), so there is nothing to interleave
//   and the single segment renders as an ordinary echo;
// - an `lst-` identity, which names the echoed source as one listing;
// - a `tbl-` identity, whose figure would end up enclosing the source;
// - a `fig-` identity whose images arrive in more than one batch, since the
//   chunk is then wrapped in a single outer figure that would enclose the
//   source too.
#let _interleaves-source(items, opts, chunk) = {
  if opts.at("results-location", default: "statement") != "statement" {
    return false
  }
  let segments = items.filter(_is-source-item)
  if segments.len() < 2 {
    return false
  }
  // Every line of the chunk has to appear in some segment, or interleaving
  // would echo less code than the chunk holds. An engine stops reporting source
  // once a statement raises, so a tolerated error (`error: true`) truncates the
  // segments; fall back to echoing the stored source whole.
  let source = chunk.at("source", default: "")
  if source != "" and segments.map(item => item.at("text", default: "")).join("\n").trim() != source.trim() {
    return false
  }
  if (
    _crossref-labels-for(chunk, "lst").len() > 0
      or opts.at("lst-caption", default: none) != none
  ) {
    return false
  }
  if (
    _crossref-labels-for(chunk, "tbl").len() > 0
      or opts.at("tbl-caption", default: none) != none
  ) {
    return false
  }
  let is-figure = (
    _crossref-labels-for(chunk, "fig").len() > 0
      or opts.at("fig-caption", default: none) != none
  )
  if is-figure and _image-group-count(items) > 1 {
    return false
  }
  true
}

// Wrap rendered output as a table figure, so `@tbl-name` resolves and the
// caption is numbered from Typst's table counter rather than the figure one.
#let _table-figure(content, caption, tbl-labels, opts, anchor) = {
  // A table figure whose body is printed output rather than a real `table`
  // element carries no implicit description, so strict PDF/UA renders demand
  // one. Reuse the chunk's `fig-alt-text` when the author set it.
  let alt = opts.at("fig-alt-text", default: none)
  let rendered = figure(
    content,
    kind: table,
    caption: _figure-caption(caption, opts.at("fig-cap-location", default: auto)),
    ..if alt == none or alt == "" { (:) } else { (alt: alt) },
  )
  if tbl-labels.len() > 0 {
    _attach-labels(rendered, tbl-labels)
  } else {
    rendered
  }
}

// `anchor` controls whether cross-reference labels (and the chunk's internal-id
// label) are attached. The inline render owns the anchor; a relocated copy that
// does not own it passes `anchor: false` so the same output can appear more than
// once without defining a Typst label twice.
// `source-block` is how the chunk's own render hands over its echo: a function
// from one source segment to the rendered code block. Given one, this call owns
// the echo, either splitting it across the chunk's output or emitting it once
// ahead of everything. A relocation passes none and never echoes.
#let _render-results(
  label,
  opts,
  anchor: true,
  overrides: (:),
  config: none,
  source-block: none,
  source-code: none,
) = {
  let runtime-config = _runtime-config(bound: config)
  let results-path = runtime-config.at("results", default: none)
  if results-path == none or results-path == "" {
    return none
  }
  let chunk = _result-chunk(_results-document(config: runtime-config), label)
  if chunk == none {
    panic("calepin results do not contain label `" + label + "`")
  }
  // Relocation-specific choices must win over the serialized chunk options
  // that `_merge-result-options` restores.
  let opts = _merge-result-options(opts, chunk) + _relocation-override-values(overrides)
  // `results: "hide"` only suppresses a chunk's own inline render. Reaching
  // `_render-results` at all (e.g. through a `#calepin.results` relocation)
  // means the output should be shown here.
  if _results-hidden(opts.at("results", default: "render")) {
    opts.insert("results", "render")
  }
  let fig-labels = if anchor { _crossref-labels-for(chunk, "fig") } else { () }
  let tbl-labels = if anchor { _crossref-labels-for(chunk, "tbl") } else { () }
  let items = chunk.at("items", default: ())
  let tbl-caption = opts.at("tbl-caption", default: none)

  // Output that is already a figure decides how the chunk's identity is
  // attached. Wrapping it a second time would take a counter step of its own,
  // numbering the reference and the printed caption one apart, so the chunk
  // hands its labels to that figure and adds no wrapper. When Calepin owns a
  // caption too there is no way around a wrapper, and the inner figures are
  // demoted to unnumbered instead.
  let own-figures = _own-figure-count(items, opts)
  let fig-caption = opts.at("fig-caption", default: none)
  let captioned = tbl-caption != none or fig-caption != none
  let identity-labels = tbl-labels + fig-labels
  let self-labels = (
    own-figures == 1
      and identity-labels.len() > 0
      and not captioned
      and _image-group-count(items) == 0
  )
  // A chunk that neither carries a `tbl-` identity nor splits its images across
  // several batches adds no figure of its own, so whatever the code printed
  // keeps its own numbering.
  let wraps-output = (
    tbl-labels.len() > 0
      or tbl-caption != none
      or (
        (fig-labels.len() > 0 or fig-caption != none) and _image-group-count(items) > 1
      )
  )
  let opts = if self-labels {
    opts + ("typst-labels": identity-labels)
  } else if own-figures > 0 and wraps-output {
    opts + ("typst-demote-figures": true)
  } else {
    opts
  }

  let interleave = source-block != none and _interleaves-source(items, opts, chunk)
  let items = if interleave { items } else { _without-source-items(items) }
  // Without interleaving, the echo is one block ahead of the output, which is
  // where a chunk has always put it.
  let echoed = if source-block != none and not interleave {
    let code = if source-code != none { source-code } else { chunk.at("source", default: "") }
    source-block(code)
  }
  let segment-source = if interleave { source-block } else { none }

  // The chunk's own figure already carries the labels, so no wrapper is added
  // and no batch claims them a second time.
  if self-labels {
    return {
      echoed
      _render-item-sequence(items, label, opts, anchor, source-block: segment-source)
    }
  }

  // A `tbl-` label names the chunk's non-image output as a table. Wrap the
  // whole rendered sequence once, the same way a split figure is wrapped: the
  // inner batches keep their own figure handling for any images the chunk also
  // produced, and the table figure encloses the result.
  if tbl-labels.len() > 0 or tbl-caption != none {
    let body = _render-figure-sequence(
      items,
      label,
      opts,
      fig-labels,
      anchor,
      source-block: segment-source,
    )
    return {
      echoed
      _table-figure(body, tbl-caption, tbl-labels, opts, anchor)
    }
  }

  echoed
  _render-figure-sequence(items, label, opts, fig-labels, anchor, source-block: segment-source)
}
