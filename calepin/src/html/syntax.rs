use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;

use crate::syntax_theme::{self, TextMateRule, TextMateTheme};
use crate::utils::path::absolutize_from;

#[derive(Debug, Clone)]
pub(crate) struct HtmlSyntaxTheme {
    foreground_light: String,
    foreground_dark: String,
    background_light: String,
    background_dark: String,
    tokens: Vec<HtmlSyntaxToken>,
    runtime_theme_source: String,
    paged_theme_source: String,
}

#[derive(Debug, Clone)]
struct HtmlSyntaxToken {
    emitted_color: String,
    class_name: String,
    variable_name: String,
    light: SyntaxStyle,
    dark: SyntaxStyle,
}

#[derive(Debug, Clone, Default)]
struct SyntaxStyle {
    foreground: Option<String>,
    background: Option<String>,
    font_style: Option<String>,
}

impl HtmlSyntaxTheme {
    pub(crate) fn builtin() -> Self {
        Self::from_textmate_themes(
            TextMateTheme::default_light(),
            TextMateTheme::default_dark(),
        )
    }

    pub(crate) fn from_paths(
        config_dir: &Path,
        light_path: Option<&Path>,
        dark_path: Option<&Path>,
    ) -> Result<Self> {
        let light = match light_path {
            Some(path) => TextMateTheme::from_file(&resolve_config_path(config_dir, path))?,
            None => TextMateTheme::default_light(),
        };
        let dark = match dark_path {
            Some(path) => TextMateTheme::from_file(&resolve_config_path(config_dir, path))?,
            None => TextMateTheme::default_dark(),
        };
        Ok(Self::from_textmate_themes(light, dark))
    }

    pub(crate) fn typst_runtime_source(&self) -> String {
        syntax_theme::typst_runtime_source(&self.runtime_theme_source, &self.paged_theme_source)
    }

    pub(crate) fn declarations(&self, dark: bool) -> String {
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
            let style = if dark { &token.dark } else { &token.light };
            if let Some(foreground) = style.foreground.as_deref() {
                push_declaration(
                    &mut declarations,
                    &token.variable_name,
                    "foreground",
                    foreground,
                );
            }
            if let Some(background) = style.background.as_deref() {
                push_declaration(
                    &mut declarations,
                    &token.variable_name,
                    "background",
                    background,
                );
            }
            if let Some(font_style) = style.font_style.as_deref() {
                for (property, value) in css_font_style(font_style) {
                    push_declaration(&mut declarations, &token.variable_name, property, value);
                }
            }
        }
        declarations
    }

    pub(crate) fn class_rules(&self) -> String {
        let mut rules = String::new();
        for token in &self.tokens {
            rules.push_str(".sourceCode .");
            rules.push_str(&token.class_name);
            rules.push_str(",\npre code .");
            rules.push_str(&token.class_name);
            rules.push_str(" {\n");
            if token.light.foreground.is_some() || token.dark.foreground.is_some() {
                rules.push_str("  color: var(--");
                rules.push_str(&token.variable_name);
                rules.push_str("-foreground);\n");
            }
            if token.light.background.is_some() || token.dark.background.is_some() {
                rules.push_str("  background-color: var(--");
                rules.push_str(&token.variable_name);
                rules.push_str("-background);\n");
            }
            if token.has_font_style("font-weight") {
                rules.push_str("  font-weight: var(--");
                rules.push_str(&token.variable_name);
                rules.push_str("-font-weight);\n");
            }
            if token.has_font_style("font-style") {
                rules.push_str("  font-style: var(--");
                rules.push_str(&token.variable_name);
                rules.push_str("-font-style);\n");
            }
            if token.has_font_style("text-decoration") {
                rules.push_str("  text-decoration: var(--");
                rules.push_str(&token.variable_name);
                rules.push_str("-text-decoration);\n");
            }
            rules.push_str("}\n\n");
        }
        rules
    }

    fn from_textmate_themes(light: TextMateTheme, dark: TextMateTheme) -> Self {
        let specs = union_rule_specs(&light, &dark);
        let mut tokens = specs
            .iter()
            .enumerate()
            .map(|(index, spec)| {
                let emitted_color = sentinel_color(index);
                let variable_name = format!("scope-{}", index + 1);
                HtmlSyntaxToken {
                    emitted_color: emitted_color.clone(),
                    class_name: format!("calepin-syntax-{variable_name}"),
                    variable_name,
                    light: style_for_scope(&light, spec.scope.as_deref(), &light.foreground),
                    dark: style_for_scope(&dark, spec.scope.as_deref(), &dark.foreground),
                }
            })
            .collect::<Vec<_>>();
        tokens.extend(fallback_syntax_tokens(&light, &dark, specs.len() + 1));
        let sentinel_rules = specs
            .into_iter()
            .enumerate()
            .map(|(index, mut rule)| {
                rule.foreground = Some(sentinel_color(index));
                rule.background = None;
                rule.font_style = None;
                rule
            })
            .collect::<Vec<_>>();
        let runtime_theme_source = syntax_theme::textmate_theme_source(
            "Calepin HTML Syntax Sentinel",
            "#000000",
            None,
            &sentinel_rules,
        );
        let paged_theme_source = syntax_theme::textmate_theme_source(
            "Calepin Paged Syntax",
            &light.foreground,
            Some(&light.background),
            &light.rules,
        );

        Self {
            foreground_light: light.foreground,
            foreground_dark: dark.foreground,
            background_light: light.background,
            background_dark: dark.background,
            tokens,
            runtime_theme_source,
            paged_theme_source,
        }
    }

    pub(super) fn rewrite_classes(&self, html: &str) -> String {
        self.rewrite_matching_blocks(html, "<pre", "</pre>")
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

    fn rewrite_matching_blocks(&self, html: &str, open: &str, close: &str) -> String {
        let mut rewritten = String::with_capacity(html.len());
        let mut remaining = html;

        while let Some(block_start) = remaining.find(open) {
            rewritten.push_str(&remaining[..block_start]);
            let block_and_after = &remaining[block_start..];
            let Some(block_end) = block_and_after.find(close) else {
                rewritten.push_str(block_and_after);
                return rewritten;
            };
            let block_end = block_end + close.len();
            let block = &block_and_after[..block_end];
            if block.contains("<code") {
                rewritten.push_str(&self.rewrite_color_attrs(block));
            } else {
                rewritten.push_str(block);
            }
            remaining = &block_and_after[block_end..];
        }

        rewritten.push_str(remaining);
        rewritten
    }
}

