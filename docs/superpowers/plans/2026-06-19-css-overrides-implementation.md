# CSS Overrides Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add top-level `styles = [...]` in `calepin.toml` so website builds and single-file HTML compiles can append project CSS after a selected base theme.

**Architecture:** Move shared render config (`theme`, `styles`) into `calepin/src/config.rs`, keep website-only settings in `website/config.rs`, and pass resolved CSS overrides through HTML theme resolution. `HtmlEntry.styles` remains the single ordering point; config CSS is appended after shared and theme-local CSS. When `theme = false` and styles are present, use a minimal style-only HTML entry so raw Typst HTML can still receive CSS.

**Tech Stack:** Rust, serde/TOML config parsing, MiniJinja HTML templates, existing Calepin theme pipeline, `make test` / targeted `cargo test --manifest-path calepin/Cargo.toml`.

---

## File Structure

- `calepin/src/config.rs`: Own shared project config: executable paths, optional project theme, config directory, and resolved CSS overrides.
- `calepin/src/website/config.rs`: Keep website-only config. Accept `theme` and `styles` only to satisfy `deny_unknown_fields`; do not resolve them here.
- `calepin/src/theme/html.rs`: Add helper methods to append config CSS to an `HtmlEntry` and to build a minimal style-only entry for `theme = false` plus styles.
- `calepin/src/html/mod.rs`: Re-export the existing stylesheet/script helpers unchanged; no new ownership here.
- `calepin/src/typst/preprocess/mod.rs`: Let single-file and website preprocessing fall back to config `theme`.
- `calepin/src/typst/cli.rs`: Load config CSS for single-file `compile` and pass it into HTML compile options.
- `calepin/src/typst/compile.rs`: Add CSS overrides to `CompileOptions` and append them before applying an HTML theme.
- `calepin/src/website/mod.rs`: Use `CalepinConfig` as source of truth for config theme/styles, include CSS overrides in generated website stylesheet assets, and watch configured CSS files.
- `docs/websites/configuration.typ`: Document `styles`.
- `docs/themes.typ`: Document `styles` as the lightweight CSS-only customization path.

## Task 1: Parse Shared Theme And CSS Overrides In General Config

**Files:**
- Modify: `calepin/src/config.rs`

- [ ] **Step 1: Write failing config tests**

Add these tests to the existing `#[cfg(test)] mod tests` in `calepin/src/config.rs`:

```rust
#[test]
fn config_parses_theme_and_css_styles() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("styles")).unwrap();
    std::fs::write(dir.path().join("styles/site.css"), ":root { --x: 1; }").unwrap();
    std::fs::write(
        dir.path().join("calepin.toml"),
        r#"
theme = "academic"
styles = ["styles/site.css"]
"#,
    )
    .unwrap();

    let config = CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml"))).unwrap();

    assert_eq!(
        config.theme_selection(dir.path()).unwrap(),
        Some(crate::theme::ThemeSelection::Builtin("academic"))
    );
    assert_eq!(config.styles.len(), 1);
    assert_eq!(config.styles[0].name, "site.css");
    assert_eq!(config.styles[0].path, dir.path().join("styles/site.css"));
    assert!(config.styles[0].css.contains("--x: 1"));
}

#[test]
fn config_styles_resolve_relative_to_config_file() {
    let dir = tempfile::tempdir().unwrap();
    let config_dir = dir.path().join("project");
    std::fs::create_dir_all(config_dir.join("styles")).unwrap();
    std::fs::write(config_dir.join("styles/site.css"), "body { color: red; }").unwrap();
    std::fs::write(config_dir.join("calepin.toml"), r#"styles = ["styles/site.css"]"#).unwrap();

    let config = CalepinConfig::load(dir.path(), Some(&config_dir.join("calepin.toml"))).unwrap();

    assert_eq!(config.config_dir, config_dir);
    assert_eq!(config.styles[0].path, config.config_dir.join("styles/site.css"));
}

#[test]
fn config_styles_reject_non_css_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("site.scss"), "body { color: red; }").unwrap();
    std::fs::write(dir.path().join("calepin.toml"), r#"styles = ["site.scss"]"#).unwrap();

    let err = CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml")))
        .unwrap_err()
        .to_string();

    assert!(err.contains("configured style must be a .css file"), "{err}");
    assert!(err.contains("site.scss"), "{err}");
}

#[test]
fn config_styles_reject_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("calepin.toml"), r#"styles = ["missing.css"]"#).unwrap();

    let err = CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml")))
        .unwrap_err()
        .to_string();

    assert!(err.contains("configured style file not found"), "{err}");
    assert!(err.contains("missing.css"), "{err}");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --manifest-path calepin/Cargo.toml config_parses_theme_and_css_styles --lib
cargo test --manifest-path calepin/Cargo.toml config_styles --lib
```

