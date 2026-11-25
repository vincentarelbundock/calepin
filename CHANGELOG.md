# Changelog

## 0.0.62

- Fix an infinite rebuild loop in `calepin watch` for websites with a custom `asset-dir`. Thanks to [@0x7CFE](https://github.com/0x7CFE) ([#131](https://github.com/vincentarelbundock/calepin/issues/131)).
- Fix a stray `Starting kernel event loops.` line in the first Julia chunk. Jupyter kernels' own startup output now goes to the terminal instead of chunk output. Thanks to [@tpoisot](https://github.com/tpoisot) ([#130](https://github.com/vincentarelbundock/calepin/issues/130)).
- `calepin health` now lists the Jupyter kernels available to the Python Calepin uses. The install docs explain which Python needs `jupyter_client`. Thanks to [@technocrat](https://github.com/technocrat) ([#125](https://github.com/vincentarelbundock/calepin/issues/125)).
- Fix Mermaid diagrams failing when Chromium's sandbox helper is unusable, as with Nix's Chromium on Ubuntu.

## 0.0.61

- Fix the website language picker staying open after an outside click.
- Fix Nix package builds.
- Shorten release notes for easier reading.

## 0.0.60

- Size the website sidebar and table of contents separately with `--calepin-sidebar-width` and `--calepin-toc-width`. Thanks to [@kylebutts](https://github.com/kylebutts) ([#120](https://github.com/vincentarelbundock/calepin/issues/120)).

## 0.0.59

- R errors and warnings now show the failing call and preserve rlang formatting. Thanks to [@etiennebacher](https://github.com/etiennebacher) ([#119](https://github.com/vincentarelbundock/calepin/issues/119)).
- Fix website search builds on Windows and reject rooted `asset-dir` paths.

## 0.0.58

- Add Windows installation through Scoop. Upgrade with `scoop update calepin`. Thanks to [@draftman9](https://github.com/draftman9) ([#118](https://github.com/vincentarelbundock/calepin/issues/118)).
- Fix unstable layouts for plain fenced code blocks. These now use Calepin syntax colors instead of `set raw(theme: ...)`.
- Show Typst warnings after successful builds. For missing HTML content inside `align(...)`, remove the wrapper. Thanks to [@uwatlib](https://github.com/uwatlib) ([#117](https://github.com/vincentarelbundock/calepin/issues/117)).

## 0.0.57

- HTTPS requests now honor system certificates, including corporate certificates, and allow longer redirect chains. Thanks to [@pdmetcalfe](https://github.com/pdmetcalfe) ([#116](https://github.com/vincentarelbundock/calepin/pull/116)).
- Fix `calepin update` on Windows. Thanks to [@vmartel08](https://github.com/vmartel08) ([#115](https://github.com/vincentarelbundock/calepin/issues/115)).
- Report broken links with clear HTTP status codes.

## 0.0.56

- Add Debian, RPM, and Arch packages for Linux on x86_64 and aarch64. Upgrade through your package manager.
- Add an [APT repository](https://vincentarelbundock.github.io/apt) for Debian and Ubuntu.

## 0.0.55

- Fix duplicate numbering and broken references for tables and figures printed by chunks.
- Allow nested, independently collapsible website sidebar sections.
- R and Python output now appears below each statement by default. Set `results-location: chunk` to collect output after the full source.
- Fix missing HTML syntax colors in themes such as Catppuccin. Thanks to [@maucejo](https://github.com/maucejo) ([#114](https://github.com/vincentarelbundock/calepin/issues/114)).

## 0.0.52

- Preserve figure options, including alt text, when relocating output with `calepin.results`. Thanks to [@eddelbuettel](https://github.com/eddelbuettel).
- Fix PDF rendering of multiple plots with explicit grid sizes.
- Use `fig-alt-text` to describe tables for accessible PDF output.
- Add `alt` descriptions to `sidefigure` and `lightbox-video` for accessible PDFs.
- Add page excerpts to `calepin.pages()` and website feeds, using metadata or the first paragraph.
- Generate a website `llms.txt` index by default. Set `llms = false` to disable it.

## 0.0.51

- Add individual panel references such as `@fig-name-1` using `fig-subcaptions`.
- Fix `fig-subcap` in chunk headers for PDF output.
- Add table and listing references with `tbl-` and `lst-` labels, plus `tbl-caption` and `lst-caption`.
- Fix duplicate figure labels and captions when R output appears between plots. Later figures may be renumbered.

## 0.0.50

- Include a `.gitignore` in new websites to exclude generated files ([#96](https://github.com/vincentarelbundock/calepin/issues/96)).
- Remove temporary files after script extraction.
- Clean up temporary files for pages renamed or deleted during website watch sessions.
- Clean up leftover temporary files before builds and after rendering panics.
- Add `--keep-intermediates` to retain generated Typst source for inspection.

## 0.0.49

- Improve syntax colors for keywords, strings, comments, and other code tokens. Thanks to [@peterpf](https://github.com/peterpf) ([#104](https://github.com/vincentarelbundock/calepin/issues/104)).
- Fix HTML syntax colors when theme rules overlap.

## 0.0.48

- Fix chunks displaying the source of an earlier code block. Thanks to [@peterpf](https://github.com/peterpf) ([#108](https://github.com/vincentarelbundock/calepin/issues/108)).
- Render fenced blocks in unrecognized languages without an installed kernel as ordinary code blocks.
- Report how many chunks actually ran when some could not execute.
- Rebuild automatically after a document artifact directory is deleted.

## 0.0.47

- Restore syntax highlighting in PDF, SVG, and PNG output. Thanks to [@peterpf](https://github.com/peterpf) ([#104](https://github.com/vincentarelbundock/calepin/issues/104)).
- Apply custom syntax themes to both documents and websites, including paged output.
- Export `.calepin/syntax.tmTheme` for matching syntax colors in packages such as codly.

## 0.0.46

- Allow Typst show rules to restyle chunk code and output. Set `theme = "typst"` to remove chunk boxes. Thanks to [@peterpf](https://github.com/peterpf) ([#104](https://github.com/vincentarelbundock/calepin/issues/104)).

## 0.0.45

- Relocated hidden output now follows document display defaults. Override display options with `calepin.results`. Thanks to [@beingalink](https://github.com/beingalink) ([#107](https://github.com/vincentarelbundock/calepin/issues/107)).
- Add `calepin compile --force` to rerun all chunks regardless of cached results. Thanks to [@dcangst](https://github.com/dcangst) ([#102](https://github.com/vincentarelbundock/calepin/issues/102)).
- Add `calepin clean [DIR]` to limit cleanup to a directory. Thanks to [@dcangst](https://github.com/dcangst) ([#102](https://github.com/vincentarelbundock/calepin/issues/102)).
- Add `output-dir` to configure where website builds are saved. Thanks to [@maucejo](https://github.com/maucejo) ([#100](https://github.com/vincentarelbundock/calepin/issues/100)).
- Use hyphenated website configuration keys and `feeds = true`. Existing spellings still work; prefer `[[feeds.file]]` over `filenames`.

## 0.0.44

- Fix `fig-align` for captioned and labeled figures in PDF output. Thanks to [@beingalink](https://github.com/beingalink) ([#103](https://github.com/vincentarelbundock/calepin/issues/103)).

## 0.0.42

- Add `typ = false` to omit Typst source from websites and remove previously published source files.
- Website view pickers now offer only available HTML, PDF, and source views. Thanks to [@maucejo](https://github.com/maucejo) ([#101](https://github.com/vincentarelbundock/calepin/issues/101)).

## 0.0.41

- Resolve relative file paths from the document directory. Move helpers stored in `.calepin/<stem>/` beside the document. Thanks to [@maucejo](https://github.com/maucejo) ([#96](https://github.com/vincentarelbundock/calepin/issues/96)).
- Center website content on wide screens ([#95](https://github.com/vincentarelbundock/calepin/issues/95)).
- HTML galleries now fill rows in the same order as PDF galleries. Thanks to [@maucejo](https://github.com/maucejo) ([#97](https://github.com/vincentarelbundock/calepin/issues/97)).

## 0.0.39

- Choose a configuration file when starting the VS Code or Positron watcher.

## 0.0.38

- Fix HTML heading references and table of contents links, including scrolling below the website header.

## 0.0.37 (2026-07-23)

- Add a shared `store` for R, Python, Typst, and themes. Mixed engines now run in source order; chunk options accept Typst arrays.

## 0.0.36 (2026-07-23)

- Fix asset paths in website scaffolds ([#94](https://github.com/vincentarelbundock/calepin/issues/94)).

## 0.0.35 (2026-07-22)

- Use `script: false` to exclude chunks or `script: path/to/file` to create named scripts. Improve extracted script formatting ([#93](https://github.com/vincentarelbundock/calepin/issues/93)).

## 0.0.34 (2026-07-22)

- Website tables of contents now float by default and handle wrapped titles. Configure with `toc.floating`.
- Fix chunk labels and dotted kernel names. Rebuild when local themes change and reject invalid SVG sizes.
- Improve figure defaults, PDF galleries, grouped tabs, and source display for dotted engine names.

## 0.0.33 (2026-07-16)

- Use Tinymist or another preview extension for PDF preview in VS Code and Positron.
- Add `calepin compile --format script` to extract notebook code into scripts, with `{ext}` and `{engine}` output templates ([#32](https://github.com/vincentarelbundock/calepin/issues/32)).
- Website watch sessions now rebuild affected pages when imported files, data, or images change ([#66](https://github.com/vincentarelbundock/calepin/issues/66)).
- Add documentation for website tags, categories, and authors using page metadata ([#47](https://github.com/vincentarelbundock/calepin/issues/47)).
- Improve HTML heading anchors, explicit labels, and table of contents links. Thanks to [@rgouveiamendes](https://github.com/rgouveiamendes) ([#80](https://github.com/vincentarelbundock/calepin/pull/80)).
- Synchronize tab selections across containers with the same `group` ([#53](https://github.com/vincentarelbundock/calepin/issues/53)).

## 0.0.32 (2026-07-11)

- Fix academic website scaffold builds. Thanks to [@YifanJiang233](https://github.com/YifanJiang233) ([#92](https://github.com/vincentarelbundock/calepin/pull/92)).
