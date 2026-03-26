// Syntax highlighting for code blocks using syntect.
//
// - Highlighter::highlight()  — Dispatch to format-specific highlighter (HTML/LaTeX/plain).
// - Highlighter::syntax_css() — Generate CSS for HTML class-based highlighting.
// - Highlighter::latex_color_definitions() — Emit \definecolor commands for LaTeX.
// - LatexColorRegistry        — Allocates and deduplicates named LaTeX colors.
// - transform_page            — TransformPage: inject syntax CSS into page template vars.

pub mod transform_page;

use std::collections::HashMap;
use std::fmt::Write;
use std::sync::{LazyLock, Mutex};

use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::html::{ClassStyle, ClassedHTMLGenerator, css_for_theme_with_class_style};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

// ---------------------------------------------------------------------------
// Process-global SyntaxSet singleton
// ---------------------------------------------------------------------------
//
// SyntaxSet::load_defaults_newlines() deserializes ~100 language grammars
// from syntect's compiled binary blob. This takes 3-5ms and allocates ~2MB.
// By making it a process-global singleton, we pay this cost once across all
// documents (important in preview mode where the same process renders many
// files). The SyntaxSet is immutable after construction, so sharing is safe.

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);

// ---------------------------------------------------------------------------
// Process-global highlighting cache
// ---------------------------------------------------------------------------
//
// In preview mode, the same document is re-rendered on every file change.
// Code blocks that haven't changed produce identical highlighted output.
// This cache stores highlighted HTML/LaTeX keyed by a hash of (code, lang, format),
// so unchanged code blocks skip syntect entirely on re-render.
//
// The cache persists for the process lifetime. For a 1000-line document with
// 50 code blocks, this saves ~5-8ms per re-render (the full syntect cost).

static HIGHLIGHT_CACHE: LazyLock<Mutex<HashMap<u64, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn highlight_cache_key(code: &str, lang: &str, ext: &str) -> u64 {
    use xxhash_rust::xxh3::xxh3_64;
    let mut buf = Vec::with_capacity(code.len() + lang.len() + ext.len() + 2);
    buf.extend_from_slice(code.as_bytes());
    buf.push(0);
    buf.extend_from_slice(lang.as_bytes());
    buf.push(0);
    buf.extend_from_slice(ext.as_bytes());
    xxh3_64(&buf)
}

/// Resolve a user-facing theme name to an internal key.
fn resolve_theme_name(name: &str) -> Option<&'static str> {
    // Check if a .tmTheme file exists in the built-in highlighting themes
    let path = format!("{}.tmTheme", name);
    if crate::render::elements::BUILTIN_HIGHLIGHTING.get_file(&path).is_some() {
        // Intern the name so we return &'static str without leaking duplicates.
        static INTERNED: LazyLock<Mutex<std::collections::HashSet<&'static str>>> =
            LazyLock::new(|| Mutex::new(std::collections::HashSet::new()));
        let mut set = INTERNED.lock().unwrap();
        if let Some(existing) = set.get(name) {
            return Some(*existing);
        }
        let leaked: &'static str = Box::leak(name.to_string().into_boxed_str());
        set.insert(leaked);
        return Some(leaked);
    }

    cwarn!("unknown highlight-style '{}'", name);
    None
}

/// User-facing highlight configuration parsed from YAML front matter.
pub enum HighlightConfig {
    /// No highlighting (default, or unrecognized style).
    None,
    /// Single theme for all modes.
    Single(String),
    /// Separate themes for light and dark modes.
    LightDark { light: String, dark: String },
}

/// Strategy for scoping light/dark CSS.
pub enum ColorScope {
    /// Use `@media (prefers-color-scheme: ...)` — for standalone HTML.
    #[allow(dead_code)]
    MediaQuery,
    /// Use `[data-theme='light']` / `[data-theme='dark']` — for Starlight/Astro.
    DataTheme,
    /// Emit both `@media` and `[data-theme]` rules — for standalone HTML with a theme toggle.
    Both,
}


/// Syntax highlighting engine.
///
/// The SyntaxSet is a process-global singleton (loaded once, shared across
/// documents). The ThemeSet is per-Highlighter and loaded lazily, containing
/// only the theme(s) the document actually uses -- not all 26 bundled themes.
pub struct Highlighter {
    ts: std::cell::OnceCell<ThemeSet>,
    config: HighlightConfig,
    latex_colors: std::cell::RefCell<LatexColorRegistry>,
}

