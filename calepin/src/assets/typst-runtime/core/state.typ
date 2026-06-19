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
#let _prefix-page-entry(entry, prefix) = {
  let entry = entry
  if type(entry.at("href", default: none)) == str {
    entry.insert("href", prefix + entry.href)
  }
  if type(entry.at("pdf", default: none)) == str {
    entry.insert("pdf", prefix + entry.pdf)
  }
  if type(entry.at("translations", default: none)) == dictionary {
    let translations = (:)
    for (language, href) in entry.translations.pairs() {
      translations.insert(language, if type(href) == str { prefix + href } else { href })
    }
    entry.insert("translations", translations)
  }
  entry
}

#let pages() = {
  if _pages-index-path == "" { return () }
  let prefix = _site-root-prefix()
  json(_pages-index-path).map(entry => _prefix-page-entry(entry, prefix))
}

#let _base-options = (
  echo: true,
  eval: true,
  results: "render",
  warning: true,
  message: true,
  error: false,
  placeholder: auto,
  "code-size": none,
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

// Deferred output. A chunk stashes its resolved render options here keyed by
// label, so a `#calepin.results(label)` call placed elsewhere can render the
// same output with identical settings. Read with `.final()` so the lookup works
// regardless of document order (a relocation may appear before its chunk).
#let _relocate-opts = state("calepin-relocate-opts", (:))

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
