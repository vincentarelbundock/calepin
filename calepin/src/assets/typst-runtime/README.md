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
   - Asset URL resolution, safe inline-JavaScript string quoting, and image
     metadata lookup.
5. `assets/typst-runtime/core/results.typ`
   - Shared loading and lookup helpers for the results document.
6. `assets/typst-runtime/core/pages.typ`
   - Website page index loading and site-relative link resolution.
7. `assets/typst-runtime/core/state.typ`
   - Compatibility facade for the helpers that historically lived in this
     module. New runtime code imports their focused modules directly.
8. `assets/typst-runtime/notebook/defaults.typ`
   - Document and call option defaults plus automatic-label counters.
9. `assets/typst-runtime/notebook/chunk-support.typ`
   - Chunk relocation state, raw-node parsing, label derivation, and the
     `_without-raw-chunk-transforms` recursion guard.
10. `assets/typst-runtime/notebook/result-support.typ`
    - Result representation selection, artifact paths, and label attachment.
11. `assets/typst-runtime/notebook/code.typ`
   - Labeled, unstyled input/output carriers (`_input-block`, `_output-block`)
     plus the default chrome helpers (`code-block`, `output-block`) and the
     HTML/paged source wrappers. Styling is applied by show rules, not here.
   - `chrome.typ` holds `default-chunk-chrome`, the show rules a theme bundle
     opts into for that styling, plus fenced-block handling in themed documents.
12. `assets/typst-runtime/notebook/render.typ`
   - Figure and result rendering utilities
     (`_render-results` -> `_render-item` -> `_render-display-item`), with
     compatibility re-exports for the code helpers.
13. `assets/typst-runtime/notebook/options.typ`
    - `setup()` plus internal option resolution, with the historical
      `_resolve-options(engine, args)` wrapper retained for themes.
14. `assets/typst-runtime/notebook/chunk.typ`
    - Public chunking API (`chunk`, `inline`, `_fenced-chunk`)
    - Query/render dispatch (`_emit-chunk`), label handling, raw-block interception

15. `assets/typst-runtime/elements/mod.typ`
   - Public element namespace and helpers, currently including `elements.gallery`

`runtime.rs` writes the generated syntax theme as
`.calepin/runtime/00_syntax-theme.typ`, copies every `.typ` file in this
directory, and writes `.calepin/calepin.typ` as the public facade.
