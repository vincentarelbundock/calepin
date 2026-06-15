use std::collections::BTreeSet;

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

fn html_color_variants(color: &str) -> Vec<String> {
    let lower = color.to_ascii_lowercase();
    let upper = color.to_ascii_uppercase();
    if lower == upper {
        vec![lower]
    } else {
        vec![lower, upper]
    }
}
