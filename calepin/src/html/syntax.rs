use anyhow::{anyhow, Context, Result};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub(crate) struct HtmlSyntaxTheme {
    foreground_light: String,
    foreground_dark: String,
    background_light: String,
    background_dark: String,
    tokens: Vec<HtmlSyntaxToken>,
}

#[derive(Debug, Clone)]
struct HtmlSyntaxToken {
    emitted_color: String,
    class_name: String,
    variable_name: String,
    light_color: String,
    dark_color: String,
}

#[derive(Debug, Clone)]
struct TmThemeEntry {
    scope: Option<String>,
    foreground: String,
    background: Option<String>,
}

impl HtmlSyntaxTheme {
    pub(super) fn builtin() -> Self {
        Self {
            foreground_light: "#003b4f".to_string(),
            foreground_dark: "#d8e7ef".to_string(),
            background_light: "#f7f7f5".to_string(),
            background_dark: "#161b22".to_string(),
            tokens: vec![
                HtmlSyntaxToken::new(
                    "#003b4f",
                    "calepin-syntax-foreground",
                    "calepin-syntax-foreground",
                    "#003b4f",
                    "#d8e7ef",
                ),
                HtmlSyntaxToken::new(
                    "#4759ab",
                    "calepin-syntax-function",
                    "calepin-syntax-function",
                    "#4759ab",
                    "#9db8ff",
                ),
                HtmlSyntaxToken::new(
                    "#ad0000",
                    "calepin-syntax-number",
                    "calepin-syntax-number",
                    "#ad0000",
                    "#ffb3a7",
                ),
                HtmlSyntaxToken::new(
                    "#5e5e5e",
                    "calepin-syntax-operator",
                    "calepin-syntax-operator",
                    "#5e5e5e",
                    "#b5bdc9",
                ),
                HtmlSyntaxToken::new(
                    "#667321",
                    "calepin-syntax-parameter",
                    "calepin-syntax-parameter",
                    "#667321",
                    "#c3d86c",
                ),
            ],
        }
    }

    pub(super) fn from_tmtheme_sources(light: &str, dark: &str) -> Result<Self> {
        let light_entries =
            parse_tmtheme_entries(light).context("failed to parse light tmTheme")?;
        let dark_entries = parse_tmtheme_entries(dark).context("failed to parse dark tmTheme")?;
        if light_entries.is_empty() {
            return Err(anyhow!("light tmTheme contains no foreground colors"));
        }
        if dark_entries.is_empty() {
            return Err(anyhow!("dark tmTheme contains no foreground colors"));
        }

        let foreground_light = default_tmtheme_foreground(&light_entries);
        let foreground_dark = default_tmtheme_foreground(&dark_entries);
        let background_light =
            default_tmtheme_background(&light_entries).unwrap_or_else(|| "#f7f7f5".to_string());
        let background_dark =
            default_tmtheme_background(&dark_entries).unwrap_or_else(|| "#161b22".to_string());
        let dark_by_scope: BTreeMap<_, _> = dark_entries
            .iter()
            .filter_map(|entry| {
                entry
                    .scope
                    .as_ref()
                    .map(|scope| (scope.clone(), entry.foreground.clone()))
            })
            .collect();

        let mut seen = BTreeSet::new();
        let mut tokens = Vec::new();
        for (index, entry) in light_entries.iter().enumerate() {
            if !seen.insert(entry.foreground.clone()) {
                continue;
            }
            let dark_color = entry
                .scope
                .as_ref()
                .and_then(|scope| dark_by_scope.get(scope))
                .cloned()
                .or_else(|| {
                    dark_entries
                        .get(index)
                        .map(|entry| entry.foreground.clone())
                })
                .unwrap_or_else(|| foreground_dark.clone());
            let class_name = format!("calepin-syntax-token-{}", tokens.len());
            tokens.push(HtmlSyntaxToken::new(
                &entry.foreground,
                &class_name,
                &class_name,
                &entry.foreground,
                &dark_color,
            ));
        }

        Ok(Self {
            foreground_light,
            foreground_dark,
            background_light,
            background_dark,
            tokens,
        })
    }

    pub(super) fn declarations(&self, dark: bool) -> String {
        let mut declarations = String::new();
        declarations.push_str("  --calepin-syntax-foreground: ");
        declarations.push_str(if dark {
            &self.foreground_dark
        } else {
            &self.foreground_light
        });
        declarations.push_str(";\n");
        declarations.push_str("  --calepin-syntax-background: ");
        declarations.push_str(if dark {
            &self.background_dark
        } else {
            &self.background_light
        });
        declarations.push_str(";\n");
        declarations.push_str("  --calepin-syntax-border: color-mix(in srgb, var(--calepin-syntax-foreground) 18%, var(--calepin-syntax-background));\n");

        let mut declared = BTreeSet::from(["calepin-syntax-foreground".to_string()]);
        for token in &self.tokens {
            if !declared.insert(token.variable_name.clone()) {
                continue;
            }
            declarations.push_str("  --");
            declarations.push_str(&token.variable_name);
            declarations.push_str(": ");
            declarations.push_str(if dark {
                &token.dark_color
            } else {
                &token.light_color
            });
            declarations.push_str(";\n");
        }
        declarations
    }

    pub(super) fn class_rules(&self) -> String {
        let mut rules = String::new();
        for token in &self.tokens {
            rules.push_str(".sourceCode .");
            rules.push_str(&token.class_name);
            rules.push_str(" {\n  color: var(--");
            rules.push_str(&token.variable_name);
            rules.push_str(");\n}\n\n");
        }
        rules
    }

