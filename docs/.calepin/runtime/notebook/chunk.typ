#import "../core/config.typ": _runtime-config
#import "../core/results.typ": _result-chunk, _results-document
#import "../core/target.typ": _is-query
#import "chunk-support.typ": _derive-label, _disable-raw-chunk-transforms, _without-raw-chunk-transforms
#import "chunk-support.typ": _raw-node, _raw-text, _relocate-opts, _sync-auto-label-counter
#import "code.typ": _html-themed-raw-block, _input-block
#import "defaults.typ": _auto-inline-label-index, _base-options, _call-defaults
#import "options.typ": _resolve-options-for
#import "render.typ": _render-results, _results-hidden


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
  out.insert("store-get", options.at("store-get"))
  out.insert("store-set", options.at("store-set"))
  out
}

// The query pass evaluates the document before any results exist, so a label
// that only gets attached during the render pass would be undefined here and
// every `@ref` to it would fail the query. Emit an empty figure of the matching
// kind per label so the reference resolves in both passes.
#let _query-placeholder-kinds = (
  "fig-": image,
  "tbl-": table,
  "lst-": raw,
)

// An `lst-` label names the chunk's echoed source as a listing. The labels here
// are the raw names from the chunk header, not the classified docs the results
// document carries, so match on the prefix.
#let _listing-names(crossref-labels) = {
  let out = ()
  for name in crossref-labels {
    if type(name) == str and name.starts-with("lst-") {
      out.push(name)
    }
  }
  out
}

// Render the echoed source, wrapped as a listing figure when the chunk carries
// an `lst-` label or caption. Without either, the code renders exactly as it
// did before, with no figure and no counter consumed.
#let _listing-block(code, engine, label, crossref-labels, options) = {
  let block = _input-block(code, lang: engine)
  let names = _listing-names(crossref-labels)
  let caption = options.at("lst-caption", default: none)
  if names.len() == 0 and caption == none {
    return block
  }
  let rendered = figure(block, kind: raw, caption: caption)
  for name in names {
    rendered = [#rendered #std.label(name)]
  }
  rendered
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

// The panel count for a `#|` header chunk. Typst reads only `label` out of the
// header (the rest is parsed on the Rust side, after this pass), but the query
// pass needs the count to emit one placeholder per referenceable panel.
#let _qmd-subcaption-count(body) = {
  let code = _raw-text(body)
  let code = if code.starts-with("\n") { code.slice(1) } else { code }
  for line in code.split("\n") {
    let trimmed = line.trim()
    if not trimmed.starts-with("#|") {
      return 0
    }
    let directive = trimmed.slice(2).trim()
    let colon = directive.position(":")
    if colon == none {
      continue
    }
    let key = directive.slice(0, colon).trim()
    if key in ("fig-subcap", "fig-subcaptions", "fig.subcap") {
      let value = _parse-qmd-label-value(directive.slice(colon + 1))
      return if type(value) == array { value.len() } else { 0 }
    }
  }
  0
}

// How many panels a `fig-` label can address. The panels themselves only exist
// once the chunk has run, which is after this pass, so the sub-caption list is
// the one count available here: a document that references a panel captions it.
#let _query-panel-count(options, body) = {
  let subcaptions = options.at("fig-subcaptions", default: none)
  let from-options = if type(subcaptions) == array { subcaptions.len() } else { 0 }
  calc.max(from-options, _qmd-subcaption-count(body))
}

#let _query-crossref-placeholders(crossref-labels, options, body) = {
  let out = []
  let panels = _query-panel-count(options, body)
  for name in crossref-labels {
    if type(name) != str {
      continue
    }
    for (prefix, figure-kind) in _query-placeholder-kinds {
      if name.starts-with(prefix) {
        out += [
          #figure(box(width: 0pt, height: 0pt), kind: figure-kind, caption: none)
          #label(name)
        ]
        if prefix == "fig-" {
          for index in range(panels) {
            out += [
              #figure(box(width: 0pt, height: 0pt), kind: figure-kind, caption: none)
              #label(name + "-" + str(index + 1))
            ]
          }
        }
      }
    }
  }
  out
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

