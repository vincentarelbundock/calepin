
#let _chunk-spec(body, engine, label, options) = {
  let out = (
    body: body,
    engine: engine,
    label: label,
  )
  for key in _base-options.keys() {
    out.insert(key, options.at(key))
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
  let label = options.at("label")
  let generated-label = label == none
  let auto-label-state = options.at("auto-label-state")
  let auto-label-prefix = options.at("auto-label-prefix")
  let label = if generated-label { auto-label-prefix + "-" + str(auto-label-state.get()) } else { label }
  let label-step = if generated-label {
    auto-label-state.update(n => n + 1)
  } else {
    _sync-auto-label-counter(auto-label-state, label)
  }
  if _mode == "query" {
    [#label-step #metadata(_chunk-spec(body, engine, label, options)) <calepin-chunk>]
  } else {
    let code = _raw-text(body)
    let code = if code.starts-with("\n") { code.slice(1) } else { code }
    let code = _strip-lang-version-suffix(engine, code)
    let options = _resolve-options(engine, options)
    let show-echo = options.at("echo") == true
    let results-path = sys.inputs.at("calepin-results", default: "")
    let display-lang = if options.at("source-lang") == auto { engine } else { options.at("source-lang") }
    label-step
    [#metadata((label: label, page: here().page())) <calepin-page>]

    if options.at("output") == false {
      none
    } else {
      if show-echo {
        _input-block(code, lang: display-lang)
      } else if results-path == "" {
        _input-block(code, lang: display-lang)
      }
      if results-path != "" {
        _render-results(label, options)
      }
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

// `raw-chunks` is the single switch for auto-running plain fenced blocks:
// `true` (every engine), an engine name, or a list of engine names.
#let _raw-chunks-runs(engine, setting) = {
  if setting == true {
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
  if _raw-chunks-runs(engine, defaults.at("raw-chunks")) {
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
    source-lang: none,
    echo: false,
    inline-output: true,
    auto-label-prefix: "inline",
    auto-label-state: _auto-inline-label-index,
  )
  chunk(engine, body, ..(defaults + opts))
}
