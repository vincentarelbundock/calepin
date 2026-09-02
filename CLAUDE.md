# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Calepin is a Rust CLI that turns `.typ` files into computational notebooks. It is Typst-native: executable code chunks live directly inside Typst documents (no Markdown layer). The CLI scans a document, executes its code chunks, and lets the real `typst` binary render the results in place. Native engines: `r`, `python`. Diagram engines: `mermaid`, `tikz`, `dot`, `d2` (stateless, always emit SVG). Any other fence language, including `julia` and `sh`, is treated as a Jupyter kernel name and routed through the Jupyter bridge; there is no `bash` alias for `sh`, and the kernel name must match (or version-prefix-match) an installed Jupyter kernel or the chunk is marked unavailable, with a warning on the terminal naming the chunk, engine, and reason (`--strict` or `CALEPIN_STRICT=1` turns that into an error).

The Typst runtime is not one embedded file. `build.rs` bundles every file under `src/assets/typst-runtime/` into the binary at compile time; at preprocess/compile time Calepin writes that bundle to `.calepin/runtime/`, plus a generated facade at `.calepin/calepin.typ` (imports the bundle and re-exports the public API) and a per-notebook binding that points the facade at that notebook's results and config. There is no separate Typst Universe package.

## Commands

The binary crate is nested at `calepin/`, so direct cargo invocations need `--manifest-path calepin/Cargo.toml`. The `Makefile` is the canonical entry point and wraps this for you.

- `make build` / `make build-release` / `make install` (installs to `~/.cargo/bin`)
- `make test` runs the suite: `cargo test --manifest-path calepin/Cargo.toml` plus the `calepin-docs` crate's tests
- Single test: `cargo test --manifest-path calepin/Cargo.toml <test_name>`
- `make check` for a fast `cargo check`
- `cargo clippy --manifest-path calepin/Cargo.toml` for lints
- `make docs-check` runs the generated-docs-fragment test (`cargo test --manifest-path calepin/Cargo.toml generated_docs_fragment_matches_source`); rerun it with `CALEPIN_UPDATE_DOCS=1` to regenerate `docs-src/reference/generated.md` after a change to the facts it tracks (see Conventions below)
- `make cli-reference` regenerates `docs-src/reference/cli.typ` from clap `--help` output
- `make website` / `make serve` build the docs site via `calepin compile docs-src docs` into `docs/` (website config auto-discovered at `docs-src/calepin.toml`)
- `make bump VERSION=x.y.z` then `make release` cuts a release (tags + pushes, which fires the cargo-dist and crates.io workflows). `make release` refuses a dirty tree.
- `make linux-packages` builds the `.deb`, `.rpm` and Arch `.pkg.tar.zst` into `dist/` from `packaging/linux/nfpm.yaml` (needs `nfpm` on `PATH`, Linux only)
- `make editors` builds the extension from `editors/vscode/`, installs it in VS Code, and installs it in Positron when the Positron CLI is available

Integration tests in `calepin/tests/typst_preprocess.rs` (there is no root `tests/` directory) shell out to the built binary plus real `typst`/`python3`/`pdftotext`. They return early (skip, not fail) when a required tool is absent, so a green run on a machine without `typst` may have skipped the meaningful tests. Set `CALEPIN_TEST_REQUIRE_TOOLS=1` (as the CI test job does) to turn every such skip into a failure naming the missing tool.

## Architecture

### Two-pass model around the real `typst` binary

Calepin never renders Typst itself. It wraps the user's `typst` binary and drives it twice over the same source file. The mode is selected via Typst CLI inputs that the runtime reads from `sys.inputs`:

1. **Query pass** (`--input calepin-mode=query`): `typst::preprocess` runs `typst eval` with internal `query(...)` expressions to extract metadata as JSON: `<calepin-config>` (setup defaults, themes) and `<calepin-chunk>` (one entry per chunk). `typst::query` parses these into `ChunkSpec`s.
2. **Render pass** (`--input calepin-mode=render`): `typst::compile` invokes the real `typst compile` or `typst watch`, passing `calepin-results=<path>` and `calepin-target=paged|html`. The runtime reads `results.json` and splices computed output back into the document.

