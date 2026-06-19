# Side notes and side figures for the academic theme

Date: 2026-06-18
Status: Approved design, ready for implementation planning

## Summary

Replace the academic theme's automatic footnote rewire with two first-class,
universal runtime elements: `calepin.elements.sidenote` and
`calepin.elements.sidefigure`. Footnotes go back to rendering as plain
footnotes. Placing content in the margin becomes an explicit author choice.
Paged (PDF) output places notes and figures with the
[marginalia](https://typst.app/universe/package/marginalia) package; HTML output
uses the academic theme's own CSS.

## Motivation

The academic theme currently runs `enhanceFootnotes()` in `scripts/main.js`,
which moves Typst endnotes into the margin as sidenotes. That couples every
footnote to the margin and gives authors no way to keep a normal footnote. It
also means the "Tufte look" is a side effect of footnotes rather than a
deliberate element.

We want the opposite: footnotes stay footnotes, and authors opt in to the margin
with explicit helpers that work in both targets.

## Decisions

These were settled during brainstorming and are fixed for this design:

1. Two helpers, mirroring marginalia's `note` / `notefigure` split:
   `sidenote` (text) and `sidefigure` (figures).
2. `sidenote` is numbered by default and suppressible: `numbering: none`
   produces an unnumbered margin note.
3. HTML `sidenote` is linked and static, with no JavaScript: a real anchor
   marker plus an id-bearing margin span, associated for assistive tech.
4. Paged output adopts a wide outer margin for all academic documents via
   `marginalia.setup`.
5. The helpers are universal `calepin.elements.*` modules (like `columns`),
   available under every theme. The marginalia dependency stays confined to the
   academic theme; under any other theme or a bare document the helpers degrade
   to a plain footnote / inline figure.

## Author-facing API

Mirrors marginalia's `note` and `notefigure`:

```typst
#calepin.elements.sidenote(numbering: auto, side: auto, label: none)[body]
#calepin.elements.sidefigure(caption: none, side: auto, label: none)[body]
```

- `sidenote`
  - `numbering`: `auto` (default) numbers the note and emits an in-text marker;
    `none` makes it an unnumbered margin note.
  - `side`: forwarded to marginalia in paged output (`"inner"`/`"outer"`/
    `"left"`/`"right"`/`"auto"`).
  - `label`: optional identifier for cross-referencing and stable anchors.
- `sidefigure`
  - `caption`: optional caption rendered below the figure.
  - `side`, `label`: as above.

## Runtime elements

New modules `runtime/elements/sidenote.typ` and
`runtime/elements/sidefigure.typ`, re-exported from `runtime/elements/mod.typ`.
They follow the existing element shape (see `elements/columns.typ`), branching on
the predicates in `core/target.typ`:

- `_is-query()`: return the body unchanged, so chunk metadata extraction is
  unaffected.
- `_is-html()`:
  - `sidenote`, numbered: emit a linked marker
    `<a role="doc-noteref" href="#sn-N" id="snref-N">N</a>` immediately followed
    by `<span class="sidenote" id="sn-N">...</span>`.
  - `sidenote`, `numbering: none`: emit `<span class="marginnote">...</span>`
    with no marker.
  - `sidefigure`: emit `<figure class="margin-figure">...<figcaption>caption
    </figcaption></figure>` (the `figcaption` only when `caption != none`).
  - `N` is drawn from a runtime sequence counter added to `core/state.typ` and is
    the single source of truth: it is the visible marker text and the index in
    the `href`/`id` pair. CSS styles the marker as a superscript but does not
    generate the number via a CSS counter, so the displayed number and the
    anchor target cannot desync.
- paged: defer to a hook (see below). Default behavior is `footnote(body)` for
  `sidenote` and an inline `figure(body, caption: caption)` for `sidefigure`.

### Paged placement hook

Add a `state` to `core/state.typ`, for example `_margin-impl`, holding a
dictionary of placement functions with keys `note` and `figure`. Defaults:

- `note`: `(body, numbering: auto, side: auto) => footnote(body)`
- `figure`: `(body, caption: none, side: auto) => figure(body, caption: caption)`

The paged branch of each element reads the hook via `context _margin-impl.get()`
and calls the relevant function. A theme installs margin placement by updating
this state before the document body runs.

## Academic theme: paged output

In `notebook.typ.jinja` (the academic paged entry, currently a raw-block show
rule plus the body):

1. Import marginalia, pinned to `0.2.0` (the version verified during
   implementation; it provides `setup`, `note`, and `notefigure`).
2. Apply `#show: marginalia.setup.with(...)` configured for a wide outer margin,
   so notes and figures have room and the PDF matches the HTML gutter. Starting
   geometry is a tuning detail; pick an outer margin wide enough for the note
   column and adjust during implementation.
3. `_margin-impl.update(...)` to install marginalia-backed placement:
   - `note`: call `marginalia.note(body, numbering: ..., side: ...)`, passing the
     element's `numbering`/`side` through (`numbering: none` suppresses the
     marginalia number).
   - `figure`: call `marginalia.notefigure(body, caption: caption, side: ...)`.

