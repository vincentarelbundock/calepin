# Semantic Website Menus Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Replace layout-oriented website navbar config with semantic named menus and support local SVG icon tokens.

**Architecture:** Replace `NavbarConfig`, `NavbarPlan`, and `NavbarModel` with deterministic named menu maps while reusing the existing nav item resolver and model. Keep `[sidebar]` separate. Expose `site.menus` and `site.menu_list` to themes, and update bundled templates plus docs to use `menus.main` and `menus.social`.

**Tech Stack:** Rust, serde TOML config parsing, Minijinja-style HTML templates, embedded theme assets, Typst docs.

---

### Task 1: Config and Menu Model

**Files:**
- Modify: `calepin/src/website/config.rs`
- Modify: `calepin/src/website/mod.rs`

- [x] Write failing tests for `[menus]` parsing, invalid menu names, menu resolution, weight ordering, and removal of `[navbar]`.
- [x] Run targeted tests and verify they fail because `menus` does not exist yet.
- [x] Add `menus: BTreeMap<String, Vec<MenuItemConfig>>` to `WebsiteConfig`.
- [x] Replace navbar plan/model code with named menu plan/model maps.
- [x] Preserve sidebar behavior unchanged.
- [x] Run targeted tests and verify they pass.

### Task 2: Theme Context and Templates

**Files:**
- Modify: `calepin/src/html/theme.rs`
- Modify: `calepin/src/assets/themes/calepin/partials/site-topbar.html`
- Modify: `calepin/src/assets/themes/academic/partials/site-nav.html`
- Modify: `calepin/src/html/mod.rs`

- [x] Write failing tests that bundled themes render `site.menus.main` and `site.menus.social`.
- [x] Run targeted tests and verify they fail because templates still use `navbar_left/right`.
- [x] Replace `navbar_left`, `navbar_center`, and `navbar_right` with `menus` and `menu_list` context fields.
- [x] Update bundled templates to render `menus.main` and `menus.social`.
- [x] Run targeted tests and verify they pass.

### Task 3: Local SVG Icon Tokens

**Files:**
- Modify: `calepin/src/website/mod.rs`

- [x] Write failing tests for `{icon:assets/icons/local.svg}`, outside-source rejection, and unsafe SVG rejection.
- [x] Run targeted tests and verify they fail because icon specs only support Iconify names.
- [x] Extend icon resolution to accept source-relative local SVG paths.
- [x] Generalize sanitizer comments and apply sanitizer to local SVGs.
- [x] Run targeted tests and verify they pass.

### Task 4: Docs and Scaffolds

**Files:**
- Modify: `docs/websites/config.typ`
- Modify: `docs/themes.typ`
- Modify: `docs/calepin.toml`
- Modify: `calepin/src/assets/scaffolds/website/calepin/calepin.toml`
- Modify: `calepin/src/assets/scaffolds/website/academic/calepin.toml`
- Modify tests that assert scaffold contents.

- [x] Replace `[navbar]` examples with `[menus]`.
- [x] Document semantic menu names, custom menu names, `weight`, and local icon tokens.
- [x] Remove stale sidebar `icon` documentation.
- [x] Update scaffold configs to use `menus.main` and `menus.social`.
- [x] Run scaffold and docs-related tests.

### Task 5: Verification and Commit

**Files:**
- All touched files.

- [x] Run `cargo fmt`.
- [x] Run `cargo test --manifest-path calepin/Cargo.toml website`.
- [x] Run broader checks if targeted tests pass.
- [x] Review `git diff`.
- [x] Commit only files changed for this task.
