# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Calepin is a Rust CLI that turns `.typ` files into computational notebooks. It is Typst-native: executable code chunks live directly inside Typst documents (no Markdown layer). The CLI scans a document, executes its code chunks, and lets the real `typst` binary render the results in place. Supported engines: `r`, `python`, `julia`, `sh` (`bash` is an alias for `sh`), plus diagram engines `mermaid`, `tikz`, `dot`, `d2`.

The matching Typst runtime is embedded in the binary (`include_str!` of `runtime.typ`) and written to `.calepin/calepin.typ` at compile/watch time. There is no separate Typst Universe package.

## Commands

The binary crate is nested at `calepin/`, so direct cargo invocations need `--manifest-path calepin/Cargo.toml`. The `Makefile` is the canonical entry point and wraps this for you.

- `make build` / `make build-release` / `make install` (installs to `~/.cargo/bin`)
- `make test` runs the suite: `cargo test --manifest-path calepin/Cargo.toml`
- Single test: `cargo test --manifest-path calepin/Cargo.toml <test_name>`
- `make check` for a fast `cargo check`
- `cargo clippy --manifest-path calepin/Cargo.toml` for lints
- `make cli-reference` regenerates `docs-src/cli.md` from clap `--help` output
- `make website` / `make serve` build the docs site via `calepin compile docs docs` into `docs/` (website config auto-discovered at `docs/calepin.toml`)
- `make bump VERSION=x.y.z` then `make release` cuts a release (tags + pushes, which fires the cargo-dist and crates.io workflows). `make release` refuses a dirty tree.
- `make editors` builds the extension from `editors/vscode/`, installs it in VS Code, and installs it in Positron when the Positron CLI is available

Integration tests in `tests/typst_preprocess.rs` shell out to the built binary plus real `typst`/`python3`/`pdftotext`. They return early (skip, not fail) when a required tool is absent, so a green run on a machine without `typst` may have skipped the meaningful tests.

## Architecture

### Two-pass model around the real `typst` binary

Calepin never renders Typst itself. It wraps the user's `typst` binary and drives it twice over the same source file. The mode is selected via Typst CLI inputs that `runtime.typ` reads from `sys.inputs`:

1. **Query pass** (`--input calepin-mode=query`): `typst::preprocess` runs `typst eval` with internal `query(...)` expressions to extract metadata as JSON: `<calepin-config>` (setup defaults, themes) and `<calepin-chunk>` (one entry per chunk). `typst::query` parses these into `ChunkSpec`s.
2. **Render pass** (`--input calepin-mode=render`): `typst::compile` invokes the real `typst compile` or `typst watch`, passing `calepin-results=<path>` and `calepin-target=paged|html`. The embedded runtime reads `results.json` and splices computed output back into the document.

Between the two passes, `typst::execute` runs every chunk and writes `results.json`.

So the data flow is: `preprocess` (write runtime -> query metadata -> execute chunks -> write results.json) then `compile_with_typst` (render with results spliced in). `handle_compile` in `typst/cli.rs` chains these; `watch` does the same once, then keeps both processes alive (see below).

### Reserved inputs

`calepin-mode`, `calepin-results`, `calepin-target`, and `calepin-raw-theme` are reserved. Anything after `--` on the CLI is forwarded verbatim to `typst`, but `reject_reserved_typst_inputs` blocks a user from overriding these reserved `--input` keys.

### Engines (`engines/`)

Each language engine (`r`, `python`, `julia`, `sh`) is a **persistent subprocess** that lives for the whole document render, so variables persist across chunks (notebook semantics). `EnginePool` (`typst/execute.rs`) lazily spawns a session per engine on first use; `EngineContext` hands out mutable references during a chunk.

