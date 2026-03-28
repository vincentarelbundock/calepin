// ISC License
//
// Copyright (c) 2021, Timothee Mazzucotelli
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

//! AST walking and docstring extraction using the ruff Python parser.
//!
//! Parses Python source files and extracts function/class definitions with
//! their docstrings and parameter signatures.

use std::collections::HashSet;

use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_python_parser::parse_module;
use ruff_text_size::Ranged;

use super::types::{PyObject, PyObjectKind, PyParam};

/// Extract the string value from a `StringLiteral` AST node.
fn string_literal_value(str_lit: &ast::ExprStringLiteral) -> String {
    str_lit
        .value
        .iter()
        .map(|part| part.value.as_ref())
        .collect::<Vec<_>>()
        .join("")
}

/// Parse a Python source file and extract all documented public objects.
pub fn extract_objects(source: &str, module_path: &str) -> Vec<PyObject> {
    let parsed = match parse_module(source) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let mut objects = Vec::new();
    walk_body(&parsed.syntax().body, source, module_path, &mut objects);
    objects
}

/// Extract `__all__` names from a Python source file.
///
/// Handles:
/// - `__all__ = ["name1", "name2"]`
/// - `__all__ = ("name1", "name2")`
/// - `__all__ += ["name3"]`
/// - `__all__ = list(SOME_DICT.keys())` -- falls back to extracting keys
///   from dict literals assigned to the referenced variable.
pub fn extract_all_names(source: &str) -> HashSet<String> {
    let parsed = match parse_module(source) {
        Ok(p) => p,
        Err(_) => return HashSet::new(),
    };
    let body = &parsed.syntax().body;
    let mut names = HashSet::new();

    for stmt in body {
        match stmt {
            // __all__ = [...]  or  __all__ = (...)
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    if is_all_name(target) {
                        if !collect_string_elements(&assign.value, &mut names) {
                            // Couldn't extract literals -- try to resolve
                            // patterns like list(DICT.keys()) or DICT.keys()
                            if let Some(dict_name) = extract_dict_keys_ref(&assign.value) {
                                collect_dict_keys(body, &dict_name, &mut names);
                            }
                        }
                    }
                }
            }
            // __all__ += [...]
            Stmt::AugAssign(aug) => {
                if is_all_name(&aug.target) {
                    collect_string_elements(&aug.value, &mut names);
                }
            }
            // __all__: list[str] = [...]
            Stmt::AnnAssign(ann) => {
                if let Some(value) = &ann.value {
                    if is_all_name(&ann.target) {
                        collect_string_elements(value, &mut names);
                    }
                }
            }
            _ => {}
        }
    }
    names
}

/// Extract names imported at module level (from `__init__.py`).
///
/// Handles:
/// - `from .sub import name1, name2`
/// - `from .sub import name1 as alias`
/// - `import sub.name` (takes last component)
pub fn extract_imports(source: &str) -> HashSet<String> {
    let parsed = match parse_module(source) {
        Ok(p) => p,
        Err(_) => return HashSet::new(),
    };
    let mut names = HashSet::new();
    for stmt in &parsed.syntax().body {
        match stmt {
            Stmt::ImportFrom(imp) => {
                for alias in &imp.names {
                    // Use the alias if present, otherwise the original name
                    let name = alias
                        .asname
                        .as_ref()
                        .map(|a| a.as_str())
                        .unwrap_or_else(|| alias.name.as_str());
                    if name != "*" {
                        names.insert(name.to_string());
                    }
                }
            }
            Stmt::Import(imp) => {
                for alias in &imp.names {
                    let name = alias
                        .asname
                        .as_ref()
                        .map(|a| a.as_str())
                        .unwrap_or_else(|| {
                            alias.name.as_str().rsplit('.').next().unwrap_or("")
                        });
                    names.insert(name.to_string());
                }
            }
            _ => {}
        }
    }
    names
}

/// Check if an expression is the name `__all__`.
fn is_all_name(expr: &Expr) -> bool {
    matches!(expr, Expr::Name(name) if name.id.as_str() == "__all__")
}

/// Collect string literals from a list or tuple expression.
/// Returns `true` if any string literals were found.
fn collect_string_elements(expr: &Expr, out: &mut HashSet<String>) -> bool {
    let elts = match expr {
        Expr::List(list) => &list.elts,
        Expr::Tuple(tuple) => &tuple.elts,
        Expr::BinOp(binop) => {
            let left = collect_string_elements(&binop.left, out);
            let right = collect_string_elements(&binop.right, out);
            return left || right;
        }
        _ => return false,
    };
    let mut found = false;
    for elt in elts {
        if let Some(s) = string_value(elt) {
            out.insert(s);
            found = true;
        }
    }
    found
}

