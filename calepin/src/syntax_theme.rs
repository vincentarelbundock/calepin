use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use plist::Value;

pub(crate) const DEFAULT_LIGHT_FOREGROUND: &str = "#003b4f";
pub(crate) const DEFAULT_DARK_FOREGROUND: &str = "#d8e7ef";
pub(crate) const DEFAULT_LIGHT_BACKGROUND: &str = "#f7f7f5";
pub(crate) const DEFAULT_DARK_BACKGROUND: &str = "#161b22";

#[derive(Debug, Clone)]
pub(crate) struct TextMateTheme {
    pub(crate) foreground: String,
    pub(crate) background: String,
    pub(crate) rules: Vec<TextMateRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextMateRule {
    pub(crate) name: Option<String>,
    pub(crate) scope: Option<String>,
    pub(crate) foreground: Option<String>,
    pub(crate) background: Option<String>,
    pub(crate) font_style: Option<String>,
}

impl TextMateTheme {
    pub(crate) fn from_file(path: &Path) -> Result<Self> {
        let value = Value::from_file(path)
            .with_context(|| format!("failed to read TextMate theme {}", path.display()))?;
        Self::from_plist(value).with_context(|| {
            format!(
                "failed to parse TextMate theme {}: expected a .tmTheme plist",
                path.display()
            )
        })
    }

    pub(crate) fn default_light() -> Self {
        Self {
            foreground: DEFAULT_LIGHT_FOREGROUND.to_string(),
            background: DEFAULT_LIGHT_BACKGROUND.to_string(),
            rules: default_rules(false),
        }
    }

    pub(crate) fn default_dark() -> Self {
        Self {
            foreground: DEFAULT_DARK_FOREGROUND.to_string(),
            background: DEFAULT_DARK_BACKGROUND.to_string(),
            rules: default_rules(true),
        }
    }