impl HtmlSyntaxToken {
    fn has_font_style(&self, property: &str) -> bool {
        [&self.light, &self.dark].into_iter().any(|style| {
            style.font_style.as_deref().is_some_and(|font_style| {
                css_font_style(font_style)
                    .iter()
                    .any(|(key, _)| *key == property)
            })
        })
    }
}

fn union_rule_specs(light: &TextMateTheme, dark: &TextMateTheme) -> Vec<TextMateRule> {
    let mut seen = BTreeSet::new();
    let mut specs = Vec::new();
    for rule in light.rules.iter().chain(dark.rules.iter()) {
        let Some(scope) = rule.scope.as_deref() else {
            continue;
        };
        if !seen.insert(scope.to_string()) {
            continue;
        }
        specs.push(TextMateRule {
            name: rule.name.clone(),
            scope: Some(scope.to_string()),
            foreground: None,
            background: None,
            font_style: None,
        });
    }
    specs
}

fn style_for_scope(
    theme: &TextMateTheme,
    scope: Option<&str>,
    fallback_foreground: &str,
) -> SyntaxStyle {
    let mut style = SyntaxStyle {
        foreground: Some(fallback_foreground.to_string()),
        background: None,
        font_style: None,
    };
    let Some(scope) = scope else {
        return style;
    };
    if let Some(rule) = theme.rules.iter().find(|rule| {
        rule.scope
            .as_deref()
            .is_some_and(|rule_scope| scope_matches(rule_scope, scope))
    }) {
        if rule.foreground.is_some() {
            style.foreground = rule.foreground.clone();
        }
        style.background = rule.background.clone();
        style.font_style = rule.font_style.clone();
    }
    style
}

fn fallback_syntax_tokens(
    light: &TextMateTheme,
    dark: &TextMateTheme,
    start: usize,
) -> Vec<HtmlSyntaxToken> {
    fallback_emitted_colors()
        .iter()
        .enumerate()
        .map(|(index, (emitted_color, scope))| {
            let variable_name = format!("fallback-{}", start + index);
            HtmlSyntaxToken {
                emitted_color: (*emitted_color).to_string(),
                class_name: format!("calepin-syntax-{variable_name}"),
                variable_name,
                light: style_for_scope(light, Some(scope), &light.foreground),
                dark: style_for_scope(dark, Some(scope), &dark.foreground),
            }
        })
        .collect()
}

fn fallback_emitted_colors() -> &'static [(&'static str, &'static str)] {
    &[
        ("#74747c", "comment"),
        ("#198810", "string"),
        ("#d73948", "constant.numeric"),
        ("#4b69c6", "support.type.property-name.toml"),
        ("#8b41b1", "keyword"),
        ("#b60157", "entity.other.attribute-name"),
    ]
}

fn scope_matches(rule_scope: &str, scope: &str) -> bool {
    rule_scope == scope
        || rule_scope.split(',').map(str::trim).any(|part| {
            part == scope
                || part.starts_with(&format!("{scope}."))
                || scope.starts_with(&format!("{part}."))
        })
}

fn sentinel_color(index: usize) -> String {
    format!("#{:06x}", index + 1)
}

fn push_declaration(declarations: &mut String, variable: &str, suffix: &str, value: &str) {
    declarations.push_str("  --");
    declarations.push_str(variable);
    declarations.push('-');
    declarations.push_str(suffix);
    declarations.push_str(": ");
    declarations.push_str(value);
    declarations.push_str(";\n");
}

fn css_font_style(font_style: &str) -> Vec<(&'static str, &'static str)> {
    let mut declarations = Vec::new();
    for part in font_style.split_whitespace() {
        match part {
            "bold" => declarations.push(("font-weight", "700")),
            "italic" => declarations.push(("font-style", "italic")),
            "underline" => declarations.push(("text-decoration", "underline")),
            _ => {}
        }
    }
    declarations
}

fn resolve_config_path(config_dir: &Path, path: &Path) -> std::path::PathBuf {
    absolutize_from(config_dir, path)
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
