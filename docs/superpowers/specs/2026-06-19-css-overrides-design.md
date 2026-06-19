# CSS overrides on top of base themes

Date: 2026-06-19
Status: Approved design, ready for implementation planning

## Summary

Add a top-level `styles` key to `calepin.toml` so users can keep a base theme
and append project CSS files after that theme's own CSS.

The intended user flow is:

```toml
theme = "academic"
styles = [
  "styles/tufte.css",
  "styles/margin-figure.css",
]
```

This gives users the common "base theme plus local tweaks" workflow without
ejecting a full theme directory and without treating a small CSS patch as a
complete theme.

## Motivation

The current customization paths are too coarse for simple visual overrides.
Users can:

- select a built-in theme,
- disable theming,
- or eject/copy an entire theme directory and edit it.

That is appropriate for layout, template, and JavaScript changes, but it is too
heavy for cases like applying the staged `tufte/` CSS on top of the built-in
`academic` theme. The project already exposes stable `--calepin-*` CSS tokens;
users need a small, documented way to load CSS that overrides those tokens or
adds a few targeted rules.

## User-facing API

`calepin.toml` accepts a top-level `styles` array:

```toml
theme = "academic"
styles = ["styles/tufte.css", "styles/margin-figure.css"]
```

Semantics:

- `styles` is an ordered list of CSS file paths.
- Paths resolve relative to the config file.
- CSS files are loaded after the selected theme's CSS, in listed order.
- The key applies only to HTML output.
- The same key works for website builds and single-file compiles when the user
  passes `--config calepin.toml`.
- `styles = []` is a valid no-op.

Theme selection precedence remains unchanged:

1. CLI `--theme`
2. document `#calepin.setup(theme: ...)`
3. config `theme`
4. default theme

Config `styles` remain attached to the config file even when the base theme is
chosen by CLI or document setup. This allows a project to define its common CSS
patches once while still letting individual builds select a different base
theme.

## `theme = false`

`theme = false` disables the base Calepin theme, but configured `styles` still
apply to HTML output. This creates a useful raw-HTML-plus-CSS mode:

```toml
theme = false
styles = ["styles/raw.css"]
```

The behavior is intentionally distinct from "ignore all project styling." If a
future need appears, that can be handled with an explicit CLI switch such as
`--no-config-styles`; it should not be overloaded onto `theme = false`.

## Configuration model

This is a shared render-config feature, not a website-only feature.

Extend the general config loader so `CalepinConfig` carries:

- executable paths, as today,
- the config directory or resolved config path,
- optional raw `theme`,
- ordered `styles`.

Website-specific config can continue to parse its own site keys. The shared
render keys (`theme` and `styles`) should have one source of truth in the
general config layer, and website code should consume that resolved value rather
than reimplementing path resolution. The important contract is that single-file
compiles using `--config calepin.toml` and website builds interpret the same
top-level `styles` key identically.

## CSS resolution

For each configured style path:

1. Resolve relative paths against the config file's directory.
2. Require the resolved path to be a file.
3. Require the extension to be `.css`.
4. Read the file as UTF-8 text.
5. Preserve the listed order.

Missing files, directories, invalid extensions, and unreadable files are hard
errors that include the resolved path.

## Rendering behavior

HTML theme resolution must append config CSS after the selected theme's CSS.
The existing `HtmlEntry.styles` ordering already represents the right model:
shared imports first, then theme-local files. Config styles become the final
entries in that list.

For websites:

- Include config CSS in the generated fingerprinted
  `.calepin/calepin-website.<hash>.css`.
- Page templates continue linking one generated stylesheet as they do today.
- Changing a configured CSS file must invalidate the generated stylesheet and
  trigger the normal website rebuild/watch behavior.

For single-file HTML compiles:

- Inline config CSS through the same template `styles` loop used by theme CSS.
- `calepin compile paper.typ --format html --config calepin.toml` should produce
  themed HTML containing the configured CSS after base theme CSS.

For paged, SVG, and PNG output:

- Ignore `styles`; the key is HTML-only.
- Do not warn for non-HTML output, because the same config may reasonably be
  shared across HTML and paged builds.

## Cache and watch

Configured CSS file contents are part of the HTML rendering inputs.

- Website builds must include the CSS contents in the generated asset hash,
  either directly through the combined stylesheet content or through the
  existing generated-asset hashing path.
- Website watch must treat configured CSS paths like theme assets: changes to
  those files trigger rebuilds.
- Single-file preprocessing does not need CSS in the execution fingerprint
  because CSS does not affect chunk execution or the Typst render wrapper. HTML
  theming happens after Typst writes HTML.

## Error handling

Errors should be early and concrete:

- `styles = "styles/site.css"` is invalid; use an array.
- `styles = ["styles/site.scss"]` errors because the file is not `.css`.
- `styles = ["styles"]` errors because the path is a directory.
- `styles = ["missing.css"]` errors with the resolved path.

The config parser should continue rejecting unknown keys where it already does
so. Adding `styles` must not weaken that validation.

## Documentation

Update the configuration and theme documentation to show:

```toml
theme = "academic"
styles = ["styles/site.css"]
```

Explain that CSS files load after the theme and should prefer stable
`--calepin-*` tokens for broad visual customization.

The docs should distinguish the three customization levels:

1. `styles` for CSS-only project tweaks.
2. local/ejected themes for template, JavaScript, or bundled asset changes.
3. `theme = false` plus `styles` for raw HTML with user CSS.

## Testing

Behavior-focused tests:

- Config parsing accepts `styles = ["styles/site.css"]`.
- Relative style paths resolve against the config file's directory.
- Invalid style paths produce useful errors.
- Website generated stylesheet includes config CSS after theme CSS.
- Single-file HTML compile with `--config` includes config CSS after theme CSS.
- `theme = false` plus `styles` still emits user CSS in HTML output.
- Non-HTML compiles tolerate `styles` without warning or failure.
- Website watch recognizes configured CSS paths as rebuild inputs.

Avoid tests that pin exact full generated HTML or byte output.

## Out of scope

- Structured TOML token overrides such as `[theme.tokens]`.
- JavaScript override files in `calepin.toml`.
- Per-page CSS lists.
- CLI `--style` flags.
- CSS preprocessing, bundling, minification beyond the existing HTML minifier.
- Applying these CSS files to paged/PDF output.

## Affected areas

- `calepin/src/config.rs`
- `calepin/src/website/config.rs`
- `calepin/src/website/mod.rs`
- `calepin/src/theme/html.rs`
- `calepin/src/html/theme.rs`
- `calepin/src/typst/cli.rs`
- `calepin/src/typst/compile.rs`
- website watch path handling
- tests under `calepin/src/` and `calepin/tests/`
- `docs/websites/configuration.typ`
- `docs/themes.typ`
