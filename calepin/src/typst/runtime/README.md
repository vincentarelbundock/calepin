# Calepin runtime split

The embedded Typst runtime (`RUNTIME_SOURCE` in `runtime.rs`, written to
`.calepin/calepin.typ`) is concatenated from six focused files in this order.
Typst resolves symbols top to bottom, so each file may only reference symbols
defined in itself or an earlier file:

1. `runtime/template.typ`
   - HTML shell helper (`html`) for the final document wrapper
2. `runtime/themes.typ`
   - Syntax-highlighting themes for the input/output code blocks
     (`_input-syntax-theme`, `_output-syntax-theme`)
3. `runtime/state.typ`
   - The mode constant, label counters, and the option-defaults dictionaries
   - Small pure helpers (raw-node parsing, MIME selection, asset/href resolution,
     label attachment)
4. `runtime/render.typ`
   - Styled input/output code blocks (`_input-block`, `_output-block`, `_raw-block`)
   - CSS/HTML figure helpers and the result-dispatch chain
     (`_render-results` -> `_render-item` -> `_render-display-item`)
5. `runtime/options.typ`
   - `setup()` plus full option resolution (`_setup-options`, `_resolve-options`)
6. `runtime/chunk.typ`
   - Public chunking API (`chunk`, `inline`, `chunk-from-raw-plain`)
   - Query/render dispatch (`_emit-chunk`), label handling, raw-block interception

`runtime.rs` writes `.calepin/calepin.typ` from:

```rust
include_str!("runtime/template.typ") +
include_str!("runtime/themes.typ") +
include_str!("runtime/state.typ") +
include_str!("runtime/render.typ") +
include_str!("runtime/options.typ") +
include_str!("runtime/chunk.typ")
```
