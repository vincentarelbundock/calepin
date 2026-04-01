// Syntax highlighting for code blocks using syntect.
//
// - Highlighter::highlight()  — Dispatch to format-specific highlighter (HTML/LaTeX/plain).
// - Highlighter::syntax_css() — Generate CSS for HTML class-based highlighting.
// - Highlighter::latex_color_definitions() — Emit \definecolor commands for LaTeX.
// - LatexColorRegistry        — Allocates and deduplicates named LaTeX colors.
// - transform_page            — TransformPage: inject CSS (HTML) or color defs (LaTeX).

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

/// Global LaTeX color registry. Persists across rebuilds so that cached
/// highlight results (which reference `hlN` names) remain valid.
static LATEX_COLOR_REGISTRY: LazyLock<Mutex<LatexColorRegistry>> =
    LazyLock::new(|| Mutex::new(LatexColorRegistry::new()));

/// Process-global theme cache. Themes are expensive to parse from XML (~40ms
/// for bundled .tmTheme files). Since all documents in a batch typically use
/// the same themes, caching them avoids redundant parsing across rayon threads.
static THEME_CACHE: LazyLock<Mutex<HashMap<String, syntect::highlighting::Theme>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Process-global CSS cache. The CSS output is a pure function of the highlight
/// config (theme names), so we cache it to avoid regenerating identical CSS for
/// every file in a batch.
static CSS_CACHE: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn highlight_cache_key(code: &str, lang: &str, writer: &str) -> u64 {
    use xxhash_rust::xxh3::xxh3_64;
    let mut buf = Vec::with_capacity(code.len() + lang.len() + writer.len() + 2);
    buf.extend_from_slice(code.as_bytes());
    buf.push(0);
    buf.extend_from_slice(lang.as_bytes());
    buf.push(0);
    buf.extend_from_slice(writer.as_bytes());
    xxh3_64(&buf)
}

/// Highlight configuration: either disabled or a light/dark theme pair.
pub enum HighlightConfig {
    /// No highlighting.
    None,
    /// Separate themes for light and dark modes.
    LightDark { light: String, dark: String },
}

/// Syntax highlighting engine.
///
/// The SyntaxSet is a process-global singleton (loaded once, shared across
/// documents). The ThemeSet is per-Highlighter and loaded lazily, containing
/// only the theme(s) the document actually uses -- not all 26 bundled themes.
pub struct Highlighter {
    ts: std::cell::OnceCell<ThemeSet>,
    config: HighlightConfig,
}

/// Load a .tmTheme by name.
///
/// Resolution order:
///   1. Project filesystem: `{stem}_calepin/assets/highlighting/{name}.tmTheme`
///   2. Built-in: discovered from embedded project tree
///   3. Filesystem path (for absolute/relative .tmTheme file paths)
fn load_bundled_theme(name: &str) -> Option<syntect::highlighting::Theme> {
    // Check global cache first
    if let Ok(cache) = THEME_CACHE.lock() {
        if let Some(theme) = cache.get(name) {
            return Some(theme.clone());
        }
    }

    let theme = load_bundled_theme_uncached(name)?;

    if let Ok(mut cache) = THEME_CACHE.lock() {
        cache.insert(name.to_string(), theme.clone());
    }

    Some(theme)
}

fn load_bundled_theme_uncached(name: &str) -> Option<syntect::highlighting::Theme> {
    use std::io::Cursor;

    let filename = format!("{}.tmTheme", name);

    // 1. Project filesystem
    let root = crate::paths::get_project_dir();
    let project_path = crate::paths::assets_dir(&root).join("highlighting").join(&filename);
    if project_path.exists() {
        if let Ok(theme) = ThemeSet::get_theme(&project_path) {
            return Some(theme);
        }
    }

    // 2. Built-in theme extension directories (src/themes/{name}/*.tmTheme)
    if let Some(contents) = crate::config::extension::builtin_colors_tmtheme(&filename) {
        return ThemeSet::load_from_reader(&mut Cursor::new(contents)).ok();
    }

    // 3. Direct filesystem path (user-provided .tmTheme path)
    let path = std::path::Path::new(name);
    if path.exists() {
        return ThemeSet::get_theme(path).ok();
    }

    None
}

impl Highlighter {
    /// Create a highlighter from the active color scheme.
    /// Priority: first entry in cfg.colors > extension default_colors > nord fallback.
    pub fn from_metadata(metadata: &crate::config::Metadata) -> Self {
        let target_name = crate::paths::get_active_target().unwrap_or_default();
        let colors = crate::config::extension::resolve_active_colors(&metadata.cfg, &target_name);
        let hl = colors.as_ref().and_then(|c| c.highlight.as_ref());
        let config = HighlightConfig::LightDark {
            light: hl.and_then(|h| h.light.clone()).unwrap_or_else(|| "github".to_string()),
            dark: hl.and_then(|h| h.dark.clone()).unwrap_or_else(|| "nord".to_string()),
        };
        Self::new(config)
    }

