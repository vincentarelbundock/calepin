# Calepin runtime split

The embedded Typst runtime (`RUNTIME_SOURCE` in `typst/runtime.rs`, written to
`.calepin/calepin.typ`) is concatenated from six focused files in this order.
Typst resolves symbols top to bottom, so each file may only reference symbols
defined in itself or an earlier file:

1. `assets/typst-runtime/template.typ`
   - HTML shell helper (`html`) for the final document wrapper
2. `assets/typst-runtime/themes.typ`
   - Syntax-highlighting themes for the input/output code blocks
     (`_input-syntax-theme`, `_output-syntax-theme`)
3. `assets/typst-runtime/state.typ`
   - The mode constant, label counters, and the option-defaults dictionaries
   - Small pure helpers (raw-node parsing, MIME selection, asset/href resolution,
     label attachment)
4. `assets/typst-runtime/render.typ`
   - Styled input/output code blocks (`_input-block`, `_output-block`, `_raw-block`)
   - CSS/HTML figure helpers and the result-dispatch chain
     (`_render-results` -> `_render-item` -> `_render-display-item`)
5. `assets/typst-runtime/options.typ`
   - `setup()` plus full option resolution (`_setup-options`, `_resolve-options`)
6. `assets/typst-runtime/chunk.typ`
   - Public chunking API (`chunk`, `inline`, `chunk-from-raw-plain`)
   - Query/render dispatch (`_emit-chunk`), label handling, raw-block interception

`runtime.rs` writes `.calepin/calepin.typ` from:

```rust
include_str!("../assets/typst-runtime/template.typ") +
include_str!("../assets/typst-runtime/themes.typ") +
include_str!("../assets/typst-runtime/state.typ") +
include_str!("../assets/typst-runtime/render.typ") +
include_str!("../assets/typst-runtime/options.typ") +
include_str!("../assets/typst-runtime/chunk.typ")
```