Between the two passes, `typst::execute` runs every chunk and writes `results.json` (schema version 2; see `docs-src/reference/generated.md` for the exact number, generated from `typst/model.rs`).

So the data flow is: `preprocess` (write the runtime bundle -> query metadata -> execute chunks -> write results.json) then `compile_with_typst` (render with results spliced in). `handle_compile` in `typst/cli.rs` chains these; `watch` does the same once, then keeps both processes alive (see below).

### Reserved inputs

Ten `--input` keys are reserved (`calepin-mode`, `calepin-results`, `calepin-store`, `calepin-target`, `calepin-assets`, `calepin-pages`, `calepin-current-href`, `calepin-image-meta`, `calepin-source-dir`, `calepin-site-root`; see `typst/run.rs` and `docs-src/reference/generated.md` for the generated, authoritative list). Anything after `--` on the CLI is forwarded verbatim to `typst`, but `reject_reserved_typst_inputs` blocks a user from overriding these reserved `--input` keys, in either `--input=key=value` or `--input key=value` form.

### Engines (`engines/`)

`r` and `python` are **persistent subprocesses** that live for the whole document render, so variables persist across chunks (notebook semantics). Every other engine name, including `julia` and `sh`, is routed through a single persistent Jupyter bridge subprocess that manages one `jupyter_client` kernel per requested name. `EnginePool` (`typst/execute.rs`) lazily spawns a session per engine on first use; `EngineContext` hands out mutable references during a chunk.

Communication uses a **sentinel protocol** in `engines/subprocess.rs`: the request is framed `{sentinel}_BEGIN\n{payload}\n{sentinel}_END\n`, and the subprocess replies with tagged lines (e.g. `{sentinel}_OUTPUT:`, `_ERROR:`, `_WARNING:`, `_PLOT:`) terminated by `{sentinel}_DONE`. The sentinel is `PID + atomic counter` to avoid collisions with user output. A reader thread plus `recv_timeout` enforces the per-chunk timeout (kills the subprocess on hang). `process_results` in `engines/mod.rs` parses the tagged stream into `EngineResult` variants; `normalize_engine_results` in `execute.rs` turns those into the serialized `ResultItem`s.

Diagram engines (`engines/diagram.rs` and `engines/diagram/`) are different: stateless CLI tools (`mmdc`, `dot`, `d2`, tikz via `tectonic`+`dvisvgm`) that convert source to SVG. They do not use a persistent session and always emit SVG regardless of the chunk's figure format. A chunk's `fig-device-format` is otherwise `svg`, `png`, `jpeg`/`jpg`, or `pdf`; the runtime renders `svg`, `png`, and `jpeg` on both targets and `pdf` on the paged target only; a `pdf` figure on the HTML target fails the render with an error naming the chunk.

### Data model (`typst/model.rs`)

`ResultsDocument` (see `docs-src/reference/generated.md` for the current schema version) is the on-disk JSON contract with the Typst runtime: it maps chunk labels to `ChunkResultDocument`s, each holding `ResultItem`s (types: stream, diagnostic, error, display, result) carrying text or MIME-keyed `data`. Chunk behavior is split into `ExecOptions` (eval, error tolerance, figure device) and `DisplayOptions` (echo, results mode, captions, layout). `SetupDefaults` are document-wide defaults from `calepin.setup(...)` that individual chunk options override.

### Layout / paths (`typst/paths.rs`)

For input `paper.typ` under a project root, artifacts live under `.calepin/<stem>/`: `results.json` and `figures/`. `LayoutPaths` carries root, input (absolute + root-relative), working dir, results path, and figures dir. The `.calepin/` directory is gitignored and treated as regenerable. `artifact_reference` produces root-relative `/`-prefixed paths for Typst.