    /// Create a highlighter from a given highlight config (for theme CSS generation).
    pub fn from_config(config: HighlightConfig) -> Self {
        Self::new(config)
    }

    /// Create a highlighter with no syntax highlighting (for tests or plain output).
    #[allow(dead_code)]
    pub fn disabled() -> Self {
        Self::new(HighlightConfig::None)
    }

    fn new(config: HighlightConfig) -> Self {
        Self {
            ts: std::cell::OnceCell::new(),
            config,
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
                HighlightConfig::LightDark { light, dark } => vec![light.as_str(), dark.as_str()],
            };
            for key in keys {
                if ts.themes.contains_key(key) {
                    continue;
                }
                if let Some(theme) = load_bundled_theme(key) {
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
    pub fn highlight(&self, code: &str, lang: &str, writer: &str) -> String {
        let theme_key = match &self.config {
            HighlightConfig::None => return crate::util::escape_html(code),
            HighlightConfig::LightDark { light, .. } => light,
        };

        // Check cache (lock is held briefly -- just a HashMap lookup)
        let key = highlight_cache_key(code, lang, writer);
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

        let result = match writer {
            "html" => self.highlight_html(code, syntax),
            "latex" => self.highlight_latex(code, syntax, theme_key),
            _ => crate::util::escape_html(code),
        };

        // Store in cache (including LaTeX, for other consumers)
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
        let mut colors = LATEX_COLOR_REGISTRY.lock().unwrap();

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
    /// For light/dark, wraps each theme's CSS in both `@media (prefers-color-scheme)`
    /// and `[data-theme]` selectors.
    pub fn syntax_css(&self) -> String {
        let cache_key = match &self.config {
            HighlightConfig::None => return String::new(),
            HighlightConfig::LightDark { light, dark } => format!("{}\0{}", light, dark),
        };

        if let Ok(cache) = CSS_CACHE.lock() {
            if let Some(cached) = cache.get(&cache_key) {
                return cached.clone();
            }
        }

        let css = self.generate_syntax_css();

        if let Ok(mut cache) = CSS_CACHE.lock() {
            cache.insert(cache_key, css.clone());
        }

        css
    }

    fn generate_syntax_css(&self) -> String {
        match &self.config {
            HighlightConfig::None => String::new(),
            HighlightConfig::LightDark { light, dark } => {
                let (light_css, light_bg, light_fg) = self.theme_css_and_colors(light);
                let (dark_css, dark_bg, dark_fg) = self.theme_css_and_colors(dark);

                let mut css = String::new();

                let scopes = [
                    ("@media (prefers-color-scheme: light)", "@media (prefers-color-scheme: dark)"),
                    ("[data-theme='light']", "[data-theme='dark']"),
                    ("html:not(.dark)", "html.dark"),
                ];

                for (light_wrap, dark_wrap) in &scopes {
                    write!(css, "{} {{\n{}", light_wrap, light_css).unwrap();
                    Self::append_pre_overrides_static(&mut css, &light_bg, &light_fg);
                    css.push_str("\n}\n");
                    write!(css, "{} {{\n{}", dark_wrap, dark_css).unwrap();
                    Self::append_pre_overrides_static(&mut css, &dark_bg, &dark_fg);
                    css.push_str("\n}\n");
                }

                css
            }
        }
    }

    /// Get CSS and bg/fg colors for a theme (generated at runtime, cached globally).
    fn theme_css_and_colors(&self, key: &str) -> (String, String, String) {
        let theme = self.theme(key);
        let css = css_for_theme_with_class_style(theme, ClassStyle::Spaced)
            .unwrap_or_default();
        let bg = theme.settings.background.unwrap_or(syntect::highlighting::Color::WHITE);
        let fg = theme.settings.foreground.unwrap_or(syntect::highlighting::Color::BLACK);
        (
            css,
            format!("#{:02x}{:02x}{:02x}", bg.r, bg.g, bg.b),
            format!("#{:02x}{:02x}{:02x}", fg.r, fg.g, fg.b),
        )
    }

    fn append_pre_overrides_static(css: &mut String, bg: &str, fg: &str) {
        write!(
            css,
            "\npre:has(> code.code), pre:has(> code.output), pre:has(> code.warning), pre:has(> code.error), pre:has(> code.message) {{ background-color: {}; color: {}; }}",
            bg, fg
        ).unwrap();
    }

    /// Emit LaTeX color definitions (call after all elements are rendered).
    pub fn latex_color_definitions(&self) -> String {
        LATEX_COLOR_REGISTRY.lock().unwrap().emit_definitions()
    }
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
