# Extension System -- Remaining Work

## Not yet implemented

1. ~~**Element-level external modules**~~ Done for `element_children` (div-level). External scripts receive JSON with content/classes/attrs and return rendered output. `span` and `element` kinds not yet supported.

2. **WASM runtime integration** -- `.wasm` files should run in the Extism runtime. Currently all external modules run as shell subprocesses. The Extism PDK exists in `plugins/` but is not wired into the extension system.

3. ~~**Extension vars in external module input**~~ Done. Vars from extension manifests are passed to external modules.

4. ~~**`config` field in project module JSON input**~~ Done. Project modules receive `config` with `title` and `target`.

5. **Project module bodies** -- `site_wrap`, `crossref_global`, and `orchestrator` are stubs. The actual logic lives in `collection/`. Full migration would move collection code into these modules.

6. ~~**Full pipeline unification**~~ Done. `render_one_with_context` now calls `render_page` directly (the shared per-page pipeline) instead of `render_file`. Both single-doc and collection rendering use the same `render_page` function. `render_file` remains only for the preview system.

## Cleanup

7. ~~**Remove `compile` field on Target**~~ Done. Replaced with `post` commands. `post = ["typst compile {input}"]` in pdf/book targets. No auto-detection.

8. ~~**Unify `plugins` and `extensions` config keys**~~ Done. `calepin.plugins` is now an alias for `calepin.extensions`. Both feed into the same list.

9. ~~**Inline `themes.rs`**~~ Done. `Theme` struct removed. Replaced with standalone `copy_builtin_assets()` function.

10. ~~**Remove backward-compat config aliases**~~ Done. Removed `title` (alias for `text`) and `index` (alias for `href`). Kept `pages` and `dir` since they're used in real configs.

11. ~~**Rename `--theme` flag**~~ Done. Renamed to `--target` on `init website`, `init notebook`, and `init sidecar`.