// `fallback` renders `body` when it turns out not to be a chunk after all,
// because no engine was available to run it. Callers that are themselves `raw`
// show rules must not hand the element straight back — that would re-enter the
// same rule — so the default re-renders it through Calepin's code styling.
#let _emit-chunk(config, engine, body, fallback: _html-themed-raw-block, ..args) = context {
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
      #_query-crossref-placeholders(crossref-labels, options, body)
    ]
  } else {
    let code = _raw-text(body)
    let code = if code.starts-with("\n") { code.slice(1) } else { code }
    let code = _strip-qmd-header(code)
    let options = _resolve-options-for(options)
    let runtime-config = _runtime-config(bound: config)
    let results-path = runtime-config.at("results", default: none)
    // The runtime parses only `#| label` from the fence header itself; the
    // remaining `#|` options are parsed during the query/execute pass and
    // stored in results.json. Fold the display toggles back in so directives
    // like `#| echo: false`, `#| results: hide`, or `#| warning: false` drive
    // the runtime's own gates below (and the paged renderer, which otherwise
    // forwards only figure options). Figure-layout keys are deliberately left
    // to `_render-results`, which converts their JSON form for the active
    // target. Function-call options already match what is stored, so this is a
    // no-op for them.
    let chunk = if results-path != none and results-path != "" {
      _result-chunk(_results-document(config: runtime-config), label)
    } else {
      none
    }
    // A fenced block names a language, which is not a promise that Calepin can
    // run it: anything outside the built-in engines is looked up as a Jupyter
    // kernel, and prose fences such as ```rust or ```json usually have none.
    // Execution reports that back as `unavailable`, and a block Calepin cannot
    // run is not a chunk — hand it to whatever styles ordinary fenced code
    // here, which is how a package like codly keeps its own blocks. The label
    // counter still steps, so this decision cannot pull the automatic `chunk-N`
    // numbering out of step with the query pass (issue #108).
    if chunk != none and chunk.at("status", default: "") == "unavailable" {
      label-step
      // `body` may be the wrapper content of an explicit `calepin.chunk` call;
      // the fallbacks all style a raw element, so hand them the fence itself.
      fallback(_raw-node(body))
    } else {
      if chunk != none {
        let stored-source = chunk.at("source", default: "")
        if stored-source != "" {
          code = stored-source
        } else {
          // Results written before canonical source was stored can still
          // contain the tail of a dotted engine version at the start of the
          // raw body (for example `.2` for `julia-1.2`). Recover it from the
          // already-canonical engine name instead of duplicating Rust's
          // language/version parser in Typst.
          let canonical-engine = chunk.at("engine", default: engine)
          if canonical-engine != engine and canonical-engine.starts-with(engine + ".") {
            let suffix = canonical-engine.slice(engine.len())
            if code.starts-with(suffix + "\n") {
              code = code.slice(suffix.len() + 1)
            }
          }
        }
        let stored = chunk.at("options", default: (:))
        for key in ("echo", "results", "warning", "message") {
          if key in stored {
            options.insert(key, stored.at(key))
          }
        }
      }
      let show-echo = options.at("echo") == true
      let results-mode = options.at("results")
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
        _listing-block(code, engine, label, crossref-labels, options)
      } else if results-path == none or results-path == "" {
        _listing-block(code, engine, label, crossref-labels, options)
      }
      // `results: "hide"`/`"hidden"` runs the chunk but renders nothing here; the
      // output can still be shown elsewhere with `#calepin.results(label)`.
      if results-path != none and results-path != "" and not _results-hidden(results-mode) {
        _render-results(label, options, anchor: true, config: runtime-config)
      }
    }
  }
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

