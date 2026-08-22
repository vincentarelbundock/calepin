//! Resolve a package's public surface: read `__all__`, follow the re-export
//! chain through `from .module import Name`, and load the defining module.
//!
//! This is deliberately a *bounded* subset of what a full static analyzer does.
//! It handles the common documentation case — a package whose `__init__.py`
//! re-exports names from submodules — and reports what it could not resolve
//! instead of guessing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use ruff_python_ast::{Expr, Stmt};

use crate::extract::{find_item, parse_source, public_items, ParsedModule};
use crate::model::ApiItem;

/// How deep to follow a re-export chain before giving up.
const MAX_HOPS: usize = 8;

pub struct Package {
    pub name: String,
    /// Directory holding `__init__.py`, or the parent of a single-module package.
    pub root: PathBuf,
}

/// A name the resolver could not trace to a definition, kept so the caller can
/// report it rather than silently dropping part of the API.
#[derive(Debug)]
pub struct Unresolved {
    pub name: String,
    pub reason: String,
}

pub struct Resolution {
    pub items: Vec<ApiItem>,
    pub unresolved: Vec<Unresolved>,
}

impl Package {
    pub fn open(path: &Path) -> Result<Self> {
        let path = path
            .canonicalize()
            .with_context(|| format!("cannot open {}", path.display()))?;

        if path.is_dir() {
            if !path.join("__init__.py").exists() {
                bail!(
                    "{} has no __init__.py — point at a package directory or a single .py file",
                    path.display()
                );
            }
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            Ok(Self { name, root: path })
        } else if path.extension().map(|e| e == "py").unwrap_or(false) {
            let name = path
                .file_stem()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
            Ok(Self { name, root })
        } else {
            bail!(
                "{} is neither a package directory nor a .py file",
                path.display()
            )
        }
    }

    fn entry_point(&self) -> PathBuf {
        let init = self.root.join("__init__.py");
        if init.exists() {
            init
        } else {
            self.root.join(format!("{}.py", self.name))
        }
    }

    /// The package's documented API, in `__all__` order when one is declared.
    pub fn resolve(&self) -> Result<Resolution> {
        let entry = self.entry_point();
        let module = load(&entry)?;

        let Some(names) = read_all(&module) else {
            // No `__all__`: fall back to every public top-level definition.
            return Ok(Resolution {
                items: public_items(&module, &self.name),
                unresolved: Vec::new(),
            });
        };

        let mut items = Vec::new();
        let mut unresolved = Vec::new();

        for name in names {
            match self.trace(&name, &entry, 0) {
                Ok(Some(item)) => items.push(item),
                Ok(None) => unresolved.push(Unresolved {
                    name: name.clone(),
                    reason: "not defined or re-exported in a module we could locate".to_string(),
                }),
                Err(err) => unresolved.push(Unresolved {
                    name: name.clone(),
                    reason: err.to_string(),
                }),
            }
        }

        // Docstrings assembled by `str.format` still carry their placeholders;
        // fill them from the package's shared fragments. The scan touches
        // every module, so only pay for it when something needs filling.
        if items.iter().any(|item| item.has_placeholder()) {
            let substitutions = self.substitutions();
            for item in &mut items {
                item.map_docstrings(&|text| substitute(text, &substitutions));
            }
        }

        Ok(Resolution { items, unresolved })
    }

    /// Follow `name` from `module_path` to wherever it is actually defined.
    fn trace(&self, name: &str, module_path: &Path, hop: usize) -> Result<Option<ApiItem>> {
        if hop >= MAX_HOPS {
            bail!("re-export chain deeper than {MAX_HOPS} hops");
        }

        let module = load(module_path)?;
        let scope = self.scope_for(module_path);

        // Defined right here?
        if let Some(item) = find_item(&module, name, &scope) {
            return Ok(Some(item));
        }

        // Otherwise it is imported: either by a real import statement, or by a
        // lazy-export table that describes one.
        let Some((target_module, original_name)) =
            find_import(&module, name).or_else(|| lazy_export(&module, name))
        else {
            return Ok(None);
        };

        let Some(next_path) = self.module_path(&target_module, module_path) else {
            bail!("cannot locate module `{target_module}` on disk");
        };

        self.trace(&original_name, &next_path, hop + 1)
    }

