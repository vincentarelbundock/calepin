use std::fmt::Write;
use std::path::PathBuf;

use syntect::highlighting::ThemeSet;
use syntect::html::{ClassStyle, css_for_theme_with_class_style};

fn main() {
    let out_dir: PathBuf = std::env::var("OUT_DIR").unwrap().into();
    let generated_path = out_dir.join("theme_css.rs");

    // Only regenerate if themes have changed
    let themes_dir = PathBuf::from("src/modules/highlight/themes");
    println!("cargo:rerun-if-changed={}", themes_dir.display());

    let mut code = String::from(
        "/// Pre-generated CSS for built-in syntax highlighting themes.\n\
         /// Generated at build time from .tmTheme files.\n\
         pub fn builtin_theme_css(name: &str) -> Option<(&'static str, &'static str, &'static str)> {\n\
         \x20   match name {\n",
    );

    for entry in std::fs::read_dir(&themes_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("tmTheme") {
            continue;
        }
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();

        let theme = match ThemeSet::get_theme(&path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("cargo:warning=failed to parse {}: {}", path.display(), e);
                continue;
            }
        };

        let css = css_for_theme_with_class_style(&theme, ClassStyle::Spaced).unwrap_or_default();

        let bg = theme
            .settings
            .background
            .unwrap_or(syntect::highlighting::Color::WHITE);
        let fg = theme
            .settings
            .foreground
            .unwrap_or(syntect::highlighting::Color::BLACK);
        let bg_hex = format!("#{:02x}{:02x}{:02x}", bg.r, bg.g, bg.b);
        let fg_hex = format!("#{:02x}{:02x}{:02x}", fg.r, fg.g, fg.b);

        write!(
            code,
            "        {:?} => Some(({css}, {bg}, {fg})),\n",
            name,
            css = format_args!("r##\"{}\"##", css),
            bg = format_args!("{:?}", bg_hex),
            fg = format_args!("{:?}", fg_hex),
        )
        .unwrap();
    }

    // Also handle syntect built-in themes
    let defaults = ThemeSet::load_defaults();
    for (name, theme) in &defaults.themes {
        let css = css_for_theme_with_class_style(theme, ClassStyle::Spaced).unwrap_or_default();
        let bg = theme
            .settings
            .background
            .unwrap_or(syntect::highlighting::Color::WHITE);
        let fg = theme
            .settings
            .foreground
            .unwrap_or(syntect::highlighting::Color::BLACK);
        let bg_hex = format!("#{:02x}{:02x}{:02x}", bg.r, bg.g, bg.b);
        let fg_hex = format!("#{:02x}{:02x}{:02x}", fg.r, fg.g, fg.b);

        write!(
            code,
            "        {:?} => Some(({css}, {bg}, {fg})),\n",
            name,
            css = format_args!("r##\"{}\"##", css),
            bg = format_args!("{:?}", bg_hex),
            fg = format_args!("{:?}", fg_hex),
        )
        .unwrap();
    }

    code.push_str("        _ => None,\n    }\n}\n");

    std::fs::write(&generated_path, code).unwrap();
}