/// Try to extract a variable name from `list(VAR.keys())` or `VAR.keys()`.
///
/// Matches patterns like:
/// - `list(_EXPORTS.keys())`
/// - `_EXPORTS.keys()`
fn extract_dict_keys_ref(expr: &Expr) -> Option<String> {
    // list(VAR.keys())
    if let Expr::Call(call) = expr {
        // Check if it's list(something.keys())
        if let Expr::Name(func_name) = &*call.func {
            if func_name.id.as_str() == "list" {
                if let Some(first_arg) = call.arguments.args.first() {
                    return extract_dict_keys_ref(first_arg);
                }
            }
        }
        // Check if it's VAR.keys()
        if let Expr::Attribute(attr) = &*call.func {
            if attr.attr.as_str() == "keys" {
                if let Expr::Name(name) = &*attr.value {
                    return Some(name.id.as_str().to_string());
                }
            }
        }
    }
    None
}

/// Find a dict literal assigned to `var_name` and collect its string keys.
fn collect_dict_keys(body: &[Stmt], var_name: &str, out: &mut HashSet<String>) {
    for stmt in body {
        if let Stmt::Assign(assign) = stmt {
            for target in &assign.targets {
                if let Expr::Name(name) = target {
                    if name.id.as_str() == var_name {
                        if let Expr::Dict(dict) = &*assign.value {
                            for key in dict.iter_keys() {
                                if let Some(key_expr) = key {
                                    if let Some(s) = string_value(key_expr) {
                                        out.insert(s);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Extract a plain string value from a string literal expression.
fn string_value(expr: &Expr) -> Option<String> {
    if let Expr::StringLiteral(str_lit) = expr {
        Some(string_literal_value(str_lit))
    } else {
        None
    }
}

/// Walk a list of statements and extract documented functions and classes.
fn walk_body(body: &[Stmt], source: &str, prefix: &str, out: &mut Vec<PyObject>) {
    for stmt in body {
        match stmt {
            Stmt::FunctionDef(func) => {
                let name = func.name.as_str();
                if name.starts_with('_') {
                    continue;
                }
                let path = format!("{}.{}", prefix, name);
                let docstring = extract_docstring(&func.body)
                    .or_else(|| extract_decorator_docstring(&func.decorator_list));
                let parameters = extract_parameters(&func.parameters, source);
                out.push(PyObject {
                    name: name.to_string(),
                    path,
                    kind: PyObjectKind::Function,
                    docstring,
                    parameters,
                });
            }
            Stmt::ClassDef(class) => {
                let name = class.name.as_str();
                if name.starts_with('_') {
                    continue;
                }
                let path = format!("{}.{}", prefix, name);
                let docstring = extract_docstring(&class.body)
                    .or_else(|| extract_decorator_docstring(&class.decorator_list));

                // Extract __init__ parameters if available
                let parameters = extract_init_params(&class.body, source);

                out.push(PyObject {
                    name: name.to_string(),
                    path: path.clone(),
                    kind: PyObjectKind::Class,
                    docstring,
                    parameters,
                });

                // Also extract public methods
                for stmt in &class.body {
                    if let Stmt::FunctionDef(method) = stmt {
                        let mname = method.name.as_str();
                        if mname.starts_with('_') {
                            continue;
                        }
                        let mpath = format!("{}.{}", path, mname);
                        let mdoc = extract_docstring(&method.body)
                            .or_else(|| extract_decorator_docstring(&method.decorator_list));
                        let mparams = extract_parameters(&method.parameters, source);
                        // Skip `self`/`cls` first parameter
                        let mparams = skip_self(mparams);
                        out.push(PyObject {
                            name: mname.to_string(),
                            path: mpath,
                            kind: PyObjectKind::Function,
                            docstring: mdoc,
                            parameters: mparams,
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

/// Extract a docstring from a decorator like `@doc("""...""")`.
///
/// Some packages (e.g., marginaleffects) use a decorator pattern where the
/// docstring is passed as a string argument to a decorator call rather than
/// placed as the first statement in the function body. This function checks
/// each decorator for a call whose first positional argument is a string
/// literal, and returns that string as the docstring.
fn extract_decorator_docstring(
    decorators: &[ast::Decorator],
) -> Option<String> {
    for decorator in decorators {
        // Look for @something("""...""") -- a Call with a string first arg
        if let Expr::Call(call) = &decorator.expression {
            if let Some(first_arg) = call.arguments.args.first() {
                if let Expr::StringLiteral(str_lit) = first_arg {
                    let text = clean_docstring(&string_literal_value(str_lit));
                    if !text.trim().is_empty() {
                        // Strip unresolved {placeholder} template variables
                        let text = strip_placeholders(&text);
                        return Some(text);
                    }
                }
            }
        }
    }
    None
}

/// Remove lines that consist solely of an unresolved `{placeholder}` reference.
///
/// Decorator-based docstrings often use `{param_model}` style placeholders
/// that are interpolated at runtime. Since we parse statically, we strip
/// lines that are just a bare placeholder so the output stays clean.
fn strip_placeholders(text: &str) -> String {
    let mut lines = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        // Skip lines that are just {word} or {word_word}
        if trimmed.starts_with('{')
            && trimmed.ends_with('}')
            && !trimmed.contains(' ')
            && trimmed.len() > 2
        {
            continue;
        }
        lines.push(line.to_string());
    }
    // Collapse runs of 3+ blank lines to 2
    let mut result = Vec::new();
    let mut blank_count = 0;
    for line in &lines {
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                result.push(line.clone());
            }
        } else {
            blank_count = 0;
            result.push(line.clone());
        }
    }
    // Trim trailing blanks
    while result.last().map_or(false, |l| l.trim().is_empty()) {
        result.pop();
    }
    result.join("\n")
}

/// Extract the docstring from the first statement of a body, if it is a
/// string literal expression.
fn extract_docstring(body: &[Stmt]) -> Option<String> {
    let first = body.first()?;
    if let Stmt::Expr(ast::StmtExpr { value, .. }) = first {
        if let Expr::StringLiteral(str_lit) = value.as_ref() {
            let text = string_literal_value(str_lit);
            if text.trim().is_empty() {
                return None;
            }
            return Some(clean_docstring(&text));
        }
    }
    None
}

/// Clean a raw docstring: trim leading/trailing blank lines, dedent.
fn clean_docstring(raw: &str) -> String {
    let lines: Vec<&str> = raw.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    // First line may not be indented
    let first = lines[0].trim();
    let rest = &lines[1..];

    // Find minimum indentation of non-empty lines after the first
    let min_indent = rest
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    let mut cleaned = Vec::new();
    cleaned.push(first.to_string());
    for line in rest {
        if line.trim().is_empty() {
            cleaned.push(String::new());
        } else if line.len() >= min_indent {
            cleaned.push(line[min_indent..].to_string());
        } else {
            cleaned.push(line.trim().to_string());
        }
    }

    // Trim trailing empty lines
    while cleaned.last().map_or(false, |l| l.is_empty()) {
        cleaned.pop();
    }

    cleaned.join("\n")
}

/// Extract parameters from a function definition's parameter list.
fn extract_parameters(params: &ast::Parameters, source: &str) -> Vec<PyParam> {
    let mut result = Vec::new();

    // Positional-only parameters
    for pwd in &params.posonlyargs {
        result.push(param_with_default(pwd, source));
    }

    // Regular parameters
    for pwd in &params.args {
        result.push(param_with_default(pwd, source));
    }

    // *args
    if let Some(vararg) = &params.vararg {
        result.push(PyParam {
            name: format!("*{}", vararg.name.as_str()),
            annotation: vararg.annotation.as_ref().map(|a| expr_to_source(a, source)),
            default: None,
        });
    }

    // Keyword-only parameters
    for pwd in &params.kwonlyargs {
        result.push(param_with_default(pwd, source));
    }

    // **kwargs
    if let Some(kwarg) = &params.kwarg {
        result.push(PyParam {
            name: format!("**{}", kwarg.name.as_str()),
            annotation: kwarg.annotation.as_ref().map(|a| expr_to_source(a, source)),
            default: None,
        });
    }

    result
}

/// Build a PyParam from a ParameterWithDefault.
fn param_with_default(
    pwd: &ast::ParameterWithDefault,
    source: &str,
) -> PyParam {
    PyParam {
        name: pwd.parameter.name.as_str().to_string(),
        annotation: pwd
            .parameter
            .annotation
            .as_ref()
            .map(|a| expr_to_source(a, source)),
        default: pwd.default.as_ref().map(|d| expr_to_source(d, source)),
    }
}

/// Extract `__init__` parameters for a class (skipping `self`).
fn extract_init_params(body: &[Stmt], source: &str) -> Vec<PyParam> {
    for stmt in body {
        if let Stmt::FunctionDef(func) = stmt {
            if func.name.as_str() == "__init__" {
                let params = extract_parameters(&func.parameters, source);
                return skip_self(params);
            }
        }
    }
    Vec::new()
}

/// Remove the first parameter if it is `self` or `cls`.
fn skip_self(mut params: Vec<PyParam>) -> Vec<PyParam> {
    if let Some(first) = params.first() {
        if first.name == "self" || first.name == "cls" {
            params.remove(0);
        }
    }
    params
}

/// Convert an AST expression to its source text representation.
///
/// Uses the TextRange of the expression to slice back into the original source.
fn expr_to_source(expr: &Expr, source: &str) -> String {
    let range = expr.range();
    let start: usize = range.start().into();
    let end: usize = range.end().into();
    if end <= source.len() {
        source[start..end].to_string()
    } else {
        // Fallback: try to reconstruct a simple representation
        format!("{:?}", expr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_function() {
        let source = r#"
def greet(name: str, greeting: str = "Hello") -> str:
    """Say hello to someone.

    Args:
        name: The person's name.
        greeting: The greeting word.

    Returns:
        A greeting string.
    """
    return f"{greeting}, {name}!"
"#;
        let objects = extract_objects(source, "mymod");
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].name, "greet");
        assert_eq!(objects[0].parameters.len(), 2);
        assert_eq!(objects[0].parameters[0].name, "name");
        assert_eq!(
            objects[0].parameters[0].annotation.as_deref(),
            Some("str")
        );
        assert!(objects[0].parameters[1].default.is_some());
        assert!(objects[0].docstring.is_some());
    }

    #[test]
    fn test_extract_class() {
        let source = r#"
class Dog:
    """A good dog.

    Attributes:
        name: The dog's name.
    """

    def __init__(self, name: str, breed: str = "mutt"):
        self.name = name
        self.breed = breed

    def bark(self):
        """Make noise."""
        print("Woof!")
"#;
        let objects = extract_objects(source, "animals");
        // Should get Dog + bark (private methods skipped)
        assert!(objects.len() >= 2);
        // Class should have __init__ params (minus self)
        let dog = &objects[0];
        assert_eq!(dog.name, "Dog");
        assert_eq!(dog.parameters.len(), 2);
        assert_eq!(dog.parameters[0].name, "name");
    }

    #[test]
    fn test_skip_private() {
        let source = r#"
def _private():
    """Hidden."""
    pass

def public():
    """Visible."""
    pass
"#;
        let objects = extract_objects(source, "mod");
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].name, "public");
    }

    #[test]
    fn test_decorator_docstring() {
        let source = r#"
def doc(docstring):
    def decorator(func):
        func.__doc__ = docstring
        return func
    return decorator

@doc("""Do something.

Parameters
----------
x : int
    The input.

{shared_param}

Returns
-------
bool
    Result.
""")
def predictions(model, x=None):
    pass
"#;
        let objects = extract_objects(source, "pkg");
        let pred = objects.iter().find(|o| o.name == "predictions").unwrap();
        let doc = pred.docstring.as_ref().unwrap();
        assert!(doc.contains("Do something."));
        assert!(doc.contains("x : int"));
        // {shared_param} placeholder should be stripped
        assert!(!doc.contains("{shared_param}"));
    }

    #[test]
    fn test_clean_docstring() {
        let raw = "First line.\n    Indented.\n    More.\n    ";
        let cleaned = clean_docstring(raw);
        assert_eq!(cleaned, "First line.\nIndented.\nMore.");
    }

    #[test]
    fn test_extract_all_names() {
        let source = r#"
__all__ = ["foo", "bar"]
"#;
        let names = extract_all_names(source);
        assert_eq!(names.len(), 2);
        assert!(names.contains("foo"));
        assert!(names.contains("bar"));
    }

    #[test]
    fn test_extract_all_names_tuple() {
        let source = r#"
__all__ = ("foo", "bar")
__all__ += ["baz"]
"#;
        let names = extract_all_names(source);
        assert_eq!(names.len(), 3);
        assert!(names.contains("baz"));
    }

    #[test]
    fn test_extract_all_names_concat() {
        let source = r#"
__all__ = other.__all__ + ["local"]
"#;
        let names = extract_all_names(source);
        assert!(names.contains("local"));
    }

    #[test]
    fn test_extract_all_dict_keys() {
        let source = r#"
_EXPORTS = {
    "predictions": ("mod.predictions", "predictions"),
    "comparisons": ("mod.comparisons", "comparisons"),
    "slopes": ("mod.slopes", "slopes"),
}

__all__ = list(_EXPORTS.keys())
"#;
        let names = extract_all_names(source);
        assert_eq!(names.len(), 3);
        assert!(names.contains("predictions"));
        assert!(names.contains("comparisons"));
        assert!(names.contains("slopes"));
    }

    #[test]
    fn test_extract_imports() {
        let source = r#"
from .predictions import predictions
from .comparisons import comparisons as comp
from .datagrid import datagrid
import os
"#;
        let names = extract_imports(source);
        assert!(names.contains("predictions"));
        assert!(names.contains("comp"));
        assert!(names.contains("datagrid"));
        assert!(names.contains("os"));
    }
}
