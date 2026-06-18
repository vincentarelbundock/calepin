# Relocated chunk output (`#calepin.results`)

Date: 2026-06-17
Status: approved, implementing

## Problem

Calepin renders a code chunk's output inline, right where the chunk is written.
Authors sometimes want to compute output in one place and display it somewhere
else: run a plot near the data-loading code but print the figure later in the
document, or print the same result in more than one location. Tools like
knitr and Quarto support this through deferred or relocated output.

## Goal

Let an author suppress a chunk's inline output and render that output elsewhere,
any number of times, by referring to the chunk's label.

```typst
// source chunk: runs, but renders nothing inline
#calepin.chunk(label: "myplot", results: "hide")[```python
import matplotlib.pyplot as plt
plt.plot([1, 2, 3])
plt.show()
```]

... prose ...

// render that chunk's full output here
#calepin.results("myplot")
```

## Design

### Public API: `#calepin.results`

A new public Typst function, exported on the `calepin` facade.

- The label is accepted positionally (`#calepin.results("myplot")`) or named
  (`#calepin.results(label: "myplot")`).
- It renders the chunk's entire stored output as-is, reusing the existing
  `_render-results(label, opts)` renderer. No new rendering code: streams,
  figures, warnings, and messages appear exactly as they would inline.
- It may be called any number of times, anywhere in the document, including
  before the source chunk appears in the file. Forward references work because
  `results.json` is fully built before the render pass runs.
- It takes no other arguments. Cross-reference anchor placement is automatic
  (see below).

### Plain labels as chunk ids

To relocate a chunk you must be able to name it. Previously an explicit chunk
label had to carry a `fig-`/`tbl-`/`lst-` cross-reference prefix; a plain label
such as `label: "summary"` was rejected. That is relaxed: a label without a
recognized prefix is now a valid chunk id usable by `#calepin.results`, while
prefixed labels still route to cross-references. `@summary` does not resolve for
a plain label (it is an id, not a cross-reference), which the cross-references
docs now state. Two query tests that asserted rejection were updated to assert
acceptance as a plain id.

Hiding inline output and relocating it are independent, composable knobs.
A chunk that is not hidden can still be relocated; the output then appears both
inline and at each relocation.

### `results: "hide"` suppresses all inline output

`results: "hide"` is redefined to mean "render nothing inline for this chunk":
text streams and figures alike. The chunk still executes and still writes its
results to `results.json`; only the inline render is skipped. Implementation:
when `results == "hide"`, the chunk's render branch skips the inline
`_render-results` call entirely.

This changes prior behavior, where `hide` suppressed only text streams and still
rendered figures inline. The single intuitive meaning of "hide" (show nothing)
is preferred over knitr's split between `results='hide'` (text) and
`fig.show='hide'` (figures).

### Cross-reference anchor ownership

A chunk may attach a figure, table, or listing anchor through a prefixed label
(for example label `fig-myplot`, referenced as `@fig-myplot`). Typst requires
each label to be defined exactly once.

The anchor follows the figure, with no flag and no document-wide bookkeeping:

- The inline render carries the anchor when the chunk is shown in place (not
  hidden), exactly as a normal chunk does today.
- A `#calepin.results` relocation carries the anchor when the source chunk is
  hidden (and so renders nothing at its own position); otherwise it renders the
  output without defining a label.

So `@fig-myplot` resolves to wherever the figure is actually shown: its own
position when visible, or the relocation when hidden. No argument is needed for
the common case. If the same figure is shown in more than one place and then
referenced, the label is defined twice and Typst reports "label `<...>` occurs
multiple times" when the reference is resolved. There is no custom detection:
the duplicate-label error is Typst's own, and an unreferenced repeat is fine.

### Implementation approach: Typst state, no Rust schema change

The relocated copy must render with the chunk's exact display settings, but
`results.json` stores Rust-resolved options where, for example, `fig-width`
becomes the string `"70%"` rather than a Typst ratio. That string is fine for
HTML but breaks paged `image(width:)`. The inline render avoids this because it
passes Typst-native resolved options straight to the renderer.

The implementation keeps that fidelity entirely in the Typst runtime, so no Rust
data-model or query changes are needed:

1. Each chunk, during its render-pass evaluation, stashes its resolved display
   options into a `state` keyed by label (`_relocate-opts`). Only render
   relevant keys are kept, so the value stays plain data.
2. A `#calepin.results(label)` call reads `_relocate-opts` with `.final()`, which
   returns the document-final value and therefore works regardless of source
   order (forward references fall out of this). It looks up the chunk's items in
   `results.json`, renders them with the stashed options, and attaches the
   cross-reference anchor only when the source chunk is hidden.

`_render-results` gained an `anchor` flag: when false, it attaches neither the
cross-reference labels nor the chunk's internal-id label, so the same output can
appear more than once without defining a Typst label twice. It also treats
`results: "hide"` as "show it here" once reached, since hiding is handled by the
chunk skipping its own inline render.

The wiring is small: the new `results` function lives alongside `chunk` in
`notebook/chunk.typ`, `_relocate-opts` lives in `core/state.typ`, and the
generated facade in `runtime.rs` exports `#let results = chunks.results`.

### Error handling

- Missing or misspelled label: clear error, matching the existing inline path
  that panics when a label is absent from `results.json`.
- Same figure shown in two places and then referenced: Typst's own duplicate-label
  error when the reference is resolved.
- Forward reference (relocation appears before its chunk): supported, no error.

## Testing

Behavior-focused tests only, no layout, generated-source, or byte-output pins:

- A hidden chunk plus one relocation renders the output once, at the relocation,
  and not inline.
- Relocating the same chunk twice renders the output twice.
- A relocated figure's `@fig-...` reference resolves when the chunk is hidden and
  relocated once.
- Referencing a figure printed in two places produces Typst's duplicate-label
  error; printing it twice without referencing it compiles.
- A missing label errors.
- A forward reference (relocation before its chunk) renders correctly.

## Out of scope

- Display-option overrides on the relocation call (caption, layout, echo). The
  relocated copy renders with the source chunk's stored options. This can be
  added later without changing the API shape.
- Relocating the chunk's source code (knitr `ref.label` style code reuse).