Because the template runs before `{{ document.body }}`, author calls in the body
observe the installed implementation.

This is also why `sidenote` is a distinct element rather than a `show footnote`
rule: real footnotes must remain footnotes, so we cannot intercept all footnotes
and send them to the margin.

## Academic theme: HTML output

In `styles/main.css`:

- Rewrite the margin block to style three classes, all floated into the existing
  `--academic-margin-width` gutter with the current narrow-screen inline
  fallback:
  - `.sidenote`: CSS-counter superscript marker, brick-red, numbered.
  - `.marginnote`: unnumbered, no marker.
  - `.margin-figure`: figure in the gutter, with figure/table/output child
    resets so a block fills the gutter cleanly.
- Remove the `.academic-footnote` and `.academic-footnote-backref` rules.

In `scripts/main.js`:

- Remove `enhanceFootnotes()` and its call. Keep the navigation-toggle code.

No new JavaScript is added; HTML sidenotes are pure markup plus CSS.

## Footnote behavior change

With the rewire removed, a plain `footnote[...]` renders as:

- Paged: a normal bottom-of-page footnote.
- HTML: Typst's native endnotes section, unmoved.

The academic theme keeps light styling for the native endnotes section.

## Fallbacks

- Under any non-academic theme or a bare document, `sidenote` degrades to a real
  `footnote` and `sidefigure` to an inline figure, via the default hook. An
  unnumbered `sidenote` (`numbering: none`) still shows a footnote number in this
  fallback, since plain footnotes are always numbered; this is acceptable
  degradation.
- No marginalia download is imposed on non-academic paged compiles.
- On narrow screens, the HTML margin classes collapse inline (existing academic
  responsive behavior).

## Testing

Behavior-focused only; no assertions on exact layout, generated source strings,
or byte output.

- HTML, academic theme: a document using `sidenote` produces output where the
  note content is present in a `.sidenote` container associated with a
  `doc-noteref` marker.
- HTML, academic theme: a plain `footnote` is NOT hoisted into the margin,
  proving the rewire is gone.
- HTML, academic theme: `sidefigure` caption text is present in the output.
- Default (non-academic) theme: `sidenote` compiles and its text appears as a
  footnote; no marginalia is required.
- Query pass: the elements return their body so chunk extraction is unaffected.
- Paged / marginalia integration tests shell out to real `typst` plus the
  marginalia package and return early (skip, not fail) when `typst` or the
  package is unavailable, matching the existing integration-test skip pattern.

## Out of scope

- marginalia `wideblock` / full-width spanning blocks.
- `@label` cross-references to notes.
- Migrating the staged `tufte/` files or the tufte starter example (separate
  follow-ups).

## Affected files

- `calepin/src/assets/typst-runtime/elements/sidenote.typ` (new; holds the
  sidenote sequence counter)
- `calepin/src/assets/typst-runtime/elements/sidefigure.typ` (new)
- `calepin/src/assets/typst-runtime/elements/margin.typ` (new; the paged
  placement hook state and `set-margin-impl`)
- `calepin/src/assets/typst-runtime/elements/mod.typ` (re-export sidenote,
  sidefigure, set-margin-impl)
- `calepin/src/assets/themes/academic/notebook.typ.jinja` (marginalia setup + hook install)
- `calepin/src/assets/themes/academic/styles/main.css` (new margin classes, remove footnote classes)
- `calepin/src/assets/themes/academic/scripts/main.js` (remove enhanceFootnotes)
- Tests under `calepin/tests/` and runtime tests for the new elements
- Docs page covering the new elements (follow-up)