Expected: FAIL because `CalepinConfig` has no `theme_selection`, `config_dir`, or `styles` fields.

- [ ] **Step 3: Implement shared config fields**

In `calepin/src/config.rs`, change `CalepinConfig` and add the CSS override type:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalepinConfig {
    pub executables: ExecutablePaths,
    pub config_dir: PathBuf,
    pub theme: Option<RawThemeValue>,
    pub styles: Vec<CssOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssOverride {
    pub name: String,
    pub path: PathBuf,
    pub css: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum RawThemeValue {
    Enabled(String),
    Toggle(bool),
}
```

Update `CalepinConfig::load` and `default_for_root`:

```rust
impl CalepinConfig {
    pub fn load(root: &Path, config_path: Option<&Path>) -> Result<Self> {
        let Some(path) = config_path else {
            return Ok(Self::default_for_root(root));
        };
        let path = resolve_config_path(path)?;
        if !path.exists() {
            return Err(anyhow!("config file not found: {}", path.display()));
        }
        let contents = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let raw: RawCalepinConfig = toml::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        let config_dir = path.parent().unwrap_or(root).to_path_buf();
        let styles = resolve_css_overrides(&config_dir, raw.styles)?;
        Ok(Self {
            executables: ExecutablePaths::from_raw(root, &config_dir, raw.executables),
            config_dir,
            theme: raw.theme,
            styles,
        })
    }

    fn default_for_root(root: &Path) -> Self {
        Self {
            executables: ExecutablePaths::from_raw(root, root, RawExecutablePaths::default()),
            config_dir: root.to_path_buf(),
            theme: None,
            styles: Vec::new(),
        }
    }

    pub fn theme_selection(&self, base_dir: &Path) -> Result<Option<crate::theme::ThemeSelection>> {
        match &self.theme {
            None => Ok(None),
            Some(RawThemeValue::Toggle(false)) => Ok(Some(crate::theme::ThemeSelection::Disabled)),
            Some(RawThemeValue::Toggle(true)) => Ok(Some(crate::theme::ThemeSelection::Default)),
            Some(RawThemeValue::Enabled(value)) => {
                Ok(Some(crate::theme::ThemeSelection::parse(value, base_dir)?))
            }
        }
    }
}
```

Update `RawCalepinConfig`:

```rust
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawCalepinConfig {
    executables: RawExecutablePaths,
    theme: Option<RawThemeValue>,
    styles: Vec<PathBuf>,
}
```

Add CSS resolution helpers near `resolve_tool_path`:

```rust
fn resolve_css_overrides(config_dir: &Path, paths: Vec<PathBuf>) -> Result<Vec<CssOverride>> {
    paths
        .into_iter()
        .map(|path| resolve_css_override(config_dir, path))
        .collect()
}

fn resolve_css_override(config_dir: &Path, path: PathBuf) -> Result<CssOverride> {
    if path.extension().and_then(|ext| ext.to_str()) != Some("css") {
        return Err(anyhow!(
            "configured style must be a .css file: {}",
            path.display()
        ));
    }
    let resolved = if path.is_absolute() {
        path
    } else {
        config_dir.join(path)
    };
    if !resolved.is_file() {
        return Err(anyhow!(
            "configured style file not found: {}",
            resolved.display()
        ));
    }
    let css = std::fs::read_to_string(&resolved)
        .with_context(|| format!("failed to read configured style {}", resolved.display()))?;
    let name = resolved
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| resolved.display().to_string());
    Ok(CssOverride {
        name,
        path: resolved,
        css,
    })
}
```

- [ ] **Step 4: Run targeted config tests**

Run:

```bash
cargo test --manifest-path calepin/Cargo.toml config_parses_theme_and_css_styles --lib
cargo test --manifest-path calepin/Cargo.toml config_styles --lib
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add calepin/src/config.rs
git commit -m "Add shared config CSS overrides"
```

## Task 2: Move Website Theme Source Of Truth To General Config

**Files:**
- Modify: `calepin/src/website/config.rs`
- Modify: `calepin/src/website/mod.rs`

- [ ] **Step 1: Write failing website config tests**

In `calepin/src/website/mod.rs`, replace the body of `theme_key_parses_builtin_name`, `theme_key_false_disables`, and `missing_theme_key_is_default` so they exercise `CalepinConfig` rather than `WebsiteConfig::theme_selection`. Add this new acceptance test for `styles`:

```rust
#[test]
fn website_config_accepts_shared_styles_key() {
    let config = website_config_from_toml(
        r#"
theme = "academic"
styles = ["styles/site.css"]
"#,
    );

    assert_eq!(config.title, None);
}
```

Use this body for `theme_key_parses_builtin_name`:

```rust
#[test]
fn theme_key_parses_builtin_name() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("calepin.toml"), r#"theme = "academic""#).unwrap();

    let config =
        crate::config::CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml")))
            .unwrap();

    assert_eq!(
        config.theme_selection(&config.config_dir).unwrap(),
        Some(crate::theme::ThemeSelection::Builtin("academic"))
    );
}
```

Use this body for `theme_key_false_disables`:

```rust
#[test]
fn theme_key_false_disables() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("calepin.toml"), "theme = false").unwrap();

    let config =
        crate::config::CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml")))
            .unwrap();

    assert_eq!(
        config.theme_selection(&config.config_dir).unwrap(),
        Some(crate::theme::ThemeSelection::Disabled)
    );
}
```

Use this body for `missing_theme_key_is_default`:

```rust
#[test]
fn missing_theme_key_is_default() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("calepin.toml"), "").unwrap();

    let config =
        crate::config::CalepinConfig::load(dir.path(), Some(&dir.path().join("calepin.toml")))
            .unwrap();

    assert_eq!(config.theme_selection(&config.config_dir).unwrap(), None);
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test --manifest-path calepin/Cargo.toml website_config_accepts_shared_styles_key --lib
cargo test --manifest-path calepin/Cargo.toml theme_key --lib
```

Expected: FAIL because `WebsiteConfig` rejects unknown `styles` and `build_site` still uses `WebsiteConfig::theme_selection`.

- [ ] **Step 3: Adjust website config parsing**

In `calepin/src/website/config.rs`, replace the `theme` field with ignored shared fields:

```rust
#[serde(rename = "executables")]
pub(super) _executables: Option<toml::Value>,
pub(super) theme: Option<toml::Value>,
pub(super) styles: Option<toml::Value>,
```

Delete `RawThemeValue` and `WebsiteConfig::theme_selection`. Leave `RawRobotsConfig` and the rest of `WebsiteConfig` intact.

In `calepin/src/website/mod.rs`, update `build_site` so config theme comes from `calepin_config`:

```rust
let calepin_config = CalepinConfig::load(&src_dir, Some(&config_path))?;
let cli_theme = args
    .theme
    .as_deref()
    .map(|value| crate::theme::ThemeSelection::parse(value, &current_dir))
    .transpose()?;
