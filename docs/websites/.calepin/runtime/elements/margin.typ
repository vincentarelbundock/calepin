// Paged placement hook for margin elements (`sidenote`, `sidefigure`).
//
// The default implementation keeps margin content visible without any external
// dependency: a sidenote becomes a real footnote and a side figure becomes an
// ordinary inline figure. A theme (for example `academic`) installs its own
// implementation with `set-margin-impl`, typically backed by the marginalia
// package, before the document body runs.

#let _default-note(body, numbering: auto, side: auto) = footnote(body)

#let _default-figure(body, caption: none, side: auto) = figure(body, caption: caption)

#let _margin-impl = state(
  "calepin-margin-impl",
  (note: _default-note, figure: _default-figure),
)

#let set-margin-impl(note: none, figure: none) = {
  _margin-impl.update(current => {
    let updated = current
    if note != none {
      updated.insert("note", note)
    }
    if figure != none {
      updated.insert("figure", figure)
    }
    updated
  })
}
