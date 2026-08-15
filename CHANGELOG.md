# Changelog

## 0.0.50

- Website scaffolds created by `calepin new website` now include a `.gitignore` covering `.calepin/` and the generated entry files _Calepin_ stages beside each document while it renders. Those files are hidden, so a `git add -A` after an interrupted build used to commit them silently. Follow-up to [#96](https://github.com/vincentarelbundock/calepin/issues/96).
- `calepin compile --format script` no longer leaves generated entry files behind. Script extraction stages the document body but never renders it, so the files it wrote were never cleaned up, even on success.
- A website `watch` session now cleans up entry files for pages that were renamed or deleted partway through the session. Cleanup ran over the page list as it stood at exit, so a page that disappeared from the site was never swept.
- Leftover entry files are now cleared before a document is staged again, and removed if _Calepin_ panics mid-render. Neither case is reachable through normal use; both previously left hidden files next to the document until the next successful build or a `calepin clean`.
- New `--keep-intermediates` flag for `calepin compile` and `calepin watch` keeps the generated Typst source staged beside the document instead of removing it after a successful render, for inspecting the exact source Typst saw.

## 0.0.49

- Keywords, strings and comments are now colored in code blocks. _Calepin_'s built-in syntax palette defined rules for only six TextMate scopes (function calls, numbers, operators, named arguments), and a `.tmTheme` handed to Typst replaces its built-in theme wholesale rather than merging with it, so every scope _Calepin_ did not name fell back to the plain foreground color. A Python `import` and a string literal came out black. The palette now also covers comments, strings, escape sequences, keywords and storage modifiers, language constants, types and classes, and tags, in both the light and dark variants. Documents that set `highlight-light` or `highlight-dark` to their own `.tmTheme` are unaffected. Thanks to [@peterpf](https://github.com/peterpf) for [the report](https://github.com/vincentarelbundock/calepin/issues/104).
- HTML syntax classes now resolve a scope against the most specific rule that covers it, the way TextMate does, instead of the first rule written. With nested scopes in the palette (`keyword` alongside `keyword.operator`), the old first-match rule would have painted operators with the generic keyword color.

## 0.0.48

- Fix code chunks rendering the source of an earlier block. A fenced block in a language _Calepin_ does not run (```rust, ```json, a kernel that is not installed) stepped the automatic `chunk-1`, `chunk-2`, ... numbering while _Calepin_ scanned the document for chunks, but not while Typst rendered it. The two passes then disagreed about which chunk was which, and every chunk after such a block displayed its predecessor's code. All fenced blocks now take the same path in both passes. Thanks to [@peterpf](https://github.com/peterpf) for [the report](https://github.com/vincentarelbundock/calepin/issues/108).
- A fenced block tagged with a language _Calepin_ has no engine for now renders as an ordinary code block instead of as a chunk. _Calepin_ looks up an unrecognized fence language as a Jupyter kernel name, so a ```rust or ```json block written as prose used to be reported as a chunk and styled as one. Blocks in the documented engines (`r`, `python`, `julia`, `sh`/`bash`, and the diagram engines) are unaffected: when the tool is missing they still render as chunks and echo their source, since the document plainly meant them to run. A block in a language with a real kernel installed still executes as before, and `calepin.setup(fenced-chunks: ...)` still controls which languages run automatically.
- `calepin compile` now reports how many chunks actually ran when some did not, as `[done] paper.typ: 2 of 4 chunks`, rather than counting blocks it could not execute as if it had.
- Removing a single document's artifact directory (`.calepin/<name>/`) no longer breaks every build in the project. `.calepin/` is meant to be regenerable, but a pointer file left behind still named the deleted directory, and the resulting dangling import failed before any document could compile. The pointer is now repaired automatically.

## 0.0.47

- Fix syntax highlighting in paged output, which 0.0.46 rendered in a single near-black color. Code chunks and fenced blocks were painted with the internal palette _Calepin_ uses to map HTML colors onto CSS classes; those colors are placeholders that a post-processing step replaces, not colors meant to be seen. Thanks to [@peterpf](https://github.com/peterpf) for [the report](https://github.com/vincentarelbundock/calepin/issues/104).
- `highlight-light` and `highlight-dark` now apply to paged output (PDF, SVG, PNG) as well as HTML, and to single documents as well as websites: `calepin compile paper.typ --config calepin.toml` reads the same two keys. Paged output uses the light theme, since paper has no light/dark switch.
- _Calepin_ now writes its paged syntax palette to `.calepin/syntax.tmTheme` on every build. A package that renders `raw` itself, such as [codly](https://typst.app/universe/package/codly), claims plain fenced blocks before _Calepin_'s own rules reach them, so those blocks kept Typst's built-in colors while executed chunks used _Calepin_'s. Pointing Typst at the generated file lines the two up: `#set raw(theme: "/.calepin/syntax.tmTheme")`. The file is regenerated from `highlight-light` on every build, so it follows the configured colors.

## 0.0.46

- Code chunks and their output are now emitted as labeled elements, so a document can restyle or remove _Calepin_'s code-block chrome with an ordinary Typst show rule and no _Calepin_-specific API: `#show <calepin-input>: it => it.body` hands the code to a package such as [codly](https://typst.app/universe/package/codly) with no box around it. The labels are `<calepin-input>`, `<calepin-output>`, `<calepin-result>`, `<calepin-warning>`, and `<calepin-error>`; each marks a `block` whose `body` holds the bare content, and an override must reconstruct from `it.body` rather than re-emit `it`. Setting `theme = "typst"` now removes chunk chrome everywhere while chunks still execute. Paged output is no longer a `raw` element, so a document-wide `show raw:` rule styles the code a chunk contains without also reaching what the chunk printed. Thanks to [@peterpf](https://github.com/peterpf) for [the report](https://github.com/vincentarelbundock/calepin/issues/104).

## 0.0.45

- Relocated output from a hidden chunk (`results: "hide"` plus `#calepin.results("label")`) now inherits the document-wide `calepin.setup(results: ...)` mode instead of always rendering verbatim, and `#calepin.results` accepts display options (`results`, `inline-output`, `warning`, `message`, and the `fig-*` family, also via `.with()`) that override the chunk's own choice for that rendering. Execution options are rejected with an error rather than silently ignored. Note that documents setting a non-default `results` mode in `calepin.setup` will see relocated hidden output change accordingly. Thanks to [@beingalink](https://github.com/beingalink) for reporting [#107](https://github.com/vincentarelbundock/calepin/issues/107).
- New `--force` (`-f`) flag for `calepin compile` re-executes all code chunks even when cached results are up to date. Useful when a chunk depends on an external file (e.g. a data set) that changed without the chunk code changing, which the cache fingerprint cannot detect. Unlike `calepin clean`, it does not delete `.calepin/`; it re-runs the chunks and refreshes the cache. Works for single documents, website directory builds (forcing every page), and `--format script` extraction. Thanks to [@dcangst](https://github.com/dcangst) for [the suggestion](https://github.com/vincentarelbundock/calepin/issues/102).
- `calepin clean` now accepts an optional directory argument (`calepin clean [DIR]`) and only sweeps for `.calepin` directories and stray entry files under it, defaulting to the current directory as before. This makes it harder to accidentally delete artifacts across unrelated projects when running from a broad root. Thanks to [@dcangst](https://github.com/dcangst) for [the suggestion](https://github.com/vincentarelbundock/calepin/issues/102).

- New `output-dir` option in `calepin.toml` sets the default output directory for website builds, so generated HTML no longer has to live next to the `.typ` sources. Relative paths resolve from the directory holding `calepin.toml` (e.g. `output-dir = "../_site"`), and a positional output path on the command line still takes precedence. Both `calepin compile` and `calepin watch` honor the setting. Thanks to [@maucejo](https://github.com/maucejo) for [the suggestion](https://github.com/vincentarelbundock/calepin/issues/100).
- Website configuration keys are now consistently kebab-case: `default-language`, `base-url`, `logo-alt`, `content-dir`, `url-prefix`, `show-hidden`, `atom-template`, `rss-template`. The old snake_case spellings remain accepted for backward compatibility. Feed generation is now toggled with `feeds = true` or an explicit `[feeds]` table (which implies the toggle), in the same style as `robots`; the old `generate_feeds` key still works and takes precedence when set. The `filenames` shorthand in `[feeds]` is deprecated in favor of `[[feeds.file]]` tables.

## 0.0.44

- `fig-align: left` and `fig-align: right` now take effect in paged (PDF) output for captioned or labeled chunks. A Typst `figure` centers itself and ignores an outer `align()`, so _Calepin_ silently rendered every captioned figure centered regardless of `fig-align`; uncaptioned images were unaffected. The alignment is now applied through a show rule so both cases honor the setting. Thanks to [@beingalink](https://github.com/beingalink) for [the report](https://github.com/vincentarelbundock/calepin/issues/103).

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
