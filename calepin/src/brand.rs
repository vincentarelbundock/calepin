//! `_brand.yml` support: colors, logos, typography, and CSS generation.
//!
//! Implements a subset of the brand.yml specification
//! (<https://posit-dev.github.io/brand-yml/>).
//!
//! Brand data is loaded lazily on first access from `_brand.yml` in the
//! current directory.

use std::collections::HashMap;
use std::sync::LazyLock;

use saphyr::{LoadableYamlNode, YamlOwned, ScalarOwned};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

pub struct Brand {
    pub color: BrandColor,
    pub logo: BrandLogo,
    pub typography: BrandTypography,
    pub meta: HashMap<String, String>,
}

pub struct BrandColor {
    pub semantic: HashMap<String, ColorValue>,
}

#[derive(Clone)]
pub enum ColorValue {
    Flat(String),
    Themed { light: Option<String>, dark: Option<String> },
}

pub struct BrandLogo {
    pub small: Option<LogoSlot>,
    pub medium: Option<LogoSlot>,
    pub large: Option<LogoSlot>,
}

#[derive(Clone)]
pub struct LogoImage {
    pub path: String,
    pub alt: Option<String>,
}

#[derive(Clone)]
pub enum LogoSlot {
    Single(LogoImage),
    Themed { light: Option<LogoImage>, dark: Option<LogoImage> },
}

pub struct BrandTypography {
    pub fonts: Vec<FontDef>,
    pub base: Option<String>,
    pub headings: Option<String>,
    pub monospace: Option<String>,
}

pub struct FontDef {
    pub source: String,
    pub family: String,
}

// ---------------------------------------------------------------------------
// Lazy loading
// ---------------------------------------------------------------------------

static BRAND: LazyLock<Option<Brand>> = LazyLock::new(|| {
    let content = std::fs::read_to_string("_brand.yml")
        .or_else(|_| std::fs::read_to_string("_brand.yaml"))
        .ok()?;
    if content.is_empty() {
        return None;
    }
    match parse_brand_yaml(&content) {
        Ok(b) => Some(b),
        Err(e) => {
            cwarn!("Failed to parse _brand.yml: {}", e);
            None
        }
    }
});

/// Access the lazily loaded brand data.
pub fn brand() -> Option<&'static Brand> {
    BRAND.as_ref()
}

// ---------------------------------------------------------------------------
// YAML helpers
// ---------------------------------------------------------------------------

fn yaml_key(key: &str) -> YamlOwned {
    YamlOwned::Value(ScalarOwned::String(key.to_string()))
}

fn yaml_get<'a>(map: &'a saphyr::MappingOwned, key: &str) -> Option<&'a YamlOwned> {
    map.get(&yaml_key(key))
}

fn yaml_str<'a>(map: &'a saphyr::MappingOwned, key: &str) -> Option<&'a str> {
    yaml_get(map, key).and_then(|v| v.as_str())
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn parse_brand_yaml(content: &str) -> anyhow::Result<Brand> {
    let docs = YamlOwned::load_from_str(content)?;
    let root = docs.into_iter().next().unwrap_or(YamlOwned::BadValue);
    let empty = saphyr::MappingOwned::new();
    let root_map = root.as_mapping().unwrap_or(&empty);

    let color = parse_color(root_map);
    let logo = parse_logo(root_map);
    let typography = parse_typography(root_map);
    let meta = parse_meta(root_map);

    Ok(Brand { color, logo, typography, meta })
}

