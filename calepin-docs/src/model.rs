//! The extracted API model: what a documentation page needs to know about a
//! Python definition, with every type rendered as the author wrote it.

/// Where a parameter sits in the signature, which is what decides whether it
/// needs a `/` or `*` marker when the signature is rendered back out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    PositionalOnly,
    Positional,
    VarArgs,
    KeywordOnly,
    KwArgs,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    /// Annotation source text, verbatim (`int | None`, not `Optional[int]`).
    pub annotation: Option<String>,
    pub default: Option<String>,
    pub kind: ParamKind,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub qualname: String,
    pub is_async: bool,
    pub decorators: Vec<String>,
    /// PEP 695 type parameters, e.g. `[T]`, as written.
    pub type_params: Option<String>,
    pub params: Vec<Param>,
    pub returns: Option<String>,
    pub docstring: Option<String>,
    /// Set when the member came from a base class rather than this one.
    pub inherited_from: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: String,
    pub annotation: Option<String>,
    pub default: Option<String>,
    pub description: Option<String>,
    /// Set when the member came from a base class rather than this one.
    pub inherited_from: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Class {
    pub name: String,
    pub qualname: String,
    pub bases: Vec<String>,
    pub decorators: Vec<String>,
    pub type_params: Option<String>,
    pub docstring: Option<String>,
    pub methods: Vec<Function>,
    pub attributes: Vec<Attribute>,
}

#[derive(Debug, Clone)]
pub enum ApiItem {
    Function(Function),
    Class(Class),
}

impl ApiItem {
    pub fn name(&self) -> &str {
        match self {
            Self::Function(f) => &f.name,
            Self::Class(c) => &c.name,
        }
    }

    pub fn qualname(&self) -> &str {
        match self {
            Self::Function(f) => &f.qualname,
            Self::Class(c) => &c.qualname,
        }
    }

    pub fn docstring(&self) -> Option<&str> {
        match self {
            Self::Function(f) => f.docstring.as_deref(),
            Self::Class(c) => c.docstring.as_deref(),
        }
    }
}

impl Function {
    /// Reconstruct the call signature, restoring the `/` and `*` markers that
    /// are implied by parameter kind rather than stored as tokens.
    pub fn signature(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut emitted_posonly_marker = false;
        let mut emitted_kwonly_marker = false;

        for param in &self.params {
            if !emitted_posonly_marker
                && !matches!(param.kind, ParamKind::PositionalOnly)
                && self
                    .params
                    .iter()
                    .any(|p| p.kind == ParamKind::PositionalOnly)
            {
                parts.push("/".to_string());
                emitted_posonly_marker = true;
            }
            if param.kind == ParamKind::KeywordOnly
                && !emitted_kwonly_marker
                && !self.params.iter().any(|p| p.kind == ParamKind::VarArgs)
            {
                parts.push("*".to_string());
                emitted_kwonly_marker = true;
            }

            let prefix = match param.kind {
                ParamKind::VarArgs => "*",
                ParamKind::KwArgs => "**",
                _ => "",
            };
            let mut text = format!("{prefix}{}", param.name);
            if let Some(annotation) = &param.annotation {
                text.push_str(&format!(": {annotation}"));
            }
            if let Some(default) = &param.default {
                // PEP 8: no spaces around `=` unless the parameter is annotated.
                if param.annotation.is_some() {
                    text.push_str(&format!(" = {default}"));
                } else {
                    text.push_str(&format!("={default}"));
                }
            }
            parts.push(text);
        }

        let type_params = self.type_params.clone().unwrap_or_default();
        let returns = self
            .returns
            .as_ref()
            .map(|r| format!(" -> {r}"))
            .unwrap_or_default();
        let prefix = if self.is_async { "async " } else { "" };
        format!(
            "{prefix}{}{type_params}({}){returns}",
            self.name,
            parts.join(", ")
        )
    }
}

impl ApiItem {
    /// Rewrite every docstring this item owns, including its methods'.
    pub fn map_docstrings(&mut self, rewrite: &dyn Fn(&str) -> String) {
        fn apply(slot: &mut Option<String>, rewrite: &dyn Fn(&str) -> String) {
            if let Some(text) = slot {
                *slot = Some(rewrite(text));
            }
        }

        match self {
            Self::Function(function) => apply(&mut function.docstring, rewrite),
            Self::Class(class) => {
                apply(&mut class.docstring, rewrite);
                for method in &mut class.methods {
                    apply(&mut method.docstring, rewrite);
                }
            }
        }
    }

    /// Whether any docstring here has a `{placeholder}` left to fill.
    pub fn has_placeholder(&self) -> bool {
        let has = |slot: &Option<String>| slot.as_ref().map(|t| t.contains('{')).unwrap_or(false);
        match self {
            Self::Function(function) => has(&function.docstring),
            Self::Class(class) => {
                has(&class.docstring) || class.methods.iter().any(|m| has(&m.docstring))
            }
        }
    }
}
