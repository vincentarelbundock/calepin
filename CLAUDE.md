# CLAUDE.md

This file provides guidance to Claude Code when working with this repository.

## Project

*Calepin* is a Rust CLI for preprocessing Typst documents with executable code chunks. Typst remains the renderer. *Calepin* discovers `#calepin.chunk` calls, executes R, Python, and shell chunks, writes result artifacts under `.calepin/`, and can optionally invoke `typst compile`.

The public CLI is:

```sh
calepin preprocess INPUT.typ
calepin compile INPUT.typ [OUTPUT.pdf] [-- TYPST_ARGS...]
```

The old Quarto-style renderer, preview server, template system, website/book commands, extension system, citation resolver, and Markdown parser are no longer public behavior.

When referring to the software by name in documentation or notebooks, always write *Calepin* with italic markup and capital C.

## Writing Style

* Never use em or en dashes in documentation or websites.
* Never use bold words in documentation or websites unless it was there in the original source.

## Workflow

When you are done with changes and believe the feature works, run `make install` to install the updated binary.

When asked to commit, do not run `git add` or `git commit` yourself. Instead:

1. Check `git status` to see what files are staged and unstaged.
2. Propose a full `git commit -m "..."` command the user can paste into their shell.

## Build Commands

```sh
make build
make release
make install
make check
make test
cargo test typst::
cargo test --test typst_preprocess
```

Run a single test with `cargo test test_name`.

## Typst Authoring Contract

A document imports the runtime that *Calepin* writes before querying:

```typ
#import ".calepin/calepin.typ"

#calepin.chunk(engine: "python", label: "answer")[`
print(42)
`]
```

Chunk bodies must contain exactly one bare Typst raw element. Prefer single-backtick raw delimiters. Do not use language-tagged raw blocks inside chunks because the engine is declared only by the `engine:` argument.

Labels are required and must be unique. Supported engines are `r`, `python`, `sh`, and `bash` as an alias for `sh`.

## Current Architecture

The new Typst path lives in `calepin/src/typst/`:

* `runtime.typ` and `runtime.rs` embed and write `.calepin/calepin.typ`.
* `query.rs` parses Typst metadata from `typst query`.
* `model.rs` defines chunk specs, execution options, display options, and the results JSON schema.
* `execute.rs` reuses the persistent R, Python, and shell subprocess engines and normalizes their outputs into result items.
* `cache.rs` implements content-addressed digest-chain caching with downstream invalidation.
* `paths.rs` owns `.calepin/<input-stem>/` layout and Typst root-relative artifact references.
* `results.rs` writes schema-1 `results.json`.
* `preprocess.rs` orchestrates runtime writing, Typst query, execution, caching, results writing, and optional Typst compilation.
* `cli.rs` connects public CLI handlers to the Typst pipeline.

Legacy modules under `parse/`, `render/`, `modules/`, `references/`, `collection/`, `preview/`, and older `cli/` files may still compile while the rewrite is landing, but they should not be made reachable from the public CLI again.

## Result Layout

For `paper.typ`:

```text
.calepin/calepin.typ
.calepin/paper/results.json
.calepin/paper/figures/
.calepin/paper/cache/
```

For nested input such as `chapters/intro.typ` under the project root:

```text
.calepin/chapters/intro/results.json
.calepin/chapters/intro/figures/
.calepin/chapters/intro/cache/
```