fn parse_color(root: &saphyr::MappingOwned) -> BrandColor {
    let empty_map = saphyr::MappingOwned::new();
    let color_val = yaml_get(root, "color");
    let color_map = color_val.and_then(|v| v.as_mapping()).unwrap_or(&empty_map);

    // Parse palette
    let mut palette = HashMap::new();
    if let Some(pal_map) = yaml_get(color_map, "palette").and_then(|v| v.as_mapping()) {
        for (k, v) in pal_map {
            if let (Some(name), Some(hex)) = (k.as_str(), v.as_str()) {
                palette.insert(name.to_string(), hex.to_string());
            }
        }
    }

    // Parse semantic colors
    let mut semantic = HashMap::new();
    let semantic_keys = [
        "foreground", "background", "primary", "secondary",
        "success", "info", "warning", "danger", "light", "dark",
    ];
    for key in semantic_keys {
        if let Some(val) = yaml_get(color_map, key) {
            if let Some(cv) = parse_color_value(val, &palette) {
                semantic.insert(key.to_string(), cv);
            }
        }
    }

    BrandColor { semantic }
}

fn parse_color_value(val: &YamlOwned, palette: &HashMap<String, String>) -> Option<ColorValue> {
    if let Some(s) = val.as_str() {
        return Some(ColorValue::Flat(resolve_color(s, palette)));
    }
    if let Some(map) = val.as_mapping() {
        let light = yaml_str(map, "light").map(|s| resolve_color(s, palette));
        let dark = yaml_str(map, "dark").map(|s| resolve_color(s, palette));
        if light.is_some() || dark.is_some() {
            return Some(ColorValue::Themed { light, dark });
        }
    }
    None
}

fn resolve_color(val: &str, palette: &HashMap<String, String>) -> String {
    if val.starts_with('#') {
        val.to_string()
    } else {
        palette.get(val).cloned().unwrap_or_else(|| val.to_string())
    }
}

fn parse_logo(root: &saphyr::MappingOwned) -> BrandLogo {
    let empty_map = saphyr::MappingOwned::new();
    let logo_val = yaml_get(root, "logo");
    let logo_map = logo_val.and_then(|v| v.as_mapping()).unwrap_or(&empty_map);

    // Parse named images
    let mut images = HashMap::new();
    if let Some(img_map) = yaml_get(logo_map, "images").and_then(|v| v.as_mapping()) {
        for (k, v) in img_map {
            if let Some(name) = k.as_str() {
                if let Some(img) = parse_logo_image(v) {
                    images.insert(name.to_string(), img);
                }
            }
        }
    }

    let small = yaml_get(logo_map, "small").and_then(|v| parse_logo_slot(v, &images));
    let medium = yaml_get(logo_map, "medium").and_then(|v| parse_logo_slot(v, &images));
    let large = yaml_get(logo_map, "large").and_then(|v| parse_logo_slot(v, &images));

    BrandLogo { small, medium, large }
}

fn parse_logo_image(val: &YamlOwned) -> Option<LogoImage> {
    if let Some(s) = val.as_str() {
        return Some(LogoImage { path: s.to_string(), alt: None });
    }
    if let Some(map) = val.as_mapping() {
        let path = yaml_str(map, "path")?.to_string();
        let alt = yaml_str(map, "alt").map(String::from);
        return Some(LogoImage { path, alt });
    }
    None
}

fn parse_logo_slot(val: &YamlOwned, images: &HashMap<String, LogoImage>) -> Option<LogoSlot> {
    // String: direct path or named reference
    if let Some(s) = val.as_str() {
        let img = resolve_logo_ref(s, images);
        return Some(LogoSlot::Single(img));
    }

    if let Some(map) = val.as_mapping() {
        // Check for light/dark themed
        let has_light = yaml_get(map, "light").is_some();
        let has_dark = yaml_get(map, "dark").is_some();
        if has_light || has_dark {
            let light = yaml_get(map, "light").and_then(|v| {
                if let Some(s) = v.as_str() { Some(resolve_logo_ref(s, images)) }
                else { parse_logo_image(v) }
            });
            let dark = yaml_get(map, "dark").and_then(|v| {
                if let Some(s) = v.as_str() { Some(resolve_logo_ref(s, images)) }
                else { parse_logo_image(v) }
            });
            return Some(LogoSlot::Themed { light, dark });
        }

        // Otherwise it's a {path, alt} object
        let img = parse_logo_image(val)?;
        return Some(LogoSlot::Single(img));
    }

    None
}

