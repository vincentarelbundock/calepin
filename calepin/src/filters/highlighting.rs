// Syntax highlighting for code blocks using syntect.
//
// - Highlighter::highlight()  — Dispatch to format-specific highlighter (HTML/LaTeX/plain).
// - Highlighter::syntax_css() — Generate CSS for HTML class-based highlighting.
// - Highlighter::latex_color_definitions() — Emit \definecolor commands for LaTeX.
// - LatexColorRegistry        — Allocates and deduplicates named LaTeX colors.

use std::collections::HashMap;
use std::fmt::Write;

use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::html::{ClassStyle, ClassedHTMLGenerator, css_for_theme_with_class_style};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Bundled .tmTheme files (embedded at compile time).
const BUNDLED_THEMES: &[(&str, &[u8])] = &[
    ("1337", include_bytes!("../resources/tmtheme/1337-scheme/1337.tmtheme")),
    ("ansi", include_bytes!("../resources/tmtheme/ansi/ansi.tmTheme")),
    ("base16", include_bytes!("../resources/tmtheme/base16/base16.tmTheme")),
    ("base16-256", include_bytes!("../resources/tmtheme/base16-256/base16-256.tmTheme")),
    ("catppuccin-frappe", include_bytes!("../resources/tmtheme/catppuccin/catppuccin-frappe.tmtheme")),
    ("catppuccin-latte", include_bytes!("../resources/tmtheme/catppuccin/catppuccin-latte.tmtheme")),
    ("catppuccin-macchiato", include_bytes!("../resources/tmtheme/catppuccin/catppuccin-macchiato.tmtheme")),
    ("catppuccin-mocha", include_bytes!("../resources/tmtheme/catppuccin/catppuccin-mocha.tmtheme")),
    ("coldark-cold", include_bytes!("../resources/tmtheme/coldark/coldark-cold.tmtheme")),
    ("coldark-dark", include_bytes!("../resources/tmtheme/coldark/coldark-dark.tmtheme")),
    ("darkneon", include_bytes!("../resources/tmtheme/darkneon/DarkNeon.tmTheme")),
    ("dracula", include_bytes!("../resources/tmtheme/dracula/dracula.tmtheme")),
    ("github", include_bytes!("../resources/tmtheme/github/GitHub.tmTheme")),
    ("gruvbox-dark", include_bytes!("../resources/tmtheme/gruvbox/gruvbox-dark.tmtheme")),
    ("gruvbox-light", include_bytes!("../resources/tmtheme/gruvbox/gruvbox-light.tmtheme")),
    ("monokai-extended", include_bytes!("../resources/tmtheme/monokai-extended/monokai-extended.tmtheme")),
    ("monokai-extended-bright", include_bytes!("../resources/tmtheme/monokai-extended/monokai-extended-bright.tmtheme")),
    ("monokai-extended-light", include_bytes!("../resources/tmtheme/monokai-extended/monokai-extended-light.tmtheme")),
    ("monokai-extended-origin", include_bytes!("../resources/tmtheme/monokai-extended/monokai-extended-origin.tmtheme")),
    ("nord", include_bytes!("../resources/tmtheme/nord/nord.tmtheme")),
    ("onehalf-dark", include_bytes!("../resources/tmtheme/onehalf/onehalfdark.tmtheme")),
    ("onehalf-light", include_bytes!("../resources/tmtheme/onehalf/onehalflight.tmtheme")),
    ("snazzy", include_bytes!("../resources/tmtheme/snazzy/sublime-snazzy.tmtheme")),
    ("solarized-dark-alt", include_bytes!("../resources/tmtheme/solarized/solarized-dark.tmtheme")),
    ("solarized-light-alt", include_bytes!("../resources/tmtheme/solarized/solarized-light.tmtheme")),
    ("twodark", include_bytes!("../resources/tmtheme/twodark/twodark.tmtheme")),
];

