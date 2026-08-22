//! Walk a parsed Python module and pull out the documentable definitions.
//!
//! Every type and default is captured by slicing the original source over the
//! node's range, so the rendered docs show what the author wrote rather than a
//! normalized or evaluated form.

use anyhow::{Context, Result};
use ruff_python_ast::{Expr, Parameters, Stmt, StmtClassDef, StmtFunctionDef};
use ruff_python_parser::parse_module;
use ruff_text_size::Ranged;

use crate::model::{ApiItem, Attribute, Class, Function, Param, ParamKind};

/// A parsed module, kept alongside its source so ranges stay resolvable.
pub struct ParsedModule {
    pub source: String,
    pub body: Vec<Stmt>,
}

pub fn parse_source(source: String) -> Result<ParsedModule> {
    let parsed = parse_module(&source).context("failed to parse Python source")?;
    let body = parsed.into_syntax().body.into_iter().collect();
    Ok(ParsedModule { source, body })
}

/// Slice the source over a node's range — the verbatim-annotation trick.
fn slice(source: &str, node: &impl Ranged) -> String {
    source[node.range()].trim().to_string()
}

/// A docstring is the first statement of a body, if it is a bare string.
fn docstring_of(source: &str, body: &[Stmt]) -> Option<String> {
    let Stmt::Expr(expr) = body.first()? else {
        return None;
    };
    let Expr::StringLiteral(literal) = expr.value.as_ref() else {
        return None;
    };
    let _ = source;
    Some(dedent(literal.value.to_str()))
}

/// Strip the common leading indentation from a docstring body, the way
/// `inspect.cleandoc` does, so nested definitions do not render pre-indented.
fn dedent(text: &str) -> String {
    let mut lines = text.lines();
    let first = lines.next().unwrap_or("").trim_start().to_string();
    let rest: Vec<&str> = lines.collect();

    let indent = rest
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);

    let mut out = vec![first];
    for line in rest {
        if line.len() >= indent {
            out.push(line[indent..].trim_end().to_string());
        } else {
            out.push(line.trim().to_string());
        }
    }
    while out.last().map(|l| l.is_empty()).unwrap_or(false) {
        out.pop();
    }
    out.join("\n")
}

fn with_default(source: &str, p: &ruff_python_ast::ParameterWithDefault, kind: ParamKind) -> Param {
    Param {
        name: p.parameter.name.as_str().to_string(),
        annotation: p.parameter.annotation.as_ref().map(|a| slice(source, &**a)),
        default: p.default.as_ref().map(|d| slice(source, &**d)),
        kind,
    }
}

fn params_of(source: &str, parameters: &Parameters) -> Vec<Param> {
    let mut out = Vec::new();

    for p in &parameters.posonlyargs {
        out.push(with_default(source, p, ParamKind::PositionalOnly));
    }
    for p in &parameters.args {
        out.push(with_default(source, p, ParamKind::Positional));
    }
    if let Some(vararg) = &parameters.vararg {
        out.push(Param {
            name: vararg.name.as_str().to_string(),
            annotation: vararg.annotation.as_ref().map(|a| slice(source, &**a)),
            default: None,
            kind: ParamKind::VarArgs,
        });
    }
    for p in &parameters.kwonlyargs {
        out.push(with_default(source, p, ParamKind::KeywordOnly));
    }
    if let Some(kwarg) = &parameters.kwarg {
        out.push(Param {
            name: kwarg.name.as_str().to_string(),
            annotation: kwarg.annotation.as_ref().map(|a| slice(source, &**a)),
            default: None,
            kind: ParamKind::KwArgs,
        });
    }

    out
}

fn decorators_of(source: &str, decorators: &[ruff_python_ast::Decorator]) -> Vec<String> {
    decorators
        .iter()
        .map(|d| slice(source, &d.expression))
        .collect()
}