fn resolve_logo_ref(name: &str, images: &HashMap<String, LogoImage>) -> LogoImage {
    if let Some(img) = images.get(name) {
        img.clone()
    } else {
        LogoImage { path: name.to_string(), alt: None }
    }
}

fn parse_typography(root: &saphyr::MappingOwned) -> BrandTypography {
    let empty_map = saphyr::MappingOwned::new();
    let typo_val = yaml_get(root, "typography");
    let typo_map = typo_val.and_then(|v| v.as_mapping()).unwrap_or(&empty_map);

    let mut fonts = Vec::new();
    if let Some(seq) = yaml_get(typo_map, "fonts").and_then(|v| v.as_sequence()) {
        for item in seq {
            if let Some(map) = item.as_mapping() {
                let family = yaml_str(map, "family").unwrap_or_default().to_string();
                let source = yaml_str(map, "source").unwrap_or("system").to_string();
                if !family.is_empty() {
                    fonts.push(FontDef { source, family });
                }
            }
        }
    }

    fn get_family(map: &saphyr::MappingOwned, key: &str) -> Option<String> {
        let val = yaml_get(map, key)?;
        if let Some(s) = val.as_str() {
            return Some(s.to_string());
        }
        if let Some(m) = val.as_mapping() {
            return yaml_str(m, "family").map(String::from);
        }
        None
    }

    BrandTypography {
        fonts,
        base: get_family(typo_map, "base"),
        headings: get_family(typo_map, "headings"),
        monospace: get_family(typo_map, "monospace"),
    }
}

fn parse_meta(root: &saphyr::MappingOwned) -> HashMap<String, String> {
    let mut meta = HashMap::new();
    if let Some(map) = yaml_get(root, "meta").and_then(|v| v.as_mapping()) {
        for (k, v) in map {
            if let (Some(key), Some(val)) = (k.as_str(), v.as_str()) {
                meta.insert(key.to_string(), val.to_string());
            }
        }
    }
    meta
}

// ---------------------------------------------------------------------------
// Public accessors (for shortcodes)
// ---------------------------------------------------------------------------

/// Get a brand color by semantic name, optionally for a specific mode.
pub fn brand_color(name: &str, mode: Option<&str>) -> Option<String> {
    let brand = brand()?;
    let cv = brand.color.semantic.get(name)?;
    match cv {
        ColorValue::Flat(hex) => Some(hex.clone()),
        ColorValue::Themed { light, dark } => {
            match mode.unwrap_or("light") {
                "dark" => dark.clone(),
                _ => light.clone().or_else(|| dark.clone()),
            }
        }
    }
}

/// Get the preferred logo for a given size and mode.
/// Size preference: medium > small > large.
pub fn resolve_preferred_logo(mode: &str) -> Option<LogoImage> {
    let brand = brand()?;
    let logo = &brand.logo;
    for slot in [&logo.medium, &logo.small, &logo.large].into_iter().flatten() {
        match slot {
            LogoSlot::Single(img) => return Some(img.clone()),
            LogoSlot::Themed { light, dark } => {
                return match mode {
                    "dark" => dark.clone().or_else(|| light.clone()),
                    _ => light.clone().or_else(|| dark.clone()),
                };
            }
        }
    }
    None
}

/// Generate an `<img>` tag (or pair for light/dark) for a brand logo.
pub fn brand_logo_tag(size: &str, mode: &str, format: &str) -> Option<String> {
    let brand = brand()?;
    let logo = &brand.logo;

    let slot = match size {
        "small" => logo.small.as_ref(),
        "large" => logo.large.as_ref(),
        _ => logo.medium.as_ref().or(logo.small.as_ref()).or(logo.large.as_ref()),
    }?;

    match format {
        "html" => Some(logo_slot_to_html(slot, mode)),
        _ => {
            // For non-HTML, just return the path
            let img = match slot {
                LogoSlot::Single(img) => img,
                LogoSlot::Themed { light, dark } => {
                    match mode {
                        "dark" => dark.as_ref().or(light.as_ref())?,
                        _ => light.as_ref().or(dark.as_ref())?,
                    }
                }
            };
            Some(img.path.clone())
        }
    }
}