Communication uses a **sentinel protocol** in `engines/subprocess.rs`: the request is framed `{sentinel}_BEGIN\n{payload}\n{sentinel}_END\n`, and the subprocess replies with tagged lines (e.g. `{sentinel}_OUTPUT:`, `_ERROR:`, `_WARNING:`, `_PLOT:`) terminated by `{sentinel}_DONE`. The sentinel is `PID + atomic counter` to avoid collisions with user output. A reader thread plus `recv_timeout` enforces the per-chunk timeout (kills the subprocess on hang). `process_results` in `engines/mod.rs` parses the tagged stream into `EngineResult` variants; `normalize_engine_results` in `execute.rs` turns those into the serialized `ResultItem`s.

Diagram engines (`engines/diagram/`) are different: stateless CLI tools (`mmdc`, `dot`, `d2`, tikz via `tectonic`+`dvisvgm`) that convert source to SVG. They do not use a persistent session and always emit SVG regardless of the chunk's figure format.

### Data model (`typst/model.rs`)

`ResultsDocument` (schema version 1) is the on-disk JSON contract with the Typst runtime: it maps chunk labels to `ChunkResultDocument`s, each holding `ResultItem`s (types: stream, diagnostic, error, display, result) carrying text or MIME-keyed `data`. Chunk behavior is split into `ExecOptions` (eval, error tolerance, figure device) and `DisplayOptions` (echo, results mode, captions, layout). `SetupDefaults` are document-wide defaults from `calepin.setup(...)` that individual chunk options override.

### Layout / paths (`typst/paths.rs`)

For input `paper.typ` under a project root, artifacts live under `.calepin/<stem>/`: `results.json` and `figures/`. `LayoutPaths` carries root, input (absolute + root-relative), working dir, results path, and figures dir. The `.calepin/` directory is gitignored and treated as regenerable. `artifact_reference` produces root-relative `/`-prefixed paths for Typst.

### Config (`config.rs`)

`.calepin/config.toml` has one table, `[executables]`, mapping tool names (`typst`, `python`, `rscript`, `julia`, `shell`, `mmdc`, `dot`, `d2`, `tectonic`, `dvisvgm`, `pdf2svg`, optional `chrome`) to paths. Relative path-like values resolve from the project root; bare command names (e.g. `python3`) are left for the OS to resolve on `PATH`. The output target (paged vs html) is NOT a config option: it is derived from the document front matter or the CLI `--format` flag, never from `config.toml`.

### Watch (`typst/watch/`)

`calepin watch` preprocesses once, then normally spawns a child `typst watch` for live re-rendering and runs its own filesystem watcher (`notify`) over the project root. On a source change, the watcher re-runs the metadata query; the preprocessing fingerprint prevents prose and display-only changes from re-evaluating chunks. The child `typst watch` notices changed results and re-renders. With `--eval-only`, Calepin does not spawn `typst watch` or write rendered output, leaving preview to an external frontend such as Tinymist. Calepin owns no HTTP server or port. Ctrl+C stops the active watcher processes.

### Theme bundles (`theme.rs`, `html/theme.rs`)

Themes are bundles under `calepin/src/assets/themes/<name>/` with well-known
entry files: `document.html`, `site.html`, and `paged.typ`. `theme.rs` owns
selection, builtin bundle metadata, fallback to the default `calepin` bundle,
and `calepin new theme` ejection. HTML rendering resolves a bundle entry before
calling `html/theme.rs`; paged rendering injects the effective bundle
`paged.typ` during preprocessing. User-owned themes live outside `.calepin/`;
`.calepin/` remains regenerable and overwritten by builds.

## Conventions

- Tests are behavior-focused. Do not add regression pins on exact layout, generated source strings, or byte output; assert on observable behavior instead.
- Embedded assets (`runtime.typ`, the html-theme templates and `.tmTheme` files) are compiled into the binary via `include_str!`. Editing them changes program behavior and requires a rebuild; there is no separate install step.
- `editors/vscode/` is a small TypeScript VS Code extension that bundles a built `calepin` binary. It contributes explicit start/stop commands for `calepin watch --eval-only` and does not integrate with or depend on a preview extension. Its version is kept in sync with the Rust crate by `make vscode-sync-version`.
