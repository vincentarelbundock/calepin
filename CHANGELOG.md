# Changelog

## 0.0.42

- Add a website `typ` option to `calepin.toml`. Websites publish each page's Typst source by default: the `.typ` file is copied into the output directory and the full source is embedded in the rendered HTML for the theme's view-source mode. Set `typ = false` to withhold both, which leaves an output directory of HTML only. Switching a site from `true` to `false` also removes `.typ` files copied by earlier builds.
- Bundled website themes now list only the page views that exist in their view-mode picker. A site built with `pdf = false` no longer offers a PDF entry that fails to load, `typ = false` drops the Source entry, and an HTML-only site omits the picker entirely. Themes can make the same distinction through the new `site.page_pdf` and `site.page_source` template variables. Thanks to [@maucejo](https://github.com/maucejo) for [the report](https://github.com/vincentarelbundock/calepin/issues/101).

## 0.0.41

- Relative paths in a document now resolve from the document's own directory, the way they do in plain Typst. `#import "utils.typ"`, `#image("figure.svg")`, `#csv("data.csv")`, `#bibliography("refs.bib")`, and `#include` all find files next to the document, and `../` reaches its parent directory. Previously _Calepin_ compiled a staged copy of each document under `.calepin/<stem>/`, and Typst resolved relative paths from that directory instead, so these paths failed with "file not found" while working in a Tinymist preview, which compiles the document in place. Root-relative paths beginning with `/` are unchanged. The generated entry files that carry the document body are now written beside the document as hidden `.calepin-entry.*` files and removed when the render finishes; `calepin clean` deletes any left behind by an interrupted run. Results, figures, and metadata still live under `.calepin/`. A document that relied on the old behavior by placing a helper inside `.calepin/<stem>/` must move that helper next to the document. Thanks to [@maucejo](https://github.com/maucejo) for [the report](https://github.com/vincentarelbundock/calepin/issues/96).

- Center the bundled website theme's three-column shell (sidebar, main, table of contents) in the viewport. The shell was pinned to the left edge, so on wide screens all the leftover horizontal space collected on the right while the topbar, footer, and page navigation stayed centered ([#95](https://github.com/vincentarelbundock/calepin/issues/95)).
- `calepin.elements.gallery` now fills its grid in row-major order in HTML output, matching paged output and Typst's own `grid`. Previously an integer `columns` produced a CSS multi-column container, which flowed items down each column before starting the next; a string `columns` was already row-major, so ordering depended on the argument's type. Galleries relying on the old masonry packing will now align items in rows. The documentation shows how to reorder items with a plain Typst helper when a different order is wanted. Thanks to [@maucejo](https://github.com/maucejo) for [the report](https://github.com/vincentarelbundock/calepin/issues/97).

## 0.0.39

- The VS Code and Positron extension's `Typst: Calepin` command now opens a picker before starting the watcher: start as before without a config file, or choose a config TOML to pass as `--config`. Re-running the command with a different choice restarts the watcher with the new configuration.

## 0.0.38

- Fix TOC and cross-reference anchors in HTML output: TOC entries now link to a heading's pre-existing `id` when Typst's HTML export emits one; explicit heading labels are honored (and internal `@label` reference links repointed at them) even when Typst attaches its own reference id to the heading; and anchor jumps on website pages land below the sticky topbar instead of underneath it.

## 0.0.37 (2026-07-23)

- Replace the legacy document-variable transport with a validated document-scoped `store`: initialize values with `[store]`, `calepin.store.set()`, or `--set store.*`; move structured values between R, Python, Typst, and theme templates with `store-get`/`store-set`; and support store-driven multipass chunk expansion with complete-build cache reuse and generation-safe watch publication. Mixed-engine notebooks now execute in source order instead of engine-grouped order, which can change the ordering of observable side effects. `#|` options accept Typst-style parenthesized arrays while retaining bracket-array compatibility, and themed discovery keeps adjacent fenced blocks matched to their own results.

## 0.0.36 (2026-07-23)

- Resolve website asset paths consistently by exposing `_resolve-asset-path` alongside `_resolve-asset-href` from the runtime, and update the website scaffolds to use them ([#94](https://github.com/vincentarelbundock/calepin/issues/94)).

## 0.0.35 (2026-07-22)

- Extend script extraction: chunks can use `script: false` to opt out or `script: path/to/file` to build multiple named scripts from one notebook; `calepin.setup(script: false)` provides an explicit opt-in workflow. Recognized languages now receive conventional extensions and valid comment separators, while JSON and unknown languages omit separators rather than risk invalid output ([#93](https://github.com/vincentarelbundock/calepin/issues/93)).

## 0.0.34 (2026-07-22)

- Make website tables of contents float by default with site-wide and per-page `toc.floating` overrides, and use content-driven spacing so wrapped titles expand without overlapping adjacent entries.
- Fix Typst chunk handling for dotted kernel names, duplicate labels, numeric label overflow, and path-like artifact labels; invalidate preprocessing caches when local theme files change and reject invalid SVG dimensions.
- Improve the Typst helper runtime with complete document-level figure defaults, source-aware paged galleries, collision-safe grouped tabs, safely configurable frontend assets, shared code/results modules, and canonical dotted-engine source echo with legacy-results compatibility.

## 0.0.33 (2026-07-16)

- Slim down the VS Code and Positron extension by removing its bundled PDF.js preview assets and delegating Typst preview to Tinymist or another dedicated preview extension.
- Add `calepin compile --format script` to extract a Typst notebook's executable chunks into separate language-specific `.R`, `.py`, `.jl`, and `.sh` files, with `{ext}` and `{engine}` output templates ([#32](https://github.com/vincentarelbundock/calepin/issues/32)).
- Track Typst input dependencies during website builds so `watch` incrementally rebuilds every affected page when imported Typst files, data, images, or other dependent files change ([#66](https://github.com/vincentarelbundock/calepin/issues/66)).
- Document how to build generic tag, category, and author taxonomies with page metadata and `calepin.pages()`, with a live tag index for the documentation site ([#47](https://github.com/vincentarelbundock/calepin/issues/47)).
- Generate clean HTML heading anchors from visible text, honor explicit Typst labels safely across standard and deep headings, and keep in-page TOC links aligned. Thanks to [@rgouveiamendes](https://github.com/rgouveiamendes) for [the contribution](https://github.com/vincentarelbundock/calepin/pull/80).
- Add a `group` argument to `calepin.elements.tabs` so tab containers with the same group synchronize their selected panel; see the new tab-groups notebook example ([#53](https://github.com/vincentarelbundock/calepin/issues/53)).

## 0.0.32 (2026-07-11)

- Export `_resolve-asset-href` from the generated Typst runtime, fixing academic website scaffold builds. Thanks to first-time contributor [@YifanJiang233](https://github.com/YifanJiang233) for [the fix](https://github.com/vincentarelbundock/calepin/pull/92).