/// Load a .tmTheme by name.
///
/// Resolution order:
///   1. Project filesystem: `_calepin/assets/highlighting/{name}.tmTheme`
///   2. Built-in: discovered from embedded project tree
///   3. Filesystem path (for absolute/relative .tmTheme file paths)
fn load_bundled_theme(name: &str) -> Option<syntect::highlighting::Theme> {
    use std::io::Cursor;

    let filename = format!("{}.tmTheme", name);

    // 1. Project filesystem
    let root = crate::paths::get_project_root();
    let project_path = crate::paths::assets_dir(&root).join("highlighting").join(&filename);
    if project_path.exists() {
        if let Ok(theme) = ThemeSet::get_theme(&project_path) {
            return Some(theme);
        }
    }

    // 2. Built-in: embedded highlighting themes
    if let Some(file) = crate::render::elements::BUILTIN_HIGHLIGHTING.get_file(&filename) {
        return ThemeSet::load_from_reader(&mut Cursor::new(file.contents())).ok();
    }

    // 3. Direct filesystem path (user-provided .tmTheme path)
    let path = std::path::Path::new(name);
    if path.exists() {
        return ThemeSet::get_theme(path).ok();
    }

    None
}

/// Check whether a theme key refers to a syntect built-in (not a bundled .tmTheme).
fn is_syntect_builtin(key: &str) -> bool {
    matches!(key,
        "base16-ocean.dark" | "base16-ocean.light" |
        "base16-eighties.dark" | "base16-mocha.dark" |
        "Solarized (dark)" | "Solarized (light)"
    )
}

impl Highlighter {
    pub fn new(config: HighlightConfig) -> Self {
        // Handle custom .tmTheme file path in config — resolve it eagerly so we
        // can report errors early and normalise the config to "_custom".
        let config = if let HighlightConfig::Single(ref key) = config {
            if key.ends_with(".tmTheme") || key.ends_with(".tmtheme") {
                match ThemeSet::get_theme(std::path::Path::new(key)) {
                    Ok(t) => {
                        let ts_cell = std::cell::OnceCell::new();
                        let mut ts = ThemeSet { themes: std::collections::BTreeMap::new() };
                        ts.themes.insert("_custom".to_string(), t);
                        let _ = ts_cell.set(ts);
                        return Self {
                            ts: ts_cell,
                            config: HighlightConfig::Single("_custom".to_string()),
                            latex_colors: std::cell::RefCell::new(LatexColorRegistry::new()),
                        };
                    }
                    Err(e) => {
                        cwarn!("highlight-style '{}': failed to parse: {}", key, e);
                        HighlightConfig::None
                    }
                }
            } else {
                config
            }
        } else {
            config
        };

        Self {
            ts: std::cell::OnceCell::new(),
            config,
            latex_colors: std::cell::RefCell::new(LatexColorRegistry::new()),
        }
    }

    /// Return the process-global SyntaxSet (loaded once, reused across documents).
    fn syntax_set(&self) -> &SyntaxSet {
        &SYNTAX_SET
    }

    /// Lazily load the ThemeSet, containing only the theme(s) actually needed.
    fn theme_set(&self) -> &ThemeSet {
        self.ts.get_or_init(|| {
            let mut ts = ThemeSet { themes: std::collections::BTreeMap::new() };
            let keys: Vec<&str> = match &self.config {
                HighlightConfig::None => vec![],
                HighlightConfig::Single(k) => vec![k.as_str()],
                HighlightConfig::LightDark { light, dark } => vec![light.as_str(), dark.as_str()],
            };
            for key in keys {
                if ts.themes.contains_key(key) {
                    continue;
                }
                if is_syntect_builtin(key) {
                    // Load only the syntect defaults to get this theme
                    let defaults = ThemeSet::load_defaults();
                    if let Some(theme) = defaults.themes.into_iter().find(|(n, _)| n == key) {
                        ts.themes.insert(theme.0, theme.1);
                    }
                } else if let Some(theme) = load_bundled_theme(key) {
                    ts.themes.insert(key.to_string(), theme);
                } else {
                    cwarn!("theme '{}' not found", key);
                }
            }
            ts
        })
    }