    /// Dotted scope for a module file, e.g. `pkg.sub` for `pkg/sub/__init__.py`.
    fn scope_for(&self, path: &Path) -> String {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return self.name.clone();
        };
        let mut parts = vec![self.name.clone()];
        for component in relative.components() {
            let piece = component.as_os_str().to_string_lossy();
            if piece == "__init__.py" {
                continue;
            }
            parts.push(piece.trim_end_matches(".py").to_string());
        }
        parts.join(".")
    }

    /// Turn an import target into a file path. `level` dots are already folded
    /// into `target` by [`find_import`] as a leading `.` count.
    fn module_path(&self, target: &str, from: &Path) -> Option<PathBuf> {
        let relative_depth = target.chars().take_while(|c| *c == '.').count();
        let cleaned = target.trim_start_matches('.');

        let mut base = if relative_depth > 0 {
            let mut dir = from.parent()?.to_path_buf();
            // One dot means "this package"; each extra dot climbs a level.
            for _ in 1..relative_depth {
                dir = dir.parent()?.to_path_buf();
            }
            dir
        } else {
            // Absolute import: only follow it if it points back into this package.
            let head = cleaned.split('.').next()?;
            if head != self.name {
                return None;
            }
            self.root.clone()
        };

        let segments: Vec<&str> = cleaned
            .split('.')
            .filter(|s| !s.is_empty())
            .skip(usize::from(relative_depth == 0))
            .collect();

        for segment in &segments {
            base = base.join(segment);
        }

        let as_file = base.with_extension("py");
        if as_file.exists() {
            return Some(as_file);
        }
        let as_package = base.join("__init__.py");
        if as_package.exists() {
            return Some(as_package);
        }
        None
    }
}

fn load(path: &Path) -> Result<ParsedModule> {
    let source =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    parse_source(source).with_context(|| format!("in {}", path.display()))
}

/// Read `__all__ = [...]` / `(...)`, returning the string literals in order.
fn read_all(module: &ParsedModule) -> Option<Vec<String>> {
    for stmt in &module.body {
        let value = match stmt {
            Stmt::Assign(assign) => {
                let is_all = assign
                    .targets
                    .iter()
                    .any(|t| matches!(t, Expr::Name(name) if name.id.as_str() == "__all__"));
                if !is_all {
                    continue;
                }
                assign.value.as_ref()
            }
            Stmt::AnnAssign(assign) => {
                match assign.target.as_ref() {
                    Expr::Name(name) if name.id.as_str() == "__all__" => {}
                    _ => continue,
                }
                assign.value.as_deref()?
            }
            _ => continue,
        };

        // `__all__ = list(_EXPORTS.keys())` — the key set of a lazy-export
        // table, which is still a literal even though it is computed.
        if let Some(names) = keys_from_call(module, value) {
            return Some(names);
        }

        let elements = match value {
            Expr::List(list) => &list.elts,
            Expr::Tuple(tuple) => &tuple.elts,
            _ => continue,
        };

        return Some(
            elements
                .iter()
                .filter_map(|e| match e {
                    Expr::StringLiteral(s) => Some(s.value.to_str().to_string()),
                    _ => None,
                })
                .collect(),
        );
    }
    None
}

/// Find the `from X import name` that binds `local_name`, returning the module
/// (with its relative dots preserved) and the name as it is defined there.
fn find_import(module: &ParsedModule, local_name: &str) -> Option<(String, String)> {
    for stmt in &module.body {
        let Stmt::ImportFrom(import) = stmt else {
            continue;
        };
        for alias in &import.names {
            let bound = alias
                .asname
                .as_ref()
                .map(|a| a.as_str())
                .unwrap_or_else(|| alias.name.as_str());
            if bound != local_name {
                continue;
            }
            let dots = ".".repeat(import.level as usize);
            let module_name = import
                .module
                .as_ref()
                .map(|m| m.as_str())
                .unwrap_or_default();
            return Some((
                format!("{dots}{module_name}"),
                alias.name.as_str().to_string(),
            ));
        }
    }
    None
}

/// Group items by the module they came from, for index generation.
pub fn group_by_module(items: &[ApiItem]) -> HashMap<String, Vec<&ApiItem>> {
    let mut out: HashMap<String, Vec<&ApiItem>> = HashMap::new();
    for item in items {
        let qualname = item.qualname();
        let module = qualname
            .rsplit_once('.')
            .map(|(head, _)| head.to_string())
            .unwrap_or_default();
        out.entry(module).or_default().push(item);
    }
    out
}

/// Read a dict literal `NAME = {"key": value, ...}` from the module.
fn find_dict_literal<'a>(
    module: &'a ParsedModule,
    name: &str,
) -> Option<&'a ruff_python_ast::ExprDict> {
    for stmt in &module.body {
        let Stmt::Assign(assign) = stmt else {
            continue;
        };
        let matches_name = assign
            .targets
            .iter()
            .any(|t| matches!(t, Expr::Name(n) if n.id.as_str() == name));
        if !matches_name {
            continue;
        }
        if let Expr::Dict(dict) = assign.value.as_ref() {
            return Some(dict);
        }
    }
    None
}

fn string_of(expr: &Expr) -> Option<String> {
    match expr {
        Expr::StringLiteral(s) => Some(s.value.to_str().to_string()),
        _ => None,
    }
}

/// Keys of a dict literal, in declaration order.
fn dict_keys(dict: &ruff_python_ast::ExprDict) -> Vec<String> {
    dict.items
        .iter()
        .filter_map(|item| item.key.as_ref().and_then(string_of))
        .collect()
}

