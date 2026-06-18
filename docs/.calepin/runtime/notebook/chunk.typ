#import "../core/state.typ": _auto-inline-label-index, _base-options, _call-defaults
#import "../core/state.typ": _derive-label, _disable-raw-chunk-transforms
#import "../core/state.typ": _raw-node, _raw-text, _sync-auto-label-counter
#import "../core/state.typ": _relocate-opts
#import "../core/target.typ": _is-query
#import "render.typ": _html-themed-raw-block, _input-block, _render-results, _results-hidden
#import "options.typ": _resolve-options


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
  if _is-query() {
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
    let results-mode = options.at("results")
    let results-path = sys.inputs.at("calepin-results", default: "")
    label-step
    [#metadata((label: label, page: here().page())) <calepin-page>]

    // Stash the resolved display options so a `#calepin.results(label)` call
    // placed elsewhere can render this chunk with identical settings. Only the
    // render-relevant keys are kept so the stored value stays plain data.
    let stashed = (:)
    for key in _base-options.keys() {
      stashed.insert(key, options.at(key))
    }
    stashed.insert("inline-output", options.at("inline-output"))
    _relocate-opts.update(reg => {
      reg.insert(label, stashed)
      reg
    })

    if show-echo {
      _input-block(code, lang: engine)
    } else if results-path == "" {
      _input-block(code, lang: engine)
    }
    // `results: "hide"`/`"hidden"` runs the chunk but renders nothing here; the
    // output can still be shown elsewhere with `#calepin.results(label)`.
    if results-path != "" and not _results-hidden(results-mode) {
      _render-results(label, options, anchor: true)
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

#let chunk_from_raw_plain(engine, it) = context {
  let defaults = _resolve-options(engine, _call-defaults)
  if _fenced-chunks-runs(engine, defaults.at("fenced-chunks")) {
    _emit-chunk(engine, it, ..defaults)
  } else {
    _html-themed-raw-block(it)
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

// Render a chunk's output at this location instead of (or in addition to) the
// chunk's own position. Pair it with `results: "hide"` or `results: "hidden"`
// on the chunk to move the output elsewhere. The label is given positionally
// (`calepin.results("foo")`) or named (`calepin.results(label: "foo")`).
//
// A cross-reference anchor (a `fig-`/`tbl-`/`lst-` label) lives wherever the
// figure is shown: at the chunk's own position when it is visible, and here when
// the source chunk is hidden. Referencing a figure that is shown in more than
// one place is ambiguous, and Typst reports it as a duplicate-label error.
#let results(..args) = {
  let positional = args.pos()
  let named = args.named()
  let label = if named.at("label", default: none) != none {
    named.at("label")
  } else if positional.len() >= 1 {
    positional.at(0)
  } else {
    panic("calepin.results: provide a chunk label, e.g. calepin.results(\"my-label\")")
  }
  if type(label) != str {
    panic("calepin.results: label must be a string")
  }
  if _is-query() {
    // Nothing to emit in the query pass; rendering happens during the render pass.
  } else {
    context {
      let results-path = sys.inputs.at("calepin-results", default: "")
      if results-path != "" {
        let results-doc = json(results-path)
        let chunk = results-doc.at("chunks", default: (:)).at(label, default: none)
        if chunk == none {
          panic("calepin.results: no chunk is labeled `" + label + "`")
        }
        let opts = _relocate-opts.final().at(label, default: none)
        if opts == none {
          panic("calepin.results: cannot find a chunk labeled `" + label + "` to relocate")
        }
        // The anchor follows the figure: attach it here only when the source
        // chunk is hidden (and so renders nothing at its own position).
        let hidden = _results-hidden(opts.at("results", default: "render"))
        _render-results(label, opts, anchor: hidden)
      }
    }
  }
}