    /// Syntax-highlight code for the given output format extension.
    /// Results are cached by (code, lang, format) hash so unchanged code blocks
    /// skip syntect on re-render in preview mode.
    pub fn highlight(&self, code: &str, lang: &str, ext: &str) -> String {
        let theme_key = match &self.config {
            HighlightConfig::None => return crate::util::escape_html(code),
            HighlightConfig::Single(k) => k,
            HighlightConfig::LightDark { light, .. } => light,
        };

        // Check cache (lock is held briefly -- just a HashMap lookup)
        let key = highlight_cache_key(code, lang, ext);
        if let Ok(cache) = HIGHLIGHT_CACHE.lock() {
            if let Some(cached) = cache.get(&key) {
                return cached.clone();
            }
        }

        let ss = self.syntax_set();
        let syntax = ss
            .find_syntax_by_token(lang)
            .or_else(|| ss.find_syntax_by_name(lang))
            .unwrap_or_else(|| ss.find_syntax_plain_text());

        let result = match ext {
            "html" => self.highlight_html(code, syntax),
            "latex" => self.highlight_latex(code, syntax, theme_key),
            _ => crate::util::escape_html(code),
        };

        // Store in cache
        if let Ok(mut cache) = HIGHLIGHT_CACHE.lock() {
            cache.insert(key, result.clone());
        }

        result
    }

    fn theme(&self, key: &str) -> &syntect::highlighting::Theme {
        &self.theme_set().themes[key]
    }

    fn highlight_html(&self, code: &str, syntax: &syntect::parsing::SyntaxReference) -> String {
        let ss = self.syntax_set();
        let mut generator =
            ClassedHTMLGenerator::new_with_class_style(syntax, ss, ClassStyle::Spaced);
        for line in LinesWithEndings::from(code) {
            let _ = generator.parse_html_for_line_which_includes_newline(line);
        }
        generator.finalize()
    }

    fn highlight_latex(&self, code: &str, syntax: &syntect::parsing::SyntaxReference, theme_key: &str) -> String {
        let ss = self.syntax_set();
        let mut h = HighlightLines::new(syntax, self.theme(theme_key));
        let mut output = String::new();
        let mut colors = self.latex_colors.borrow_mut();

        for (i, line) in LinesWithEndings::from(code).enumerate() {
            if i > 0 {
                output.push('\n');
            }
            match h.highlight_line(line, ss) {
                Ok(ranges) => output.push_str(&escape_latex_line(&ranges, &mut colors)),
                Err(_) => output.push_str(&escape_latex(line)),
            }
        }

        output
    }

    /// Generate CSS for syntax highlighting (HTML only).
    ///
    /// For a single theme, emits unscoped CSS.
    /// For light/dark, wraps each theme's CSS in `@media (prefers-color-scheme: ...)`.
    /// Generate CSS with a specific scoping strategy for light/dark themes.
    pub fn syntax_css_with_scope(&self, scope: ColorScope) -> String {
        match &self.config {
            HighlightConfig::None => String::new(),
            HighlightConfig::Single(key) => {
                let mut css = css_for_theme_with_class_style(self.theme(key), ClassStyle::Spaced)
                    .unwrap_or_default();
                self.append_pre_overrides(&mut css, key);
                css
            }
            HighlightConfig::LightDark { light, dark } => {
                let light_css = css_for_theme_with_class_style(self.theme(light), ClassStyle::Spaced)
                    .unwrap_or_default();
                let dark_css = css_for_theme_with_class_style(self.theme(dark), ClassStyle::Spaced)
                    .unwrap_or_default();

                let mut css = String::new();

                let scopes: Vec<(&str, &str)> = match scope {
                    ColorScope::MediaQuery => vec![
                        ("@media (prefers-color-scheme: light)", "@media (prefers-color-scheme: dark)"),
                    ],
                    ColorScope::DataTheme => vec![
                        ("[data-theme='light']", "[data-theme='dark']"),
                    ],
                    ColorScope::Both => vec![
                        ("@media (prefers-color-scheme: light)", "@media (prefers-color-scheme: dark)"),
                        ("[data-theme='light']", "[data-theme='dark']"),
                    ],
                };

                for (light_wrap, dark_wrap) in &scopes {
                    write!(css, "{} {{\n{}", light_wrap, light_css).unwrap();
                    self.append_pre_overrides(&mut css, light);
                    css.push_str("\n}\n");
                    write!(css, "{} {{\n{}", dark_wrap, dark_css).unwrap();
                    self.append_pre_overrides(&mut css, dark);
                    css.push_str("\n}\n");
                }

                css
            }
        }
    }

    /// Append pre background/foreground overrides for a theme.
    fn append_pre_overrides(&self, css: &mut String, theme_key: &str) {
        let theme = self.theme(theme_key);
        let bg = theme.settings.background.unwrap_or(syntect::highlighting::Color::WHITE);
        let fg = theme.settings.foreground.unwrap_or(syntect::highlighting::Color::BLACK);
        let bg_hex = format!("#{:02x}{:02x}{:02x}", bg.r, bg.g, bg.b);
        let fg_hex = format!("#{:02x}{:02x}{:02x}", fg.r, fg.g, fg.b);
        write!(
            css,
            "\npre:has(> code.code), pre:has(> code.output), pre:has(> code.warning), pre:has(> code.error), pre:has(> code.message) {{ background-color: {}; color: {}; }}",
            bg_hex, fg_hex
        ).unwrap();
    }