/// Resolve a name through a lazy-export table.
///
/// PEP 562 packages commonly replace eager imports with a dict mapping each
/// exported name to `(module, attribute)`, resolved on demand in
/// `__getattr__`. The table is a literal, so it reads statically even though
/// the import it describes never appears as an `import` statement.
fn lazy_export(module: &ParsedModule, local_name: &str) -> Option<(String, String)> {
    for stmt in &module.body {
        let Stmt::Assign(assign) = stmt else {
            continue;
        };
        let Expr::Dict(dict) = assign.value.as_ref() else {
            continue;
        };
        for item in &dict.items {
            let Some(key) = item.key.as_ref().and_then(string_of) else {
                continue;
            };
            if key != local_name {
                continue;
            }
            let Expr::Tuple(target) = &item.value else {
                continue;
            };
            let module_name = target.elts.first().and_then(string_of)?;
            // A `None` attribute means the module itself is exported, which is
            // not a definition we can document.
            let attribute = target.elts.get(1).and_then(string_of)?;
            return Some((module_name, attribute));
        }
    }
    None
}

/// Resolve `list(NAME.keys())`, `list(NAME)`, or `sorted(NAME)` against a dict
/// literal defined in the same module.
fn keys_from_call(module: &ParsedModule, value: &Expr) -> Option<Vec<String>> {
    let Expr::Call(call) = value else {
        return None;
    };
    let Expr::Name(func) = call.func.as_ref() else {
        return None;
    };
    if !matches!(func.id.as_str(), "list" | "sorted" | "tuple") {
        return None;
    }

    let dict_name = match call.arguments.args.first()? {
        Expr::Name(name) => name.id.to_string(),
        // `NAME.keys()`
        Expr::Call(inner) => {
            let Expr::Attribute(attribute) = inner.func.as_ref() else {
                return None;
            };
            if attribute.attr.as_str() != "keys" {
                return None;
            }
            let Expr::Name(name) = attribute.value.as_ref() else {
                return None;
            };
            name.id.to_string()
        }
        _ => return None,
    };

    let mut keys = dict_keys(find_dict_literal(module, &dict_name)?);
    if func.id.as_str() == "sorted" {
        keys.sort();
    }
    Some(keys)
}

/// A package-wide table of module-level string constants, used to fill
/// `{placeholder}` slots in docstrings assembled by `str.format`.
///
/// Projects that share documentation fragments between functions typically
/// keep them as module-level constants and interpolate them in a decorator.
/// Both halves are literals, so the assembled text is recoverable statically —
/// this collects the halves, and [`substitute`] joins them.
pub type Substitutions = HashMap<String, String>;

impl Package {
    /// Scan every module in the package for string constants and for dict
    /// literals mapping a name to one of them.
    pub fn substitutions(&self) -> Substitutions {
        let mut constants: Substitutions = HashMap::new();
        let mut aliases: Vec<(String, String)> = Vec::new();

        for path in python_files(&self.root) {
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(module) = parse_source(source) else {
                continue;
            };

            for stmt in &module.body {
                let Stmt::Assign(assign) = stmt else {
                    continue;
                };

                // `NAME = "..."`
                if let Some(text) = string_of(assign.value.as_ref()) {
                    for target in &assign.targets {
                        if let Expr::Name(name) = target {
                            constants.insert(name.id.to_string(), text.clone());
                        }
                    }
                    continue;
                }

                // `TABLE = {"key": CONSTANT_OR_LITERAL, ...}`
                if let Expr::Dict(dict) = assign.value.as_ref() {
                    for item in &dict.items {
                        let Some(key) = item.key.as_ref().and_then(string_of) else {
                            continue;
                        };
                        match &item.value {
                            Expr::StringLiteral(literal) => {
                                constants.insert(key, literal.value.to_str().to_string());
                            }
                            Expr::Name(name) => aliases.push((key, name.id.to_string())),
                            _ => {}
                        }
                    }
                }
            }
        }

        // Resolve `"key": CONSTANT` entries once every constant is known.
        for (key, constant) in aliases {
            if let Some(text) = constants.get(&constant).cloned() {
                constants.insert(key, text);
            }
        }

        constants
    }
}

/// Every `.py` file under a directory.
fn python_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(python_files(&path));
        } else if path.extension().map(|e| e == "py").unwrap_or(false) {
            out.push(path);
        }
    }
    out
}

/// Replace `{name}` placeholders with known substitutions.
///
/// Unknown placeholders are left alone: a docstring showing a dict literal or
/// an f-string example must not be mangled by a failed lookup.
pub fn substitute(text: &str, substitutions: &Substitutions) -> String {
    if !text.contains('{') {
        return text.to_string();
    }

    let mut out = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];

        let Some(close) = after.find('}') else {
            out.push_str(&rest[open..]);
            return out;
        };

        let key = &after[..close];
        match substitutions.get(key) {
            Some(value) => out.push_str(value),
            None => {
                out.push('{');
                out.push_str(key);
                out.push('}');
            }
        }
        rest = &after[close + 1..];
    }

    out.push_str(rest);
    out
}
