# Changelog

## Unreleased

- Fix Typst chunk handling for dotted kernel names, duplicate labels, numeric label overflow, and path-like artifact labels; invalidate preprocessing caches when local theme files change and reject invalid SVG dimensions.
- Improve the Typst helper runtime with complete document-level figure defaults, source-aware paged galleries, collision-safe grouped tabs, safely configurable frontend assets, shared code/results modules, and canonical dotted-engine source echo with legacy-results compatibility.
- Slim down the VS Code and Positron extension by removing its bundled PDF.js preview assets and delegating Typst preview to Tinymist or another dedicated preview extension.
- Add `calepin compile --format script` to extract a single Typst notebook's executable chunks into separate language-specific `.R`, `.py`, `.jl`, and `.sh` files, with `{ext}` and `{engine}` output templates ([#32](https://github.com/vincentarelbundock/calepin/issues/32)).
- Track Typst input dependencies during website builds so `watch` incrementally rebuilds every affected page when imported Typst files, data, images, or other dependent files change ([#66](https://github.com/vincentarelbundock/calepin/issues/66)).
- Document how to build generic tag, category, and author taxonomies with page metadata and `calepin.pages()`, with a live tag index for the documentation site ([#47](https://github.com/vincentarelbundock/calepin/issues/47)).
- Generate clean HTML heading anchors from visible text, honor explicit Typst labels safely across standard and deep headings, and keep in-page TOC links aligned. Thanks to [@rgouveiamendes](https://github.com/rgouveiamendes) for [the contribution](https://github.com/vincentarelbundock/calepin/pull/80).
- Add a `group` argument to `calepin.elements.tabs` so tab containers with the same group synchronize their selected panel; see the new tab-groups notebook example ([#53](https://github.com/vincentarelbundock/calepin/issues/53)).
- Export `_resolve-asset-href` from the generated Typst runtime, fixing academic website scaffold builds. Thanks to first-time contributor [@YifanJiang233](https://github.com/YifanJiang233) for [the fix](https://github.com/vincentarelbundock/calepin/pull/92).