### Config (`config.rs`)

`.calepin/config.toml` is never auto-loaded; it (or any `--config` path) must be passed explicitly. Its `[executables]` table maps tool names to paths (`typst`, `rscript`, `python`, `mmdc`, `dot`, `tectonic`, `dvisvgm`, `pdf2svg`, `d2`, and optional `chrome`; see `docs-src/reference/generated.md` for the generated list, there is no `julia` or `shell` key). Unknown keys, at the top level or under `[executables]`, are rejected with an error that names the key and lists the valid ones; because a website `calepin.toml` is parsed by both this module and `website/config.rs`, each side carries placeholder fields for the other, and a test loads `docs-src/calepin.toml` through both to catch drift. Path-like values resolve relative to the config file's own directory, not the project root; bare command names (e.g. `python3`) are left for the OS to resolve on `PATH`. The top level of the config also accepts `theme`, `store`, `asset-dir`, `toc`, `highlight-light`, and `highlight-dark` (again, see the generated fragment). The output target (paged vs html) is NOT a config option: it is derived from the document front matter or the CLI `--format` flag, never from `config.toml`.

### Watch (`typst/watch/`)

`calepin watch` preprocesses once, then normally spawns a child `typst watch` for live re-rendering and runs its own filesystem watcher (`notify`) over the project root. On a source change, the watcher re-runs the metadata query; the preprocessing fingerprint prevents prose and display-only changes from re-evaluating chunks. The child `typst watch` notices changed results and re-renders. With `--eval-only`, Calepin does not spawn `typst watch` or write rendered output, leaving preview to an external frontend such as Tinymist. For an HTML target, Calepin runs its own small loopback HTTP server (`typst/watch/assets.rs`) to serve generated assets to the live page; `calepin serve` runs a separate `tiny_http`-based server for previewing a built website. Calepin does own HTTP servers for these cases. SIGINT, SIGTERM, and SIGHUP all stop the active watcher processes and remove the generated entry files; a second signal kills the child `typst watch` and exits immediately. A watcher-thread failure or panic is treated as a stop and reported as an error rather than leaving `typst watch` running unattended.

### Theme bundles (`theme.rs`, `theme/notebook.rs`, `html/theme.rs`)

Themes are bundles with well-known entry files: `layouts/pdf.typ`, `layouts/document.html`, `layouts/site.html` (see `docs-src/reference/generated.md` for the generated list). `theme.rs` owns selection, builtin bundle metadata, fallback to the default `calepin` bundle, and `calepin new theme` ejection. HTML rendering resolves a bundle entry before calling `html/theme.rs`; paged rendering resolves `layouts/pdf.typ` from the effective bundle during preprocessing. User-owned themes live outside `.calepin/`; `.calepin/` remains regenerable and overwritten by builds.

## Conventions

- Tests are behavior-focused. Do not add regression pins on exact layout, generated source strings, or byte output; assert on observable behavior instead.
- Embedded assets (the Typst runtime bundle, the html-theme templates and `.tmTheme` files) are compiled into the binary via `build.rs`. Editing them changes program behavior and requires a rebuild; there is no separate install step.
- A handful of mechanical facts (reserved input keys, `[executables]` and other config keys, the results schema version, supported engine names, theme entry files) drift easily if retyped by hand, so they live in `calepin/src/docs_fragment.rs`: a `#[cfg(test)]`-only module that renders them straight from the Rust source and checks the result against `docs-src/reference/generated.md`. Run `make docs-check` after changing any of those facts; it fails on drift and `CALEPIN_UPDATE_DOCS=1 make docs-check` regenerates the checked-in fragment.
- `editors/vscode/` is a small TypeScript VS Code extension that bundles a built `calepin` binary. It contributes explicit start/stop commands for `calepin watch --eval-only` and does not integrate with or depend on a preview extension. Its version is kept in sync with the Rust crate by `make vscode-sync-version`.