let config_theme = calepin_config
    .theme_selection(&calepin_config.config_dir)?
    .unwrap_or_default();
let site_theme = cli_theme.clone().unwrap_or_else(|| config_theme.clone());
```

Keep a separate `config_dir` variable for syntax themes:

```rust
let config_dir = config_path.parent().unwrap_or(&src_dir);
let html_syntax_theme = HtmlSyntaxTheme::from_paths(
    config_dir,
    config.highlight_light.as_deref(),
    config.highlight_dark.as_deref(),
)?;
```

- [ ] **Step 4: Run targeted website config tests**

Run:

```bash
cargo test --manifest-path calepin/Cargo.toml website_config_accepts_shared_styles_key --lib
cargo test --manifest-path calepin/Cargo.toml theme_key --lib
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add calepin/src/website/config.rs calepin/src/website/mod.rs
git commit -m "Use shared config for website theme"
```

## Task 3: Add HTML Entry CSS Override Helpers

**Files:**
- Modify: `calepin/src/theme/html.rs`
- Modify: `calepin/src/theme/mod.rs`

- [ ] **Step 1: Write failing unit tests for entry CSS ordering and style-only entry**

Add these tests to `calepin/src/theme/mod.rs` tests:

```rust
#[test]
fn html_entry_appends_config_styles_after_theme_styles() {
    let mut entry = resolve_html_entry(
        &ThemeSelection::Builtin("academic"),
        HtmlScope::Document,
    )
    .unwrap()
    .unwrap();
    entry.append_styles(vec![crate::config::CssOverride {
        name: "site.css".to_string(),
        path: PathBuf::from("/project/styles/site.css"),
        css: "/* config style */".to_string(),
    }]);

    let last = entry.styles.last().unwrap();
    assert_eq!(last.0, "site.css");
    assert_eq!(last.1, "/* config style */");
}