    /// Emit LaTeX color definitions (call after all elements are rendered).
    pub fn latex_color_definitions(&self) -> String {
        self.latex_colors.borrow().emit_definitions()
    }
}

/// Parse the YAML `highlight-style` value into a HighlightConfig.
/// Accepts a string or a map with `light` and `dark` keys.
pub fn parse_highlight_config(yaml: &crate::value::Value) -> HighlightConfig {
    // String value: single theme
    if let Some(name) = yaml.as_str() {
        return resolve_single_theme(name);
    }

    // Map value: light/dark
    if let Some(light_val) = yaml.get("light") {
        if let Some(dark_val) = yaml.get("dark") {
            if let (Some(light_name), Some(dark_name)) = (light_val.as_str(), dark_val.as_str()) {
                let light = resolve_theme_key(light_name);
                let dark = resolve_theme_key(dark_name);
                if let (Some(l), Some(d)) = (light, dark) {
                    return HighlightConfig::LightDark {
                        light: l.to_string(),
                        dark: d.to_string(),
                    };
                }
            }
        }
    }

    HighlightConfig::None
}

/// Resolve a single theme name to a HighlightConfig.
fn resolve_single_theme(name: &str) -> HighlightConfig {
    if name == "none" || name == "false" {
        return HighlightConfig::None;
    }
    if name.ends_with(".tmTheme") || name.ends_with(".tmtheme") {
        let path = std::path::Path::new(name);
        if !path.exists() {
            cwarn!("highlight-style '{}': file not found", name);
            return HighlightConfig::None;
        }
        return HighlightConfig::Single(name.to_string());
    }
    match resolve_theme_name(name) {
        Some(key) => HighlightConfig::Single(key.to_string()),
        None => HighlightConfig::None,
    }
}

/// Resolve a theme name to its internal key, loading custom .tmTheme files.
fn resolve_theme_key(name: &str) -> Option<&'static str> {
    if name.ends_with(".tmTheme") || name.ends_with(".tmtheme") {
        // Custom file themes need special handling — not supported in light/dark map for now
        cwarn!("custom .tmTheme files not supported in light/dark map");
        return None;
    }
    resolve_theme_name(name)
}

// ---------------------------------------------------------------------------
// LaTeX color registry
// ---------------------------------------------------------------------------

struct LatexColorRegistry {
    map: HashMap<(u8, u8, u8), String>,
    defs: Vec<String>,
    counter: usize,
}

impl LatexColorRegistry {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            defs: Vec::new(),
            counter: 0,
        }
    }

    fn get_or_define(&mut self, r: u8, g: u8, b: u8) -> String {
        let key = (r, g, b);
        if let Some(name) = self.map.get(&key) {
            return name.clone();
        }
        self.counter += 1;
        let name = format!("hl{}", self.counter);
        self.defs.push(format!(
            "\\definecolor{{{}}}{{RGB}}{{{},{},{}}}",
            name, r, g, b
        ));
        self.map.insert(key, name.clone());
        name
    }

    fn emit_definitions(&self) -> String {
        self.defs.join("\n")
    }
}

fn escape_latex_line(ranges: &[(Style, &str)], colors: &mut LatexColorRegistry) -> String {
    let mut s = String::new();
    let mut prev_name: Option<String> = None;

    for &(style, text) in ranges {
        if text == "\n" {
            continue;
        }
        let name = colors.get_or_define(style.foreground.r, style.foreground.g, style.foreground.b);
        if prev_name.as_ref() != Some(&name) {
            if prev_name.is_some() {
                s.push('}');
            }
            let _ = write!(s, "\\textcolor{{{}}}{{", name);
            prev_name = Some(name);
        }
        s.push_str(&escape_latex(text));
    }
    if prev_name.is_some() {
        s.push('}');
    }
    s
}

fn escape_latex(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\textbackslash{}"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '%' => out.push_str("\\%"),
            '#' => out.push_str("\\#"),
            '$' => out.push_str("\\$"),
            '&' => out.push_str("\\&"),
            '_' => out.push_str("\\_"),
            '^' => out.push_str("\\^{}"),
            '~' => out.push_str("\\~{}"),
            _ => out.push(ch),
        }
    }
    out
}