/// Resolve a user-facing theme name to an internal key. Returns None for unknown names.
fn resolve_theme_name(name: &str) -> Option<&'static str> {
    match name.to_lowercase().as_str() {
        // syntect built-ins (keep as aliases)
        "inspiredgithub" | "inspired-github" => Some("github"),
        "base16-ocean-dark" | "base16-ocean.dark" => Some("base16-ocean.dark"),
        "base16-ocean-light" | "base16-ocean.light" => Some("base16-ocean.light"),
        "base16-eighties-dark" | "base16-eighties.dark" => Some("base16-eighties.dark"),
        "base16-mocha-dark" | "base16-mocha.dark" => Some("base16-mocha.dark"),
        "solarized-dark" => Some("Solarized (dark)"),
        "solarized-light" => Some("Solarized (light)"),
        // bundled .tmTheme files
        "1337" => Some("1337"),
        "ansi" => Some("ansi"),
        "base16" => Some("base16"),
        "base16-256" => Some("base16-256"),
        "catppuccin-frappe" => Some("catppuccin-frappe"),
        "catppuccin-latte" => Some("catppuccin-latte"),
        "catppuccin-macchiato" => Some("catppuccin-macchiato"),
        "catppuccin-mocha" => Some("catppuccin-mocha"),
        "coldark-cold" => Some("coldark-cold"),
        "coldark-dark" => Some("coldark-dark"),
        "darkneon" | "dark-neon" => Some("darkneon"),
        "dracula" => Some("dracula"),
        "github" => Some("github"),
        "gruvbox-dark" => Some("gruvbox-dark"),
        "gruvbox-light" => Some("gruvbox-light"),
        "monokai-extended" => Some("monokai-extended"),
        "monokai-extended-bright" => Some("monokai-extended-bright"),
        "monokai-extended-light" => Some("monokai-extended-light"),
        "monokai-extended-origin" => Some("monokai-extended-origin"),
        "nord" => Some("nord"),
        "onehalf-dark" | "onehalfdark" => Some("onehalf-dark"),
        "onehalf-light" | "onehalflight" => Some("onehalf-light"),
        "snazzy" => Some("snazzy"),
        "solarized-dark-alt" => Some("solarized-dark-alt"),
        "solarized-light-alt" => Some("solarized-light-alt"),
        "twodark" | "two-dark" => Some("twodark"),
        _ => {
            cwarn!("unknown highlight-style '{}'", name);
            None
        }
    }
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
    MediaQuery,
    /// Use `[data-theme='light']` / `[data-theme='dark']` — for Starlight/Astro.
    DataTheme,
    /// Emit both `@media` and `[data-theme]` rules — for standalone HTML with a theme toggle.
    Both,
}


/// Syntax highlighting engine holding shared state.
///
/// Theme and syntax loading is lazy: when `HighlightConfig::None`, no syntect
/// data structures are allocated. When highlighting is enabled, only the
/// requested theme(s) are parsed — not all 26 bundled themes.
pub struct Highlighter {
    ss: std::cell::OnceCell<SyntaxSet>,
    ts: std::cell::OnceCell<ThemeSet>,
    config: HighlightConfig,
    latex_colors: std::cell::RefCell<LatexColorRegistry>,
}

/// Look up a bundled .tmTheme by internal key name.
fn load_bundled_theme(name: &str) -> Option<syntect::highlighting::Theme> {
    use std::io::Cursor;
    BUNDLED_THEMES.iter()
        .find(|(n, _)| *n == name)
        .and_then(|(_, bytes)| ThemeSet::load_from_reader(&mut Cursor::new(bytes)).ok())
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
                            ss: std::cell::OnceCell::new(),
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
            ss: std::cell::OnceCell::new(),
            ts: std::cell::OnceCell::new(),
            config,
            latex_colors: std::cell::RefCell::new(LatexColorRegistry::new()),
        }
    }

    /// Lazily load the SyntaxSet (only when first needed for highlighting).
    fn syntax_set(&self) -> &SyntaxSet {
        self.ss.get_or_init(SyntaxSet::load_defaults_newlines)
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
    pub fn highlight(&self, code: &str, lang: &str, ext: &str) -> String {
        // For light/dark, we highlight using the light theme for HTML class generation
        // (class names are the same regardless of theme).
        let theme_key = match &self.config {
            HighlightConfig::None => return crate::util::escape_html(code),
            HighlightConfig::Single(k) => k,
            HighlightConfig::LightDark { light, .. } => light,
        };

        let ss = self.syntax_set();
        let syntax = ss
            .find_syntax_by_token(lang)
            .or_else(|| ss.find_syntax_by_name(lang))
            .unwrap_or_else(|| ss.find_syntax_plain_text());

        match ext {
            "html" => self.highlight_html(code, syntax),
            "latex" => self.highlight_latex(code, syntax, theme_key),
            _ => crate::util::escape_html(code),
        }
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
    pub fn syntax_css(&self) -> String {
        self.syntax_css_with_scope(ColorScope::MediaQuery)
    }

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
pub fn parse_highlight_config(yaml: &saphyr::YamlOwned) -> HighlightConfig {
    // String value: single theme
    if let Some(name) = yaml.as_str() {
        return resolve_single_theme(name);
    }

    // Map value: light/dark
    if let Some(light_val) = yaml.as_mapping_get("light") {
        if let Some(dark_val) = yaml.as_mapping_get("dark") {
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
