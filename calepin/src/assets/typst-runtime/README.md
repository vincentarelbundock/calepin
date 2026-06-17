# Calepin runtime modules

The embedded Typst runtime is copied to `.calepin/runtime/`, with a generated
`.calepin/calepin.typ` facade that imports the runtime modules. Runtime modules
must explicitly import the modules they depend on; sibling imports in the facade
do not share scope across modules.

1. Generated syntax theme prelude
   - Syntax highlighting themes for input/output code blocks, generated from
     `src/syntax_theme.rs`
2. `assets/typst-runtime/core/target.typ`
   - Render/query mode and HTML/paged target predicates.
3. `assets/typst-runtime/core/css.typ`
   - CSS string helpers shared by notebook rendering and elements.
4. `assets/typst-runtime/core/assets.typ`
   - Asset URL resolution and image metadata lookup.
5. `assets/typst-runtime/core/state.typ`
   - Label counters, option-default dictionaries, raw-node parsing, result
     selection, label attachment, and website pages helpers.
6. `assets/typst-runtime/notebook/render.typ`
   - Styled input/output block rendering helpers (`_input-block`, `_output-block`,
     `_raw-block`) and figure/result rendering utilities
     (`_render-results` -> `_render-item` -> `_render-display-item`)
7. `assets/typst-runtime/notebook/options.typ`
   - `setup()` plus full option resolution (`_setup-options`, `_resolve-options`)
8. `assets/typst-runtime/notebook/chunk.typ`
    - Public chunking API (`chunk`, `inline`, `chunk-from-raw-plain`)
    - Query/render dispatch (`_emit-chunk`), label handling, raw-block interception

9. `assets/typst-runtime/elements/mod.typ`
   - Public element namespace and helpers, currently including `elements.gallery`

`runtime.rs` writes the generated syntax theme as
`.calepin/runtime/00_syntax-theme.typ`, copies every `.typ` file in this
directory, and writes `.calepin/calepin.typ` as the public facade.