fn logo_slot_to_html(slot: &LogoSlot, mode: &str) -> String {
    match slot {
        LogoSlot::Single(img) => {
            let alt = img.alt.as_deref().unwrap_or("");
            format!(r#"<img src="{}" alt="{}">"#, img.path, alt)
        }
        LogoSlot::Themed { light, dark } => {
            if mode == "both" {
                let mut parts = Vec::new();
                if let Some(l) = light {
                    let alt = l.alt.as_deref().unwrap_or("");
                    parts.push(format!(
                        r#"<img src="{}" alt="{}" class="brand-logo-light">"#,
                        l.path, alt,
                    ));
                }
                if let Some(d) = dark {
                    let alt = d.alt.as_deref().unwrap_or("");
                    parts.push(format!(
                        r#"<img src="{}" alt="{}" class="brand-logo-dark">"#,
                        d.path, alt,
                    ));
                }
                parts.join("")
            } else {
                let img = match mode {
                    "dark" => dark.as_ref().or(light.as_ref()),
                    _ => light.as_ref().or(dark.as_ref()),
                };
                match img {
                    Some(i) => {
                        let alt = i.alt.as_deref().unwrap_or("");
                        format!(r#"<img src="{}" alt="{}">"#, i.path, alt)
                    }
                    None => String::new(),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CSS generation
// ---------------------------------------------------------------------------

/// Generate brand CSS with `@media (prefers-color-scheme)` scoping.
pub fn build_brand_css(brand: &Brand) -> String {
    build_brand_css_inner(brand, CssScope::MediaQuery)
}

/// Generate brand CSS with `[data-theme]` scoping.
pub fn build_brand_css_datatheme(brand: &Brand) -> String {
    build_brand_css_inner(brand, CssScope::DataTheme)
}

enum CssScope {
    MediaQuery,
    DataTheme,
}

fn build_brand_css_inner(brand: &Brand, scope: CssScope) -> String {
    let mut parts = Vec::new();

    // Google font imports
    for font in &brand.typography.fonts {
        if font.source == "google" {
            let encoded = font.family.replace(' ', "+");
            parts.push(format!(
                "@import url('https://fonts.googleapis.com/css2?family={}:ital,wght@0,300..900;1,300..900&display=swap');",
                encoded,
            ));
        }
    }

    // Collect flat vs themed colors
    let mut flat_props = Vec::new();
    let mut light_props = Vec::new();
    let mut dark_props = Vec::new();

    for (name, cv) in &brand.color.semantic {
        match cv {
            ColorValue::Flat(hex) => {
                flat_props.push(format!("  --brand-{}: {};", name, hex));
            }
            ColorValue::Themed { light, dark } => {
                if let Some(l) = light {
                    light_props.push(format!("  --brand-{}: {};", name, l));
                }
                if let Some(d) = dark {
                    dark_props.push(format!("  --brand-{}: {};", name, d));
                }
            }
        }
    }

    // Flat colors in :root
    if !flat_props.is_empty() {
        parts.push(":root {".to_string());
        parts.extend(flat_props);
        parts.push("}".to_string());
    }

    // Themed colors
    if !light_props.is_empty() {
        let selector = match scope {
            CssScope::MediaQuery => "@media (prefers-color-scheme: light) {\n:root {".to_string(),
            CssScope::DataTheme => "[data-theme='light'], :root:not([data-theme='dark']) {".to_string(),
        };
        parts.push(selector);
        parts.extend(light_props);
        match scope {
            CssScope::MediaQuery => parts.push("}\n}".to_string()),
            CssScope::DataTheme => parts.push("}".to_string()),
        }
    }

    if !dark_props.is_empty() {
        let selector = match scope {
            CssScope::MediaQuery => "@media (prefers-color-scheme: dark) {\n:root {".to_string(),
            CssScope::DataTheme => "[data-theme='dark'] {".to_string(),
        };
        parts.push(selector);
        parts.extend(dark_props);
        match scope {
            CssScope::MediaQuery => parts.push("}\n}".to_string()),
            CssScope::DataTheme => parts.push("}".to_string()),
        }
    }

    // Font families
    let mut font_props = Vec::new();
    if let Some(ref f) = brand.typography.base {
        font_props.push(format!("  --brand-font-base: '{}', sans-serif;", f));
    }
    if let Some(ref f) = brand.typography.headings {
        font_props.push(format!("  --brand-font-headings: '{}', sans-serif;", f));
    }
    if let Some(ref f) = brand.typography.monospace {
        font_props.push(format!("  --brand-font-monospace: '{}', monospace;", f));
    }
    if !font_props.is_empty() {
        parts.push(":root {".to_string());
        parts.extend(font_props);
        parts.push("}".to_string());
        // Apply fonts to elements
        if brand.typography.base.is_some() {
            parts.push("body { font-family: var(--brand-font-base); }".to_string());
        }
        if brand.typography.headings.is_some() {
            parts.push("h1, h2, h3, h4, h5, h6 { font-family: var(--brand-font-headings); }".to_string());
        }
        if brand.typography.monospace.is_some() {
            parts.push("code, pre { font-family: var(--brand-font-monospace); }".to_string());
        }
    }

    parts.join("\n")
}

// ---------------------------------------------------------------------------
// Template variable injection
// ---------------------------------------------------------------------------

/// Inject brand-related template variables into a vars map.
pub fn inject_brand_vars(vars: &mut HashMap<String, String>, ext: &str) {
    let brand = match brand() {
        Some(b) => b,
        None => return,
    };

    // Semantic colors
    for (name, cv) in &brand.color.semantic {
        match cv {
            ColorValue::Flat(hex) => {
                vars.insert(format!("brand-{}", name), hex.clone());
            }
            ColorValue::Themed { light, dark } => {
                if let Some(l) = light {
                    vars.insert(format!("brand-{}-light", name), l.clone());
                }
                if let Some(d) = dark {
                    vars.insert(format!("brand-{}-dark", name), d.clone());
                }
                // Default: light fallback
                let default = light.as_ref().or(dark.as_ref());
                if let Some(d) = default {
                    vars.insert(format!("brand-{}", name), d.clone());
                }
            }
        }
    }

    // Logo paths (preferred size: medium > small > large)
    if let Some(img) = resolve_preferred_logo("light") {
        vars.insert("brand_logo_light".to_string(), img.path);
        if let Some(alt) = img.alt {
            vars.insert("brand_logo_alt".to_string(), alt);
        }
    }
    if let Some(img) = resolve_preferred_logo("dark") {
        vars.insert("brand_logo_dark".to_string(), img.path);
    }

    // Meta
    for (k, v) in &brand.meta {
        vars.insert(format!("brand-meta-{}", k), v.clone());
    }

    // CSS (HTML only)
    if ext == "html" {
        let brand_css = build_brand_css(brand);
        if !brand_css.is_empty() {
            vars.insert("brand_css".to_string(), brand_css.clone());
            let css = vars.entry("css".to_string()).or_default();
            css.push_str(&format!("\n<style>\n{}\n</style>", brand_css));
        }
        let brand_css_dt = build_brand_css_datatheme(brand);
        if !brand_css_dt.is_empty() {
            vars.insert("brand_css_datatheme".to_string(), brand_css_dt);
        }
    }
}
