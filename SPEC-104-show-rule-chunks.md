# Spec: make input/output chunks overridable with plain show rules

Addresses [#104](https://github.com/vincentarelbundock/calepin/issues/104) — "Disable
code-block wrapping when `theme="typst"` or allow code-block wrapper customization in
Typst".

## Goal

A document author should be able to restyle or remove Calepin's chunk chrome with an
ordinary Typst show rule, using no Calepin-specific API:

```typst
#show <calepin-input>: it => my-frame(it.body)   // restyle
#show <calepin-input>: it => it.body             // strip Calepin's box entirely
```

and `theme = "typst"` should mean *no Calepin chrome anywhere*, so that packages like
`codly` own code block rendering completely — while chunks still execute.

## Background: how chunk chrome is produced today

There are two independent paths, and only one of them is overridable.

**Path A — fenced blocks, via show rules.** A plain ```` ```python ```` block is turned
into an executable chunk by show rules that Calepin generates into `.calepin/wrapper.typ`
(`typst/preprocess/staging.rs:126-155`, `raw_show_rule` at `:192` and
`html_raw_show_rule` at `:214`), plus a `show raw.where(block: true)` rule in the theme
bundle's `layouts/pdf.typ`. This path is at least *visible* to users.

**Path B — direct calls, not overridable.** Echoed chunk source
(`notebook/chunk.typ:252,254` → `_input-block`) and *all* result output
(`notebook/render.typ:531,533,553,560,566` → `_output-block`) call the rendering
functions directly. They never pass through a show rule.

Both paths bottom out in `notebook/code.typ`:

- `code-block` (`:19-52`) emits a bare `block(fill: rgb("#f7f7f5"), stroke: …, radius: …,
  inset: …)`. It is an anonymous block — no selector can distinguish it from any other
  block in the document.
- `_raw-block` (`:14`) always receives an explicit `theme:` argument from its callers,
  overriding any user `show raw:` rule or `set raw(theme: …)`.

## Root cause of #104

`theme = "typst"` only selects `ThemeSelection::Typst`, which makes `notebook_source()`
return `None` (`theme/notebook.rs:68-70`) so no `layouts/pdf.typ` is injected. It has no
effect on Path B, which is where echoed source and output are rendered. Hence the
reporter sees Calepin's box around codly's box regardless of the theme setting.

One secondary defect falls out of the same cause: on the HTML target, `_source-block` and
`_output-block` emit `div.sourceCode` and `div.cell-output` (`code.typ:51,78`), so HTML
users already have a CSS escape hatch. Paged users have none. That asymmetry is the real
regression.

## Verified Typst behavior

These were checked against the installed `typst` binary (0.15.0), not inferred from docs.

- **Duplicate labels are fine.** Three blocks each carrying `<calepin-input>` compile and
  all fire the show rule. (`ref` is the only thing that objects to ambiguity.)
- **Show-rule nesting: later rules run outermost, and re-displaying `it` re-fires
  earlier rules.** Two transformational rules on the same selector do NOT replace each
  other when the later one re-emits `it`: the later rule runs first (outermost), and the
  earlier rule then fires on the re-displayed `it` (innermost). Consequences, all
  verified:
  - `#show <x>: it => it` after a default rule on `<x>` is a **no-op** — the default
    still renders.
  - `#show <x>: it => my-frame(it)` yields my-frame *around* the default chrome.
  - A later rule that **reconstructs from fields** without displaying `it`
    (e.g. `it => something(it.body)` or `it => raw(it.text, …)`) genuinely displaces the
    earlier rule, because the labeled element itself is never re-displayed.

  This is the load-bearing fact of the whole design: "override by show rule" only works
  if the idiom is reconstruction, never re-emission. A `block` carrier makes
  reconstruction a single field access (`it.body`); a `raw` carrier does not.
- **Show-set rules still compose.** `show raw: set text(fill: red)` does *not* displace a
  transformational `show <calepin-input>:` rule; both apply.
- **A document-wide transformational `show raw:` does not remove a block carrier's
  chrome.** It styles the *inner* raw; the labeled outer block (and its default rule)
  survive. Stripping Calepin chrome for codly therefore takes one explicit line (see
  user-facing contract), not zero — but that line actually works, on both targets.
- **Label show rules work under `--format html`**, including the `it.body`
  strip/restyle contract.

## Design

### 1. Semantic labels on `block` carriers

Emit five labeled `block` carriers instead of pre-styled blocks:

| Label | Emitted by | Carrier body |
|---|---|---|
| `<calepin-input>` | echoed chunk source | the `raw` element (syntax-themed) |
| `<calepin-output>` | stdout streams, `repr`/`str` results | monospaced text |
| `<calepin-warning>` | `diagnostic` items with `level: "warning"` | same as output |
| `<calepin-error>` | `error` items | same as output |
| `<calepin-result>` | `repr` results (`render.typ:531`) | same as output |

`<calepin-result>` vs `<calepin-output>`: adding a label later is cheap, but *splitting*
one later is a breaking change — documents styling `<calepin-output>` would silently
miss half their targets. Since the taxonomy is being defined once, define it at the
finest grain justifiable today: value-shaped results (`render.typ:531,533`) get
`<calepin-result>`, stream output (`:553`) gets `<calepin-output>`.

The input carrier is:

```typst
[#block[#raw(code, block: true, lang: lang, theme: _input-syntax-theme)] <calepin-input>]
```

Rationale for splitting warning from error: `render.typ:560-566` currently collapses both
into `stream: "stderr"`, leaving theme authors unable to distinguish a warning from a
failed chunk. (`level: "message"` diagnostics stay on the stdout path, as today.)

### 2. Why a `block` carrier, not the `raw` itself

Given the verified nesting behavior, replacement-by-show-rule is only achievable through
reconstruction. `block` exposes `.body`, so both the theme default and any user override
are one honest expression:

- Theme default: `show <calepin-input>: it => code-block(it.body)` — reconstructs, never
  re-emits `it`, so it cannot chain under a user rule.
- User strip: `it => it.body`. User restyle: `it => my-frame(it.body)`.

Labeling the `raw` element directly (the obvious alternative) fails twice over: users
cannot strip or restyle without hand-rebuilding `raw(it.text, block: true, lang: it.lang,
theme: …)`, and the "zero-config codly" benefit it promised rests on codly happening not
to re-display `it` — a `show raw: it => block(…)[#it]` from any other package or the user
reintroduces nested chrome.

The inner raw keeps carrying `theme: _input-syntax-theme`, so syntax highlighting
survives stripping. The `theme:` argument is no longer a recursion sentinel (see §4);
it is purely presentational.

Output is program stdout, not source code. Today it is routed through `raw()` purely to
get a monospace font, which is why codly frames the *output* too in the reporter's second
screenshot. Output carrier bodies must not be `raw` elements; use
`set text(font: …)` inside the block so `show raw:` does not reach them.

### 3. Defaults live in the theme bundle

Show rules in an imported module do not apply to the importing document, and
`calepin.setup()` is a plain state-updating function (`notebook/options.typ:6`), not a
`#show:` transform. So there is no place to install document-wide defaults except the
injected wrapper or the theme bundle.

Put them in `layouts/pdf.typ` of each builtin bundle (`assets/themes/calepin/`,
`assets/themes/academic/`). House rule, stated here because it is a correctness
condition, not a style preference: **every default rule reconstructs from `it.body` and
never displays `it`.** Consequences, all desirable:

- `theme = "typst"` skips template injection, so it yields bare unstyled carriers — the
  reporter's option 1, obtained structurally rather than as a special case. Chunks still
  execute, because the fenced-chunk detection rules in the wrapper are untouched (see
  staging.rs notes below).
- Theme authors restyle chunks by writing show rules in their own bundle.
- Document authors override with a later rule. Ordering: when a theme is present the
  user body is inlined at `{{ doc.body }}` *after* the theme preamble
  (`notebook_template_context` inlines the body, `staging.rs:36-43`; the `#include` path
  is the themeless one), so author rules are always defined later than theme rules —
  and because both sides reconstruct rather than re-emit, later-defined genuinely wins.

Bundles must **share** these defaults rather than duplicate them — but not via the theme
chain. `notebook_source()` resolves the paged layout with `find_notebook_template`
(`theme/notebook.rs:87-107`), which walks layers in reverse and returns the *first*
`layouts/pdf.typ` it finds. Paged resolution is winner-takes-all; it never concatenates
layers, so `academic` cannot inherit `calepin`'s show rules by sitting on top of it in
the chain. (`ThemeLayer` composition is real, but only `theme/html.rs` uses it, and only
for partials, styles, and scripts.)

The mechanism that does work is a **runtime-exported show transform**. A `#show` written
inside a module does not escape it, but a module may export a *function* that the
importer applies with `#show:`. Define the five default rules once in the runtime
(`notebook/code.typ`, re-exported from `.calepin/calepin.typ`) as
`default-chunk-chrome`, and have each bundle's `layouts/pdf.typ` opt in with one line:

```typst
#import "/.calepin/calepin.typ" as calepin
#show: calepin.default-chunk-chrome
```

This also absorbs the duplication that already exists: the entire
`show raw.where(block: true)` rule plus the `_calepin-body-size` state is **byte-identical**
between `assets/themes/calepin/layouts/pdf.typ:9-27` and `academic/layouts/pdf.typ:38-56`
today. Fold both into the same exported transform.

Properties this preserves:

- `theme = "typst"` still yields bare carriers — nothing invokes the transform, so
  opting in stays the bundle's decision rather than the runtime's.
- A user bundle gets Calepin's defaults by adding the one `#show:` line, or omits it for
  a structurally bare start. Neither is the accidental default.
- The transform's rules still reconstruct from `it.body`, so a document author's
  later-defined rule displaces them exactly as in the inline case.

This resolves former open question 1.

### 4. Recursion guard — the load-bearing wrinkle

The forced `theme:` argument is used as a sentinel in two places to prevent Calepin's own
rendered output from being re-processed by the fenced-chunk show rules:

- `staging.rs:195,215` — selectors are `raw.where(block: true, lang: …, theme: auto)`.
- `assets/themes/calepin/layouts/pdf.typ:11` / `academic:43` — `if it.theme != auto { … }`.

Under this design the sentinel is dead anyway: the moment a user rule emits `it.body`,
that raw is displayed wherever the user's transform puts it, and a syntactic sentinel on
the selector cannot know it came from Calepin. The guard must be the existing
`_disable-raw-chunk-transforms` state (`notebook/chunk-support.typ:1`) — the spec's
former "Option B" (keep the sentinel, add an opt-out) is not viable here and is dropped.

Both wrapper rules already consult the state (`staging.rs:139,195,218` patterns), and
`_without-raw-chunk-transforms` (`chunk.typ:264`) already implements the save/set/restore
pattern. `_input-block` and `_html-themed-raw-block` wrap their carrier emission in it;
the `theme: auto` clause drops out of the generated selectors.

Why this covers user transforms too: the state guard is *positional* (document
location), and the inner raw occupies the same location whether the default rule, a user
restyle, or a bare `it.body` displays it. A guard set around the carrier's emission in
`_input-block` therefore holds for every downstream transformation. Verified manually;
still the part of the change most likely to produce surprising behavior under layout
retries, and it needs the most test coverage.

The `it.theme != auto` branch in the bundle `pdf.typ` files also drives the 0.8×
body-relative sizing of Calepin-emitted raw. That detection moves to the same place as
the chrome: the default `show <calepin-input>:` / `<calepin-output>:` rules apply the
sizing, so the raw-level branch is deleted rather than re-guarded.

### 5. HTML target

Keep emitting `div.sourceCode` / `div.cell-output`, with the labeled block carrier
*inside* the div, so a single show rule works for both targets and the existing CSS hook
is unaffected. A user strip rule removes Calepin's block chrome but leaves the div, so
CSS classes remain addressable even for users who override in Typst.

## Changes by file

**`assets/typst-runtime/notebook/code.typ`** (114 lines; most of the work)
- `_input-block`: emit the labeled block carrier
  `[#block[#raw(code, block: true, lang: lang, theme: _input-syntax-theme)] <calepin-input>]`,
  unstyled, wrapped in the recursion guard.
- `_output-block`: replace the `stream:` string parameter with a semantic `kind:` of
  `"stdout" | "result" | "warning" | "error"`; emit a labeled `block`
  (`<calepin-output>` / `<calepin-result>` / `<calepin-warning>` / `<calepin-error>`)
  whose body is monospaced text, not a `raw`.
- `code-block`: keep it exported as the default styling helper — it becomes the body of
  the default show rules rather than something call sites invoke.
- `_paged-input-code-block` becomes dead; delete it. Check first that nothing else
  reaches for it or for `_paged-syntax-theme` (`00_syntax-theme.typ`) — if
  `_paged-syntax-theme` has no other consumer once the paged default rule owns
  highlighting, it goes with it.
- Export `default-chunk-chrome`, the show transform holding the five default rules plus
  the `_calepin-body-size` sizing treatment (§3), for bundles to apply with `#show:`.

**`assets/typst-runtime/notebook/render.typ`**
- Update the five `_output-block` call sites to pass `kind:` (`:531,533,553,560,566`).
- No signature changes elsewhere.

**`assets/typst-runtime/notebook/chunk.typ`**
- No change to `:252,254` beyond what `_input-block` does internally.
- Note that `_input-block` is bound at import time (`:6`), so the default styling must be
  reachable via show rule, never by rebinding the module-level name.

**`assets/themes/{calepin,academic}/layouts/pdf.typ`**
- Replace the duplicated `show raw.where(block: true)` rule and `_calepin-body-size`
  state (identical in both bundles) with a single `#show: calepin.default-chunk-chrome`
  line (§3). `academic` keeps only its own marginalia setup and numbering helpers.
- The five default rules, each reconstructing via `it.body` and each carrying the 0.8×
  body-size treatment formerly attached to the `it.theme != auto` branch, now live in
  the runtime transform, not in the bundles.
- The `it.theme != auto` guard becomes a `_disable-raw-chunk-transforms` state read (§4)
  inside that transform; the sizing branch is deleted.

**`typst/preprocess/staging.rs`**
- Drop `theme: auto` from the generated selectors (`raw_show_rule:192`,
  `html_raw_show_rule:214`); the state guard replaces the sentinel.
- **Do NOT gate the fenced-chunk rules on theme presence.** Those rules are the
  execution entry point (`chunk_from_raw_plain`) and feed the query pass's
  `<calepin-chunk>` detection; emitting them only when a notebook theme is present would
  silently stop fenced chunks from executing under `theme = "typst"`. Bareness under
  `theme = "typst"` comes from Path B emitting unstyled carriers and no default rules
  being injected — no staging-side condition needed.

**No Rust-side data model changes.** The hooks never become chunk options, so
`_base-options` stays JSON-serializable — which matters, because every key there is
serialized into `<calepin-chunk>` metadata at `chunk.typ:19-23` and parsed by the query
pass.

## User-facing contract

See "Documentation plan" below for where each piece of this lands on the website.

```typst
#show <calepin-input>: it => it.body             // strip Calepin's chrome; bare code
#show <calepin-input>: it => my-frame(it.body)   // restyle echoed source
#show <calepin-output>: it => box(it.body)       // restyle stdout
#show <calepin-result>: it => box(it.body)       // restyle returned values
#show <calepin-error>: it => text(red, it.body)  // errors distinct from warnings
```

For codly (or any package that installs `show raw:` rules), the recipe is two lines:

```typst
#import "@preview/codly:…": *
#show: codly-init
#show <calepin-input>: it => it.body   // hand the raw to codly, drop Calepin's box
```

Caveats to document:

- Overrides must use `it.body`, never `it`. Re-emitting `it` re-fires the default rule
  (Typst show-rule semantics), so `it => my-frame(it)` nests my-frame around Calepin's
  chrome and `it => it` changes nothing. This is worth a dedicated warning box.
- A rule installed *inside* a document template (`#show: touying-template`) may land
  after the author's own rules, so ordering under templates needs an explicit note.

### Stability guarantee

The label contract is semver-relevant API and the docs must say so explicitly: the five
label names, the guarantee that each labels a `block` whose `.body` is the bare unstyled
content, and the rule that all default chrome is applied via displaceable show rules are
stable across versions. Everything else about default styling (colors, strokes, insets,
`code-block`'s look) may change freely between versions without notice. This is what
lets the defaults evolve forever without breaking documents that override them.

## Documentation plan

The labels are public API, so they need a page of their own rather than a paragraph
buried in a how-to. Three places, each with a distinct job:

**1. `docs/themes/styling.typ` — new page, canonical reference.** Sidebar section
"Themes" (`docs/calepin.toml`), after `themes/templating.typ`. This is the page the
stability guarantee attaches to. Contents:

- The table of five labels: name, what emits it, what `.body` holds.
- The `it.body` idiom, with the "never `it`" warning as a callout — this is the single
  most likely user error and re-emitting `it` fails *silently* (`it => it` is a no-op,
  `it => my-frame(it)` nests). It earns a `#callout` box, not a sentence.
- The stability guarantee, stated as such: label names and carrier shape are
  semver-relevant; colors, strokes, insets, and `code-block`'s look are not.
- How theme bundles opt into the defaults (`#show: calepin.default-chunk-chrome`) and
  what omitting that line gives you.
- A note that `theme = "typst"` emits these carriers bare, with chunks still executing.

**2. `docs/notebooks/code_execution.typ` — pointer, not a duplicate.** A short
"Styling code blocks" section: one strip example, one restyle example, and a link to the
reference page. Readers arrive here asking "how do I run code", and discover the
override hook in passing.

**3. `docs/tips.typ` — the codly worked example** (see below). There is no FAQ page;
"Tips & tricks" is the de-facto one, and it already holds exactly this kind of
short recipe.

`docs/themes/pdf_templates.typ` should also gain a cross-reference, since a theme author
writing `layouts/pdf.typ` is the other audience for the label contract.

### The codly example (for `docs/tips.typ`)

Worth a full worked example rather than a fragment, because #104 shows the failure mode
is visual and easy to misdiagnose — you get *two* nested boxes and no error.

```typ
= Using codly with executed chunks

Packages such as #link("https://typst.app/universe/package/codly")[codly] install their
own `show raw:` rules. Calepin's chunk chrome is a separate labeled block wrapped
*around* that raw, so without the third line below you get codly's frame inside
Calepin's:

```typ
#import "@preview/codly:1.3.0": *
#show: codly-init
#show <calepin-input>: it => it.body   // hand the raw to codly, drop Calepin's box
```

Add `#show <calepin-output>: it => it.body` as well if you want program output bare too;
output is not a `raw` element, so codly does not reach it either way.

Set `theme = "typst"` in the document front matter to drop Calepin's chrome everywhere at
once, without any show rules. Chunks still execute.
```

The example should be a real executed chunk on the page where feasible, so the docs build
catches drift in the contract.

This contract is also the forward-migration path: when Typst ships user-defined
elements, `<calepin-input>` + `it.body` translates mechanically into a real
`calepin.input` element with a `body` field, and the labels can keep working through a
deprecation window.

## Tests

Behavior-focused, per repo convention — assert on observable behavior, not layout or
generated source.

- A document with `#show <calepin-input>: it => it.body` produces no Calepin block around
  echoed source, while chunk execution and output are unaffected.
- A restyle rule (`it => my-frame(it.body)`) replaces the default chrome without nesting
  inside it.
- A `show raw: set text(…)` show-set rule does *not* displace default chrome.
- `theme = "typst"` produces no Calepin chrome for echoed source, output, or fenced
  blocks — **and fenced chunks still execute and render their results.**
- Warnings and errors are separately targetable; `repr`-style results (`<calepin-result>`)
  are targetable separately from stream output (`<calepin-output>`).
- **Recursion guard:** (a) a chunk whose output itself contains a fenced code block does
  not recurse or double-render; (b) a user rule that emits `it.body` for a chunk in a
  registered language (`python`) does not re-trigger `chunk_from_raw_plain`. Both on
  paged and HTML targets. This is the regression risk for the state-based guard and
  deserves the most coverage.
- HTML target still emits `div.sourceCode` / `div.cell-output` and honors label rules.

## Follow-up: extending the label contract (separate changes, not #104)

The end state worth aiming at: **theme bundles own all paged chrome; the runtime emits
only labeled, unstyled structure.** That is the same separation the HTML target already
has with its CSS classes, which is evidence it is the right boundary. Criteria for
giving something a label: (a) it is decorative chrome, not option-driven layout; (b) no
core Typst selector can reach it; (c) HTML already has a CSS hook for it, so paged users
are currently second-class — the exact asymmetry behind #104.

Candidates that pass, in recommended order:

1. **Callouts** (`elements/callout.typ:51`). HTML emits `div.calepin-callout-<kind>`;
   paged is an anonymous `block` with hardcoded colors, stroke, and title styling. Emit
   per-kind labels (`<calepin-callout-note>`, `<calepin-callout-tip>`, …) — a label is
   the only channel that carries the kind, since a block has no custom fields. This is
   the right *first* follow-up: no recursion hazard, no Rust-side involvement, so it
   validates the idiom cheaply after #104 lands. (An additional generic
   `<calepin-callout>` outer wrapper — generic rule for all callouts, specific ones for
   exceptions — works with the same `it.body` idiom but doubles the contract surface;
   only add it if a real use case asks.)
2. **Cards** (`elements/card.typ:51`). Same pattern: `article.calepin-card` on HTML,
   anonymous styled block on paged. One label, `<calepin-card>`.
3. **Image-grid subcaptions** (`_grid-cell`, `render.typ:406-418`), which hardcode
   `stack(spacing: 0.35em, …)` and `text(size: 0.85em)`, plus the gallery/lightbox paged
   caption fallbacks (`gallery.typ:265`, `lightbox.typ:87,97`). A `<calepin-subcaption>`
   label on the caption cell.

Explicitly *not* candidates:

- Figure and caption assembly (`render.typ:35,164,256`): delegates to Typst's real
  `figure`, already reachable via `show figure:` and `show figure.caption:`. A Calepin
  label there would create a second competing override path.
- Width/alignment wrappers (`render.typ:432`, `_wrap-grid-display`) and the tabs
  container (`tabs.typ:227`): structural layout driven by chunk options (`width:`,
  `columns:`), not decoration. A show rule fighting the options API would make behavior
  order-dependent.
- Inline output (plain text) and the HTML divs (already CSS-addressable).

## Out of scope for #104

- Everything in the follow-up section above — #104's risk is concentrated in the
  recursion guard and should land alone.
- A zero-config codly path. Making a document-wide `show raw:` silently remove Calepin's
  chunk chrome was considered and rejected: it only works when the foreign rule happens
  not to re-display `it`, and it turns an unrelated styling rule into a destructive
  side effect for every user. One documented line that works beats zero lines that
  sometimes do.

## Resolved questions

1. **Should `academic` inherit `calepin`'s chunk rules rather than duplicating them?**
   Yes — but not through the theme chain. Paged layout resolution is winner-takes-all
   (`find_notebook_template` returns the first `layouts/pdf.typ` it finds), so chain
   position cannot compose show rules. Sharing goes through a runtime-exported
   `default-chunk-chrome` transform that each bundle applies with one `#show:` line. See
   §3; this also removes duplication that exists today.