// The single entry point for a fenced block, whether it reached us from the
// staged source rewrite or from a `raw` show rule. Both passes must take the
// same branch here: a block that emits chunk metadata during the query pass but
// not during the render pass desynchronises the automatic `chunk-N` counter, so
// every later chunk renders its predecessor's source (issue #108). Nothing in
// this decision may therefore depend on the pass.
#let _fenced-chunk(config, engine, it, fallback: _html-themed-raw-block) = context {
  let defaults = _resolve-options-for(_call-defaults)
  if _fenced-chunks-runs(engine, defaults.at("fenced-chunks")) {
    _emit-chunk(config, engine, it, fallback: fallback, ..defaults)
  } else {
    fallback(it)
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
#let _chunk(config, ..args) = {
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
  _without-raw-chunk-transforms(() => _emit-chunk(config, engine, body, ..args.named()))
}

#let chunk(..args) = _chunk(none, ..args)

#let _inline(config, engine, body, ..args) = {
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
  _chunk(config, engine, body, ..(defaults + opts))
}

#let inline(engine, body, ..args) = _inline(none, engine, body, ..args)

// Options a `#calepin.results` relocation may override. These are exactly the
// options the renderer consults; execution options (`eval`, `script`, the
// `fig-device-*` family) are settled when the chunk runs and cannot be changed
// after the fact, so passing one is an error rather than a silent no-op.
#let _relocation-options = (
  "results",
  "inline-output",
  "warning",
  "message",
  "fig-width",
  "fig-height",
  "fig-align",
  "fig-responsive",
  "fig-link",
  "fig-caption",
  "fig-cap-location",
  "fig-alt-text",
  "fig-subcaptions",
  "fig-layout-columns",
  "fig-layout-rows",
)

#let _relocation-overrides(named) = {
  let out = (:)
  for (key, value) in named.pairs() {
    if key == "label" {
      continue
    }
    if key not in _relocation-options {
      panic(
        "calepin.results: `"
          + key
          + "` cannot be overridden here; accepted options are "
          + _relocation-options.join(", "),
      )
    }
    // `auto` keeps whatever the chunk itself resolved.
    if value != auto {
      out.insert(key, value)
    }
  }
  out
}

// Render a chunk's output at this location instead of (or in addition to) the
// chunk's own position. Pair it with `results: "hide"` or `results: "hidden"`
// on the chunk to move the output elsewhere. The label is given positionally
// (`calepin.results("foo")`) or named (`calepin.results(label: "foo")`).
// The relocated rendering mode defaults to `calepin.setup(results: ...)` for a
// hidden chunk. Any display option accepted by `calepin.chunk` can be given
// here to override the chunk's own choice for this rendering, directly or
// through Typst's `.with()`; `auto` means "inherit from the chunk".
//
// A cross-reference anchor (a `fig-`/`tbl-`/`lst-` label) lives wherever the
// figure is shown: at the chunk's own position when it is visible, and here when
// the source chunk is hidden. Referencing a figure that is shown in more than
// one place is ambiguous, and Typst reports it as a duplicate-label error.
#let _results(config, ..args) = {
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
      let runtime-config = _runtime-config(bound: config)
      let results-path = runtime-config.at("results", default: none)
      if results-path != none and results-path != "" {
        let chunk = _result-chunk(_results-document(config: runtime-config), label)
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
        let overrides = _relocation-overrides(named)
        // A hidden chunk spends its own `results` slot on hiding, so an
        // unspecified relocation falls back to the document-wide default
        // rather than to the chunk's `"hide"`.
        if "results" not in overrides and hidden {
          let defaults = _resolve-options-for(_call-defaults)
          overrides.insert("results", defaults.at("results"))
        }
        _render-results(
          label,
          opts,
          anchor: hidden,
          overrides: overrides,
          config: runtime-config,
        )
      }
    }
  }
}

#let results(..args) = _results(none, ..args)