pub fn function_from(source: &str, def: &StmtFunctionDef, scope: &str) -> Function {
    let name = def.name.as_str().to_string();

    let literal_docstring = docstring_of(source, &def.body);
    let decorator_docstring = literal_docstring
        .is_none()
        .then(|| docstring_from_decorator(&def.decorator_list))
        .flatten();

    // A decorator consumed as documentation is not also listed as behaviour;
    // rendering the whole docstring twice helps nobody.
    let decorators = def
        .decorator_list
        .iter()
        .filter(|d| !(decorator_docstring.is_some() && carries_docstring(d)))
        .map(|d| slice(source, &d.expression))
        .collect();

    Function {
        qualname: qualify(scope, &name),
        name,
        is_async: def.is_async,
        decorators,
        type_params: def.type_params.as_ref().map(|tp| slice(source, &**tp)),
        params: params_of(source, &def.parameters),
        returns: def.returns.as_ref().map(|r| slice(source, &**r)),
        docstring: literal_docstring.or(decorator_docstring),
        inherited_from: None,
    }
}

pub fn class_from(source: &str, def: &StmtClassDef, scope: &str) -> Class {
    let name = def.name.as_str().to_string();
    let qualname = qualify(scope, &name);

    let mut methods = Vec::new();
    let mut attributes = Vec::new();

    for stmt in &def.body {
        match stmt {
            Stmt::FunctionDef(f)
                if !is_private(f.name.as_str()) || f.name.as_str() == "__init__" =>
            {
                let mut method = function_from(source, f, &qualname);
                drop_receiver(&mut method);
                if is_property_accessor(&method) {
                    // The getter already carries the documentation.
                } else if is_property(&method) {
                    attributes.push(property_as_attribute(method));
                } else {
                    methods.push(method);
                }
            }
            // `name: type = default` — an annotated class attribute.
            Stmt::AnnAssign(assign) => {
                if let Expr::Name(target) = assign.target.as_ref() {
                    if !is_private(target.id.as_str()) {
                        attributes.push(Attribute {
                            name: target.id.to_string(),
                            annotation: Some(slice(source, &*assign.annotation)),
                            default: assign.value.as_ref().map(|v| slice(source, &**v)),
                            description: None,
                            inherited_from: None,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    Class {
        name,
        qualname,
        bases: def.bases().iter().map(|b| slice(source, b)).collect(),
        decorators: decorators_of(source, &def.decorator_list),
        type_params: def.type_params.as_ref().map(|tp| slice(source, &**tp)),
        docstring: docstring_of(source, &def.body),
        methods,
        attributes,
    }
}

/// Find a top-level definition by name in an already-parsed module.
pub fn find_item(module: &ParsedModule, name: &str, scope: &str) -> Option<ApiItem> {
    for stmt in &module.body {
        match stmt {
            Stmt::FunctionDef(f) if f.name.as_str() == name => {
                let mut item = ApiItem::Function(function_from(&module.source, f, scope));
                fill_aliased_docstring(module, &mut item);
                return Some(item);
            }
            Stmt::ClassDef(c) if c.name.as_str() == name => {
                return Some(ApiItem::Class(class_from(&module.source, c, scope)));
            }
            _ => {}
        }
    }
    None
}

/// Every public top-level definition, for modules with no `__all__`.
pub fn public_items(module: &ParsedModule, scope: &str) -> Vec<ApiItem> {
    let mut out = Vec::new();
    for stmt in &module.body {
        match stmt {
            Stmt::FunctionDef(f) if !is_private(f.name.as_str()) => {
                let mut item = ApiItem::Function(function_from(&module.source, f, scope));
                fill_aliased_docstring(module, &mut item);
                out.push(item);
            }
            Stmt::ClassDef(c) if !is_private(c.name.as_str()) => {
                out.push(ApiItem::Class(class_from(&module.source, c, scope)));
            }
            _ => {}
        }
    }
    out
}

fn qualify(scope: &str, name: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        format!("{scope}.{name}")
    }
}

fn is_private(name: &str) -> bool {
    name.starts_with('_')
}

/// Drop the implicit `self` / `cls` receiver: it is part of the definition but
/// not of the call, and documenting it only adds noise.
fn drop_receiver(method: &mut Function) {
    let is_receiver = method
        .params
        .first()
        .map(|p| {
            matches!(p.kind, ParamKind::Positional | ParamKind::PositionalOnly)
                && (p.name == "self" || p.name == "cls")
                && p.annotation.is_none()
        })
        .unwrap_or(false);
    if is_receiver {
        method.params.remove(0);
    }
}

/// A docstring supplied by a decorator rather than a literal body string.
///
/// Some projects set `__doc__` from a decorator so the text can be assembled
/// from shared fragments (`@doc("...")`, `@add_docstring("...")`). Runtime
/// introspection sees the result; static analysis has to read the argument.
/// Any decorator call whose first argument is a plain string literal counts —
/// the pattern is what identifies it, not the decorator's name.
fn docstring_from_decorator(decorators: &[ruff_python_ast::Decorator]) -> Option<String> {
    decorators.iter().find_map(|decorator| {
        let Expr::Call(call) = &decorator.expression else {
            return None;
        };
        let Some(Expr::StringLiteral(literal)) = call.arguments.args.first() else {
            return None;
        };
        Some(dedent(literal.value.to_str()))
    })
}

/// Whether this decorator is the one carrying the docstring.
fn carries_docstring(decorator: &ruff_python_ast::Decorator) -> bool {
    let Expr::Call(call) = &decorator.expression else {
        return false;
    };
    matches!(call.arguments.args.first(), Some(Expr::StringLiteral(_)))
}

/// Resolve `target.__doc__ = source.__doc__`, a module-level docstring alias.
///
/// Wrapper functions frequently borrow their documentation from the function
/// they delegate to. The assignment is a plain statement, so the alias reads
/// statically even though only the runtime object carries the result.
fn aliased_docstring(module: &ParsedModule, name: &str) -> Option<String> {
    let source_name = module.body.iter().find_map(|stmt| {
        let Stmt::Assign(assign) = stmt else {
            return None;
        };
        if !assign.targets.iter().any(|t| is_doc_attribute(t, name)) {
            return None;
        }
        let Expr::Attribute(attribute) = assign.value.as_ref() else {
            return None;
        };
        if attribute.attr.as_str() != "__doc__" {
            return None;
        }
        let Expr::Name(source) = attribute.value.as_ref() else {
            return None;
        };
        Some(source.id.to_string())
    })?;

    // Guard against `a.__doc__ = a.__doc__`, which would recurse forever.
    if source_name == name {
        return None;
    }

    module.body.iter().find_map(|stmt| match stmt {
        Stmt::FunctionDef(f) if f.name.as_str() == source_name => {
            docstring_of(&module.source, &f.body)
                .or_else(|| docstring_from_decorator(&f.decorator_list))
                .or_else(|| aliased_docstring(module, &source_name))
        }
        _ => None,
    })
}

/// Whether `expr` is `<name>.__doc__`.
fn is_doc_attribute(expr: &Expr, name: &str) -> bool {
    let Expr::Attribute(attribute) = expr else {
        return false;
    };
    if attribute.attr.as_str() != "__doc__" {
        return false;
    }
    matches!(attribute.value.as_ref(), Expr::Name(n) if n.id.as_str() == name)
}

/// Fill in a docstring the definition itself does not carry.
fn fill_aliased_docstring(module: &ParsedModule, item: &mut ApiItem) {
    let ApiItem::Function(function) = item else {
        return;
    };
    if function.docstring.is_none() {
        function.docstring = aliased_docstring(module, &function.name);
    }
}

/// Decorator names that turn a method into a read-only attribute.
const PROPERTY_DECORATORS: &[&str] = &["property", "cached_property", "functools.cached_property"];

/// Whether this method is really a property, and so belongs with the
/// attributes rather than among the callables.
fn is_property(function: &Function) -> bool {
    function
        .decorators
        .iter()
        .any(|d| PROPERTY_DECORATORS.contains(&d.as_str()))
}

/// A property's setter or deleter, which documents nothing the getter has not
/// already said.
fn is_property_accessor(function: &Function) -> bool {
    function
        .decorators
        .iter()
        .any(|d| d.ends_with(".setter") || d.ends_with(".deleter"))
}

/// Turn a property getter into an attribute: its return type is the attribute
/// type, and its docstring summary is the description.
fn property_as_attribute(function: Function) -> Attribute {
    Attribute {
        name: function.name,
        annotation: function.returns,
        default: None,
        description: function.docstring,
        inherited_from: function.inherited_from,
    }
}
