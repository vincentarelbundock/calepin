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
