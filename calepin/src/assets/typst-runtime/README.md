# Calepin runtime split

The embedded Typst runtime, written to `.calepin/calepin.typ`, is concatenated from
all `.typ` files in this directory in lexicographic filename order.
Typst resolves symbols top to bottom, so each file may only reference symbols
defined in itself or an earlier file:

1. `assets/typst-runtime/10_template.typ`
   - HTML shell helper (`html`) for the final document wrapper
2. `assets/typst-runtime/20_state.typ`
   - The mode constant, label counters, and the option-defaults dictionaries
   - Small pure helpers (raw-node parsing, MIME selection, asset/href resolution,
     label attachment)
3. `assets/typst-runtime/30_render.typ`
   - Styled input/output block rendering helpers (`_input-block`, `_output-block`,
     `_raw-block`) and figure/result rendering utilities
     (`_render-results` -> `_render-item` -> `_render-display-item`)
4. `assets/typst-runtime/40_options.typ`
   - `setup()` plus full option resolution (`_setup-options`, `_resolve-options`)
5. `assets/typst-runtime/50_chunk.typ`
   - Public chunking API (`chunk`, `inline`, `chunk-from-raw-plain`)
   - Query/render dispatch (`_emit-chunk`), label handling, raw-block interception

`runtime.rs` reads that directory at build time and concatenates every `.typ` file
in sorted filename order. Theme customization remains in
`src/assets/themes/shared/typst/code-block.typ`; runtime semantics stay in
`src/assets/typst-runtime`.