    fn from_plist(value: Value) -> Result<Self> {
        let Value::Dictionary(root) = value else {
            bail!("theme root is not a dictionary");
        };
        let Some(Value::Array(settings)) = root.get("settings") else {
            bail!("theme does not contain a settings array");
        };

        let mut foreground = None;
        let mut background = None;
        let mut rules = Vec::new();

        for item in settings {
            let Value::Dictionary(rule) = item else {
                continue;
            };
            let name = string_value(rule, "name");
            let scope = string_value(rule, "scope").map(normalize_scope);
            let Some(Value::Dictionary(style)) = rule.get("settings") else {
                continue;
            };
            let rule_foreground = string_value(style, "foreground").map(normalize_color);
            let rule_background = string_value(style, "background").map(normalize_color);
            let font_style = string_value(style, "fontStyle");

            if scope.is_none() {
                foreground = rule_foreground.or(foreground);
                background = rule_background.or(background);
                continue;
            }

            rules.push(TextMateRule {
                name,
                scope,
                foreground: rule_foreground,
                background: rule_background,
                font_style,
            });
        }

        Ok(Self {
            foreground: foreground.unwrap_or_else(|| DEFAULT_LIGHT_FOREGROUND.to_string()),
            background: background.unwrap_or_else(|| DEFAULT_LIGHT_BACKGROUND.to_string()),
            rules,
        })
    }
}

pub(crate) fn textmate_theme_source(
    name: &str,
    foreground: &str,
    background: Option<&str>,
    rules: &[TextMateRule],
) -> String {
    let mut lines = vec![
        r#"<?xml version="1.0" encoding="UTF-8"?>"#.to_string(),
        r#"<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">"#.to_string(),
        r#"<plist version="1.0">"#.to_string(),
        "<dict>".to_string(),
        "  <key>name</key>".to_string(),
        format!("  <string>{}</string>", xml_escape(name)),
        "  <key>settings</key>".to_string(),
        "  <array>".to_string(),
        "    <dict>".to_string(),
        "      <key>settings</key>".to_string(),
        "      <dict>".to_string(),
        "        <key>foreground</key>".to_string(),
        format!("        <string>{}</string>", xml_escape(foreground)),
    ];
    if let Some(background) = background {
        lines.extend([
            "        <key>background</key>".to_string(),
            format!("        <string>{}</string>", xml_escape(background)),
        ]);
    }
    lines.extend(["      </dict>".to_string(), "    </dict>".to_string()]);

    for rule in rules {
        let Some(scope) = rule.scope.as_deref() else {
            continue;
        };
        lines.extend([
            "    <dict>".to_string(),
            "      <key>name</key>".to_string(),
            format!(
                "      <string>{}</string>",
                xml_escape(rule.name.as_deref().unwrap_or(scope))
            ),
            "      <key>scope</key>".to_string(),
            format!("      <string>{}</string>", xml_escape(scope)),
            "      <key>settings</key>".to_string(),
            "      <dict>".to_string(),
        ]);
        if let Some(foreground) = rule.foreground.as_deref() {
            lines.extend([
                "        <key>foreground</key>".to_string(),
                format!("        <string>{}</string>", xml_escape(foreground)),
            ]);
        }
        if let Some(background) = rule.background.as_deref() {
            lines.extend([
                "        <key>background</key>".to_string(),
                format!("        <string>{}</string>", xml_escape(background)),
            ]);
        }
        if let Some(font_style) = rule.font_style.as_deref() {
            lines.extend([
                "        <key>fontStyle</key>".to_string(),
                format!("        <string>{}</string>", xml_escape(font_style)),
            ]);
        }
        lines.extend(["      </dict>".to_string(), "    </dict>".to_string()]);
    }

    lines.extend([
        "  </array>".to_string(),
        "</dict>".to_string(),
        "</plist>".to_string(),
    ]);
    lines.join("\n")
}

pub(crate) fn typst_runtime_source(html_theme_source: &str, paged_theme_source: &str) -> String {
    let mut source = String::new();
    source.push_str("#let _input-syntax-theme = bytes((");
    push_typst_string_lines(&mut source, html_theme_source.lines());
    source.push_str(").join(\"\\n\"))\n\n");
    source.push_str("#let _output-syntax-theme = _input-syntax-theme\n\n");
    source.push_str("#let _paged-syntax-theme = bytes((");
    push_typst_string_lines(&mut source, paged_theme_source.lines());
    source.push_str(").join(\"\\n\"))\n\n");
    source
}

fn default_rules(dark: bool) -> Vec<TextMateRule> {
    let colors: BTreeMap<&str, &str> = if dark {
        BTreeMap::from([
            ("function", "#9db8ff"),
            ("number", "#ffb3a7"),
            ("operator", "#b5bdc9"),
            ("parameter", "#c3d86c"),
        ])
    } else {
        BTreeMap::from([
            ("function", "#4759ab"),
            ("number", "#ad0000"),
            ("operator", "#5e5e5e"),
            ("parameter", "#667321"),
        ])
    };
    vec![
        TextMateRule {
            name: Some("Function calls".to_string()),
            scope: Some("entity.name.function, support.function, variable.function".to_string()),
            foreground: Some(colors["function"].to_string()),
            background: None,
            font_style: None,
        },
        TextMateRule {
            name: Some("Numeric literals".to_string()),
            scope: Some("constant.numeric".to_string()),
            foreground: Some(colors["number"].to_string()),
            background: None,
            font_style: None,
        },
        TextMateRule {
            name: Some("Operators and special characters".to_string()),
            scope: Some(
                "keyword.operator, punctuation.definition, punctuation.separator".to_string(),
            ),
            foreground: Some(colors["operator"].to_string()),
            background: None,
            font_style: None,
        },
        TextMateRule {
            name: Some("Assignments".to_string()),
            scope: Some("keyword.operator.assignment".to_string()),
            foreground: None,
            background: None,
            font_style: None,
        },
        TextMateRule {
            name: Some("Named arguments".to_string()),
            scope: Some(
                "variable.parameter, entity.other.attribute-name, support.variable.parameter"
                    .to_string(),
            ),
            foreground: Some(colors["parameter"].to_string()),
            background: None,
            font_style: None,
        },
        TextMateRule {
            name: Some("Output emphasis".to_string()),
            scope: Some("markup.strong".to_string()),
            foreground: None,
            background: None,
            font_style: Some("bold".to_string()),
        },
    ]
}

fn string_value(dict: &plist::Dictionary, key: &str) -> Option<String> {
    dict.get(key)
        .and_then(|value| match value {
            Value::String(value) => Some(value.trim().to_string()),
            _ => None,
        })
        .filter(|value| !value.is_empty())
}

fn normalize_scope(scope: String) -> String {
    scope
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

fn normalize_color(color: String) -> String {
    let color = color.trim();
    if let Some(hex) = color.strip_prefix('#') {
        format!("#{}", hex.to_ascii_lowercase())
    } else {
        color.to_string()
    }
}

fn push_typst_string_lines<'a>(source: &mut String, lines: impl IntoIterator<Item = &'a str>) {
    for line in lines {
        source.push_str("\n  \"");
        for ch in line.chars() {
            match ch {
                '\\' => source.push_str("\\\\"),
                '"' => source.push_str("\\\""),
                _ => source.push(ch),
            }
        }
        source.push_str("\",");
    }
    source.push('\n');
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