    pub(super) fn rewrite_classes(&self, html: &str) -> String {
        let mut rewritten = String::with_capacity(html.len());
        let mut remaining = html;

        while let Some(block_start) = remaining.find("<div class=\"sourceCode\"") {
            rewritten.push_str(&remaining[..block_start]);
            let block_and_after = &remaining[block_start..];
            let Some(block_end) = block_and_after.find("</div>") else {
                rewritten.push_str(block_and_after);
                return rewritten;
            };
            let block_end = block_end + "</div>".len();
            rewritten.push_str(&self.rewrite_color_attrs(&block_and_after[..block_end]));
            remaining = &block_and_after[block_end..];
        }

        rewritten.push_str(remaining);
        rewritten
    }

    fn rewrite_color_attrs(&self, html: &str) -> String {
        let mut rewritten = html.to_string();
        for token in &self.tokens {
            let class_attr = format!("class=\"{}\"", token.class_name);
            for color in html_color_variants(&token.emitted_color) {
                rewritten = rewritten.replace(&format!("style=\"color: {color}\""), &class_attr);
                rewritten = rewritten.replace(&format!("style=\"color:{color}\""), &class_attr);
            }
        }
        rewritten
    }
}

impl HtmlSyntaxToken {
    fn new(
        emitted_color: &str,
        class_name: &str,
        variable_name: &str,
        light_color: &str,
        dark_color: &str,
    ) -> Self {
        Self {
            emitted_color: emitted_color.to_string(),
            class_name: class_name.to_string(),
            variable_name: variable_name.to_string(),
            light_color: light_color.to_string(),
            dark_color: dark_color.to_string(),
        }
    }
}

fn default_tmtheme_foreground(entries: &[TmThemeEntry]) -> String {
    entries
        .iter()
        .find(|entry| entry.scope.is_none())
        .or_else(|| entries.first())
        .map(|entry| entry.foreground.clone())
        .unwrap_or_else(|| "#003b4f".to_string())
}

fn default_tmtheme_background(entries: &[TmThemeEntry]) -> Option<String> {
    entries
        .iter()
        .find(|entry| entry.scope.is_none())
        .and_then(|entry| entry.background.clone())
        .or_else(|| entries.iter().find_map(|entry| entry.background.clone()))
}

fn parse_tmtheme_entries(source: &str) -> Result<Vec<TmThemeEntry>> {
    let array = tmtheme_settings_array(source)?;
    let dicts = top_level_plist_dicts(array);
    let mut entries = Vec::new();

    for dict in dicts {
        let Some(foreground) = plist_string_after_key(dict, "foreground")
            .and_then(|color| normalize_hex_color(&color))
        else {
            continue;
        };
        let background = plist_string_after_key(dict, "background")
            .and_then(|color| normalize_hex_color(&color));
        let scope = plist_string_after_key(dict, "scope").map(|scope| scope.trim().to_string());
        entries.push(TmThemeEntry {
            scope,
            foreground,
            background,
        });
    }

    Ok(entries)
}

fn tmtheme_settings_array(source: &str) -> Result<&str> {
    let settings_key = source
        .find("<key>settings</key>")
        .ok_or_else(|| anyhow!("tmTheme is missing settings array"))?;
    let after_settings = &source[settings_key..];
    let array_open = after_settings
        .find("<array>")
        .ok_or_else(|| anyhow!("tmTheme settings are missing an array"))?
        + "<array>".len();
    let after_array_open = &after_settings[array_open..];
    let array_close = after_array_open
        .find("</array>")
        .ok_or_else(|| anyhow!("tmTheme settings array is unterminated"))?;
    Ok(&after_array_open[..array_close])
}

fn top_level_plist_dicts(array: &str) -> Vec<&str> {
    let mut dicts = Vec::new();
    let mut search = 0;

    while let Some(relative_start) = array[search..].find("<dict>") {
        let start = search + relative_start;
        let mut position = start;
        let mut depth = 0usize;

        loop {
            let next_open = array[position..]
                .find("<dict>")
                .map(|offset| position + offset);
            let next_close = array[position..]
                .find("</dict>")
                .map(|offset| position + offset);

            match (next_open, next_close) {
                (Some(open), Some(close)) if open < close => {
                    depth += 1;
                    position = open + "<dict>".len();
                }
                (_, Some(close)) => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    position = close + "</dict>".len();
                    if depth == 0 {
                        dicts.push(&array[start..position]);
                        search = position;
                        break;
                    }
                }
                _ => {
                    search = array.len();
                    break;
                }
            }
        }
    }

    dicts
}

fn plist_string_after_key(fragment: &str, key: &str) -> Option<String> {
    let needle = format!("<key>{key}</key>");
    let key_start = fragment.find(&needle)?;
    let after_key = &fragment[key_start + needle.len()..];
    let string_start = after_key.find("<string>")? + "<string>".len();
    let after_string_start = &after_key[string_start..];
    let string_end = after_string_start.find("</string>")?;
    Some(xml_unescape(&after_string_start[..string_end]))
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn normalize_hex_color(value: &str) -> Option<String> {
    let value = value.trim();
    let value = value.strip_prefix('#').unwrap_or(value);
    if (value.len() == 6 || value.len() == 8) && value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Some(format!("#{}", value.to_ascii_lowercase()))
    } else {
        None
    }
}

fn html_color_variants(color: &str) -> Vec<String> {
    let lower = color.to_ascii_lowercase();
    let upper = color.to_ascii_uppercase();
    if lower == upper {
        vec![lower]
    } else {
        vec![lower, upper]
    }
}