#[test]
fn style_only_entry_preserves_document_shell_and_adds_styles() {
    let entry = crate::theme::style_only_html_entry(vec![crate::config::CssOverride {
        name: "raw.css".to_string(),
        path: PathBuf::from("/project/styles/raw.css"),
        css: "body { color: red; }".to_string(),
    }]);

    assert_eq!(entry.theme_name, "styles");
    assert!(!entry.is_default);
    assert_eq!(entry.styles.len(), 1);
    assert!(entry.layout.contains("{{ doc.head }}"));
    assert!(entry.layout.contains("{{ doc.body }}"));
    assert!(entry.layout.contains("{% for style in styles %}"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --manifest-path calepin/Cargo.toml html_entry_appends_config_styles_after_theme_styles --lib
cargo test --manifest-path calepin/Cargo.toml style_only_entry_preserves_document_shell_and_adds_styles --lib
```

Expected: FAIL because `append_styles` and `style_only_html_entry` do not exist.

- [ ] **Step 3: Implement helpers**

In `calepin/src/theme/html.rs`, add an impl block below `HtmlEntry`:

```rust
impl HtmlEntry {
    pub fn append_styles(&mut self, styles: Vec<crate::config::CssOverride>) {
        self.styles
            .extend(styles.into_iter().map(|style| (style.name, style.css)));
    }
}
```

Add this public function:

```rust
pub fn style_only_html_entry(styles: Vec<crate::config::CssOverride>) -> HtmlEntry {
    let mut entry = HtmlEntry {
        theme_name: "styles".to_string(),
        layout: r#"{{ doc.head }}
{% if site.stylesheet %}
  <link rel="stylesheet" href="{{ site.stylesheet }}">
{% else %}
  {% for style in styles %}
  <style>
{{ style.css }}
  </style>
  {% endfor %}
{% endif %}
{{ doc.body_open }}
{{ doc.body }}
{{ doc.body_close }}"#
            .to_string(),
        partials: Vec::new(),
        styles: Vec::new(),
        scripts: Vec::new(),
        is_default: false,
    };
    entry.append_styles(styles);
    entry
}
```

In `calepin/src/theme/mod.rs`, export the new function:

```rust
pub use html::{
    resolve_explicit_site_html_entry, resolve_html_entry, style_only_html_entry, HtmlEntry,
    HtmlScope,
};
```

- [ ] **Step 4: Run targeted theme tests**

Run:

```bash
cargo test --manifest-path calepin/Cargo.toml html_entry_appends_config_styles_after_theme_styles --lib
cargo test --manifest-path calepin/Cargo.toml style_only_entry_preserves_document_shell_and_adds_styles --lib
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add calepin/src/theme/html.rs calepin/src/theme/mod.rs
git commit -m "Add HTML style override entries"
```

## Task 4: Pass Config Theme And Styles Through Single-File Compile

**Files:**
- Modify: `calepin/src/typst/preprocess/mod.rs`
- Modify: `calepin/src/typst/compile.rs`
- Modify: `calepin/src/typst/cli.rs`

- [ ] **Step 1: Write failing unit test for config theme fallback in preprocessing**

Add this test to `calepin/src/typst/preprocess/mod.rs` tests:

```rust
#[test]
fn preprocess_theme_can_come_from_config() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("paper.typ");
    std::fs::write(&input, "#set document(title: [Paper])\nHello").unwrap();
    std::fs::write(dir.path().join("calepin.toml"), r#"theme = "academic""#).unwrap();

    let plan = prepare_preprocess_plan(PreprocessOptions {
        input,
        root: Some(dir.path().to_path_buf()),
        config: Some(dir.path().join("calepin.toml")),
        display_root: None,
        quiet: true,
        status: false,
        progress: false,
        timeout: None,
        sync_pages: false,
        theme: None,
        fallback_theme: crate::theme::ThemeSelection::Default,
        html_syntax_theme: None,
        param_overrides: Vec::new(),
    })
    .unwrap();

    assert_eq!(plan.theme, crate::theme::ThemeSelection::Builtin("academic"));
}
```

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
cargo test --manifest-path calepin/Cargo.toml preprocess_theme_can_come_from_config --lib
```

Expected: FAIL because `prepare_preprocess_plan` ignores config `theme`.

- [ ] **Step 3: Use config theme as fallback**

In `prepare_preprocess_plan`, after `let config = CalepinConfig::load(...)`, compute:

```rust
let config_theme = config.theme_selection(&config.config_dir)?;
```

Then replace the effective theme expression with:

```rust
let effective_theme = options
    .theme
    .clone()
    .or(setup_config.defaults.theme_selection(&layout.root)?)
    .or(config_theme)
    .unwrap_or_else(|| options.fallback_theme.clone());
```

- [ ] **Step 4: Add compile option for CSS overrides**

In `calepin/src/typst/compile.rs`, add a field to `CompileOptions<'a>`:

```rust
pub config_styles: &'a [crate::config::CssOverride],
```

Update every `CompileOptions` construction to include `config_styles: &[]` until the next step wires real values.

- [ ] **Step 5: Append styles before HTML theme application**

In `compile_with_typst`, replace the HTML entry resolution block with:

```rust
let resolved_html_entry = if options.format == Some("html") && options.html_entry.is_none() {
    crate::theme::resolve_html_entry(options.theme, options.html_scope)?
} else {
    None
};
let style_only_entry;
let mut owned_html_entry;
let html_entry = if options.format == Some("html") {
    let base = options.html_entry.or(resolved_html_entry.as_ref());
    if let Some(entry) = base {
        owned_html_entry = entry.clone();
        owned_html_entry.append_styles(options.config_styles.to_vec());
        Some(&owned_html_entry)
    } else if !options.config_styles.is_empty() {
        style_only_entry = crate::theme::style_only_html_entry(options.config_styles.to_vec());
        Some(&style_only_entry)
    } else {
        None
    }
} else {
    None
};
```

If `HtmlEntry` does not derive `Clone`, add `#[derive(Clone)]` to it in `calepin/src/theme/html.rs`.

- [ ] **Step 6: Load config styles in single-file compile**

In `calepin/src/typst/cli.rs`, after computing `current_dir`, load the config once:

```rust
let calepin_config = crate::config::CalepinConfig::load(&current_dir, args.common.config.as_deref())?;
let config_styles = calepin_config.styles.clone();
```

Keep `PreprocessOptions.config` unchanged so preprocessing still loads executables from config. In the `CompileOptions` construction, pass:

```rust
config_styles: &config_styles,
```

- [ ] **Step 7: Run targeted tests**

Run:

```bash
cargo test --manifest-path calepin/Cargo.toml preprocess_theme_can_come_from_config --lib
cargo test --manifest-path calepin/Cargo.toml compile::tests --lib
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add calepin/src/typst/preprocess/mod.rs calepin/src/typst/compile.rs calepin/src/typst/cli.rs calepin/src/theme/html.rs
git commit -m "Apply config styles to document HTML"
```

## Task 5: Include Config CSS In Website Generated Assets

**Files:**
- Modify: `calepin/src/website/mod.rs`

- [ ] **Step 1: Write failing asset-ordering test**

Add this test to `calepin/src/website/mod.rs` tests:

```rust
#[test]
fn theme_generated_assets_include_config_styles_last() {
    let mut entry = crate::theme::resolve_html_entry(
        &crate::theme::ThemeSelection::Default,
        crate::theme::HtmlScope::Site,
    )
    .unwrap()
    .unwrap();
    entry.append_styles(vec![crate::config::CssOverride {
        name: "site.css".to_string(),
        path: PathBuf::from("/project/styles/site.css"),
        css: "/* config style */\n:root { --site: yes; }".to_string(),
    }]);

    let assets = ThemeGeneratedAssets::from_entry(&entry, &HtmlSyntaxTheme::builtin()).unwrap();
    let stylesheet = assets.stylesheet.as_ref().unwrap();

    let theme_pos = stylesheet.content.find(".calepin-website-shell").unwrap();
    let override_pos = stylesheet.content.find("--site: yes").unwrap();
    assert!(override_pos > theme_pos);
}
```

- [ ] **Step 2: Run test to verify current behavior**

Run:

```bash
cargo test --manifest-path calepin/Cargo.toml theme_generated_assets_include_config_styles_last --lib
```

Expected: PASS if Task 3 already made `ThemeGeneratedAssets::from_entry` naturally include appended styles. If it fails, fix `html_theme_stylesheet` usage so `entry.styles` is the only CSS source.

- [ ] **Step 3: Wire website build site entries with config styles**

In `build_site`, after resolving `site_entry`, create a styled entry:

```rust
let site_entry = crate::theme::resolve_html_entry(&site_theme, crate::theme::HtmlScope::Site)?;
let site_entry = match site_entry {
    Some(mut entry) => {
        entry.append_styles(calepin_config.styles.clone());
        Some(entry)
    }
    None if !calepin_config.styles.is_empty() => {
        Some(crate::theme::style_only_html_entry(calepin_config.styles.clone()))
    }
    None => None,
};
let mut external_theme_assets = site_entry
    .as_ref()
    .is_some_and(|entry| entry.is_default || !calepin_config.styles.is_empty());
```

When constructing `theme_assets`, prefer the styled `site_entry` when styles are present:

```rust
let theme_assets = if external_theme_assets {
    let entry = if let Some(entry) = site_entry.as_ref() {
        entry.clone()
    } else {
        crate::theme::resolve_html_entry(
            &crate::theme::ThemeSelection::Default,
            crate::theme::HtmlScope::Site,
        )?
        .expect("default theme must provide a site entry")
    };
    ThemeGeneratedAssets::from_entry(&entry, &html_syntax_theme)?
} else {
    ThemeGeneratedAssets::default()
};
```

Pass the styled entry through website render context where the existing code currently resolves entries per page. For each page render, append `calepin_config.styles.clone()` to the resolved entry before calling `compile_with_typst`. The exact pattern should match this helper shape in `website/mod.rs`:

```rust
fn html_entry_with_config_styles(
    entry: Option<crate::theme::HtmlEntry>,
    styles: &[crate::config::CssOverride],
) -> Option<crate::theme::HtmlEntry> {
    match entry {
        Some(mut entry) => {
            entry.append_styles(styles.to_vec());
            Some(entry)
        }
        None if !styles.is_empty() => Some(crate::theme::style_only_html_entry(styles.to_vec())),
        None => None,
    }
}
```

- [ ] **Step 4: Add config styles to render context**

Add this field to `WebsiteRenderContext`:

```rust
config_styles: Vec<crate::config::CssOverride>,
```

When building the context, set:

```rust
config_styles: calepin_config.styles.clone(),
```

When constructing `CompileOptions` for pages, pass:

```rust
config_styles: &context.config_styles,
```

- [ ] **Step 5: Run website asset tests**

Run:

```bash
cargo test --manifest-path calepin/Cargo.toml theme_generated_assets_include_config_styles_last --lib
cargo test --manifest-path calepin/Cargo.toml theme_generated_assets_use_fingerprinted_calepin_paths --lib
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add calepin/src/website/mod.rs
git commit -m "Include config styles in website assets"
```

## Task 6: Watch Configured CSS Files In Website Watch

**Files:**
- Modify: `calepin/src/website/mod.rs`

- [ ] **Step 1: Write failing watch-result test**

Add `style_paths` to `WebsiteBuildResult`:

```rust
style_paths: Vec<PathBuf>,
```

Add this test near other website tests:

```rust
#[test]
fn website_build_result_tracks_config_style_paths() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("styles")).unwrap();
    std::fs::write(root.join("styles/site.css"), "body { color: red; }").unwrap();
    std::fs::write(
        root.join("calepin.toml"),
        r#"
theme = "calepin"
styles = ["styles/site.css"]
"#,
    )
    .unwrap();
    std::fs::write(root.join("index.typ"), "#set document(title: [Home])\nHome").unwrap();

    let result = build_site(WebsiteBuildOptions {
        src: Some(root.to_path_buf()),
        out: Some(root.join("out")),
        config: root.join("calepin.toml"),
        theme: None,
        format: Some("html".to_string()),
        minify: false,
        quiet: true,
        timeout: None,
        params: Vec::new(),
        typst_args: Vec::new(),
        incremental_inputs: None,
        parallelism: Some(1),
    })
    .unwrap();

    assert_eq!(result.style_paths, vec![root.join("styles/site.css")]);
}
```

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
cargo test --manifest-path calepin/Cargo.toml website_build_result_tracks_config_style_paths --lib
```

Expected: FAIL until `WebsiteBuildResult` carries style paths.

- [ ] **Step 3: Populate and watch style paths**

In `WebsiteBuildResult`, add:

```rust
style_paths: Vec<PathBuf>,
```

In the `Ok(WebsiteBuildResult { ... })` returned by `build_site`, add:

```rust
style_paths: calepin_config.styles.iter().map(|style| style.path.clone()).collect(),
```

In `watch_website`, where watch roots are assembled from the latest build result, append each configured CSS file:

```rust
for style_path in &current.style_paths {
    watches.push((style_path.clone(), RecursiveMode::NonRecursive));
}
```

Also include initial style paths in the change classifier if it checks only config path and theme dir. The behavior should be: a changed configured CSS path triggers a full rebuild, like a changed theme asset.

- [ ] **Step 4: Run targeted watch/style test**

Run:

```bash
cargo test --manifest-path calepin/Cargo.toml website_build_result_tracks_config_style_paths --lib
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add calepin/src/website/mod.rs
git commit -m "Watch configured website styles"
```

## Task 7: Add End-To-End HTML Behavior Tests

**Files:**
- Modify: `calepin/tests/typst_preprocess.rs`

- [ ] **Step 1: Add integration tests**

Add tests following the existing integration-test helper style in `calepin/tests/typst_preprocess.rs`:

```rust
#[test]
fn compile_html_config_styles_append_after_theme_css() {
    if !has_command("typst") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::create_dir_all(dir.path().join("styles")).unwrap();
    std::fs::write(
        dir.path().join("paper.typ"),
        r#"#set document(title: [Paper])
Hello
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("calepin.toml"),
        r#"
theme = "academic"
styles = ["styles/site.css"]
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("styles/site.css"),
        ":root { --config-style-marker: yes; }",
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args([
            "compile",
            "paper.typ",
            "paper.html",
            "--format",
            "html",
            "--config",
            "calepin.toml",
            "--quiet",
        ])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");
    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let html = std::fs::read_to_string(dir.path().join("paper.html")).unwrap();
    let theme_pos = html.find("--calepin-color-background").unwrap();
    let config_pos = html.find("--config-style-marker").unwrap();
    assert!(config_pos > theme_pos, "{html}");
}

#[test]
fn compile_html_theme_false_still_applies_config_styles() {
    if !has_command("typst") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::create_dir_all(dir.path().join("styles")).unwrap();
    std::fs::write(
        dir.path().join("paper.typ"),
        r#"#set document(title: [Paper])
Hello
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("calepin.toml"),
        r#"
theme = false
styles = ["styles/raw.css"]
"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("styles/raw.css"),
        "body { --raw-style-marker: yes; }",
    )
    .unwrap();

    let output = Command::new(calepin_bin())
        .args([
            "compile",
            "paper.typ",
            "paper.html",
            "--format",
            "html",
            "--config",
            "calepin.toml",
            "--quiet",
        ])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");
    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let html = std::fs::read_to_string(dir.path().join("paper.html")).unwrap();
    assert!(html.contains("--raw-style-marker"), "{html}");
    assert!(!html.contains("calepin-copy-code"), "{html}");
}

#[test]
fn compile_pdf_ignores_config_styles() {
    if !has_command("typst") {
        return;
    }

    let dir = typst_accessible_tempdir();
    std::fs::create_dir_all(dir.path().join("styles")).unwrap();
    std::fs::write(
        dir.path().join("paper.typ"),
        r#"#set document(title: [Paper])
Hello
"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("calepin.toml"), r#"styles = ["styles/site.css"]"#)
        .unwrap();
    std::fs::write(dir.path().join("styles/site.css"), "body { color: red; }").unwrap();

    let output = Command::new(calepin_bin())
        .args([
            "compile",
            "paper.typ",
            "paper.pdf",
            "--format",
            "pdf",
            "--config",
            "calepin.toml",
            "--quiet",
        ])
        .current_dir(dir.path())
        .output()
        .expect("failed to run calepin compile");
    assert!(
        output.status.success(),
        "compile failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(dir.path().join("paper.pdf").is_file());
}
```

- [ ] **Step 2: Run integration tests**

Run:

```bash
cargo test --manifest-path calepin/Cargo.toml compile_html_config_styles_append_after_theme_css --test typst_preprocess
cargo test --manifest-path calepin/Cargo.toml compile_html_theme_false_still_applies_config_styles --test typst_preprocess
cargo test --manifest-path calepin/Cargo.toml compile_pdf_ignores_config_styles --test typst_preprocess
```

Expected: PASS. If `typst` is missing, these tests should follow the existing skip behavior used by this test file.

- [ ] **Step 3: Commit**

```bash
git add calepin/tests/typst_preprocess.rs
git commit -m "Test config CSS overrides in compiles"
```

## Task 8: Document CSS Overrides

**Files:**
- Modify: `docs/websites/configuration.typ`
- Modify: `docs/themes.typ`

- [ ] **Step 1: Update website configuration docs**

In `docs/websites/configuration.typ`, add this after the `theme = "academic"` example:

```typst
# Extra CSS files loaded after the selected theme. Paths are relative to
# calepin.toml. Use this for small project-specific visual tweaks.
styles = ["styles/site.css"]
```

In the paragraph beginning `Paths in calepin.toml are interpreted`, add `styles`
to the list of path-like settings.

- [ ] **Step 2: Update theme docs**

In `docs/themes.typ`, after the built-in theme list and before `= Ejecting and local themes`, add:

````typst
= CSS overrides

For CSS-only changes, keep the base theme and append project styles from
`calepin.toml`:

```toml
theme = "academic"
styles = ["styles/site.css"]
```

The files load after the theme's own CSS, in the order listed. Paths are
relative to `calepin.toml`. Prefer stable `--calepin-*` tokens for broad visual
customization, and use local or ejected themes when you need to change HTML
templates, JavaScript, or bundled assets.

`theme = false` disables the base theme but still loads `styles`, which gives a
raw HTML plus user CSS mode.
````

- [ ] **Step 3: Run docs build check**

Run:

```bash
make website
```

Expected: PASS and regenerated docs if the repository expects built docs to be checked in.

- [ ] **Step 4: Commit**

```bash
git add docs/websites/configuration.typ docs/themes.typ docs/websites/configuration.html docs/websites/configuration.pdf docs/themes.html docs/themes.pdf
git commit -m "Document config CSS overrides"
```

If `make website` does not regenerate one of the listed HTML/PDF files, omit that absent file from `git add`.

## Task 9: Final Verification

**Files:**
- No new files. This task verifies the accumulated branch.

- [ ] **Step 1: Run formatter**

Run:

```bash
cargo fmt --manifest-path calepin/Cargo.toml
```

Expected: command exits 0.

- [ ] **Step 2: Run fast check**

Run:

```bash
make check
```

Expected: command exits 0.

- [ ] **Step 3: Run full test suite**

Run:

```bash
make test
```

Expected: command exits 0. Integration tests may skip external-tool-dependent assertions if the required tools are unavailable; do not treat a skip as coverage for behavior that can be tested locally.

- [ ] **Step 4: Inspect final diff**

Run:

```bash
git status --short
git diff --stat HEAD
```

Expected: only files from this plan are modified or added.

- [ ] **Step 5: Commit verification-only changes if formatting touched files**

If `cargo fmt` changed files not already committed, run:

```bash
git add calepin/src/config.rs calepin/src/theme/html.rs calepin/src/theme/mod.rs calepin/src/typst/preprocess/mod.rs calepin/src/typst/compile.rs calepin/src/typst/cli.rs calepin/src/website/config.rs calepin/src/website/mod.rs
git commit -m "Format CSS override implementation"
```

If there are no formatting changes, do not create a commit.

## Self-Review Notes

- Spec coverage: top-level `styles`, config-relative paths, HTML-only behavior, website and single-file compiles, `theme = false`, generated asset hashing, watch behavior, validation, and docs are all covered by tasks.
- Scope check: no CLI `--style`, no TOML token table, no JS override support, no paged CSS support.
- Type consistency: `CssOverride`, `RawThemeValue`, `CalepinConfig::theme_selection`, `HtmlEntry::append_styles`, and `style_only_html_entry` are introduced before dependent tasks use them.
