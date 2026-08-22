//! Behavioral tests: what the generator observes about Python source, and what
//! guarantees the generated Typst has to keep. Deliberately no byte-for-byte
//! pins on generated markup — those break on harmless formatting changes.

use std::fs;
use std::path::PathBuf;

use calepin_docs::extract::parse_source;
use calepin_docs::model::ApiItem;
use calepin_docs::typst_escape::{escape_content, render_prose};
use calepin_docs::{generate, Package};

fn parse_one(source: &str) -> ApiItem {
    let module = parse_source(source.to_string()).expect("parses");
    calepin_docs::extract::public_items(&module, "m")
        .into_iter()
        .next()
        .expect("one public item")
}

fn function_of(item: &ApiItem) -> &calepin_docs::model::Function {
    match item {
        ApiItem::Function(f) => f,
        _ => panic!("expected a function"),
    }
}

#[test]
fn signature_restores_positional_and_keyword_markers() {
    let item = parse_one("def f(a, /, b, *, c): ...");
    let signature = function_of(&item).signature();
    assert!(
        signature.contains("a, /"),
        "positional-only marker: {signature}"
    );
    assert!(
        signature.contains("*, c"),
        "keyword-only marker: {signature}"
    );
}

#[test]
fn star_args_suppresses_the_bare_star_marker() {
    // `*args` already opens the keyword-only section; adding `*` would be a
    // syntax error in the rendered signature.
    let item = parse_one("def f(*args, c=1): ...");
    let signature = function_of(&item).signature();
    assert!(signature.contains("*args"));
    assert!(!signature.contains(", *,"), "spurious marker: {signature}");
}

#[test]
fn annotations_render_as_written_not_normalized() {
    let item = parse_one("def f(x: int | None = None) -> list[str]: ...");
    let signature = function_of(&item).signature();
    assert!(signature.contains("int | None"), "{signature}");
    assert!(signature.contains("-> list[str]"), "{signature}");
}

#[test]
fn async_functions_keep_their_modifier() {
    let item = parse_one("async def f(): ...");
    assert!(function_of(&item).signature().starts_with("async f("));
}

#[test]
fn methods_drop_the_implicit_receiver() {
    let module = parse_source("class C:\n    def m(self, x): ...\n".to_string()).unwrap();
    let items = calepin_docs::extract::public_items(&module, "m");
    let ApiItem::Class(class) = &items[0] else {
        panic!("expected a class");
    };
    let signature = class.methods[0].signature();
    assert!(!signature.contains("self"), "receiver leaked: {signature}");
    assert!(signature.contains("m(x)"), "{signature}");
}

#[test]
fn an_annotated_first_parameter_is_not_treated_as_a_receiver() {
    // A module-level function whose first argument happens to be named `self`
    // with an annotation is a real parameter, not a bound receiver.
    let module = parse_source("class C:\n    def m(self: Foo, x): ...\n".to_string()).unwrap();
    let items = calepin_docs::extract::public_items(&module, "m");
    let ApiItem::Class(class) = &items[0] else {
        panic!("expected a class");
    };
    assert!(class.methods[0].signature().contains("self: Foo"));
}

#[test]
fn private_definitions_stay_out_of_the_public_surface() {
    let module = parse_source("def _hidden(): ...\ndef shown(): ...\n".to_string()).unwrap();
    let items = calepin_docs::extract::public_items(&module, "m");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name(), "shown");
}

#[test]
fn dunder_init_is_documented_but_other_dunders_are_not() {
    let module = parse_source(
        "class C:\n    def __init__(self): ...\n    def __repr__(self): ...\n".to_string(),
    )
    .unwrap();
    let items = calepin_docs::extract::public_items(&module, "m");
    let ApiItem::Class(class) = &items[0] else {
        panic!("expected a class");
    };
    let names: Vec<&str> = class.methods.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, vec!["__init__"]);
}

#[test]
fn docstrings_are_dedented_to_their_common_indentation() {
    let module = parse_source(
        "class C:\n    def m(self):\n        \"\"\"One.\n\n        Two.\n        \"\"\"\n"
            .to_string(),
    )
    .unwrap();
    let items = calepin_docs::extract::public_items(&module, "m");
    let ApiItem::Class(class) = &items[0] else {
        panic!("expected a class");
    };
    let docstring = class.methods[0].docstring.as_deref().unwrap();
    assert_eq!(docstring, "One.\n\nTwo.");
}

#[test]
fn typst_metacharacters_in_prose_are_neutralized() {
    // These all carry markup meaning in Typst and appear constantly in Python
    // prose; none may reach the output unescaped.
    for hazard in [
        "*args",
        "_private",
        "$HOME",
        "@dataclass",
        "#chunk",
        "a~b",
        "x[1]",
        "<tag>",
    ] {
        let escaped = escape_content(hazard);
        let stripped = escaped.replace('\\', "");
        assert_eq!(stripped, hazard, "round-trip failed for {hazard}");
        assert!(escaped.contains('\\'), "{hazard} was not escaped");
    }
}

#[test]
fn line_start_markers_are_escaped_but_mid_line_ones_are_not() {
    assert!(escape_content("- item").starts_with("\\-"));
    assert!(escape_content("= heading").starts_with("\\="));
    assert!(escape_content("1. first").starts_with("1\\."));
    // A dash inside a sentence is not a list marker.
    assert_eq!(escape_content("a - b"), "a - b");
}

#[test]
fn code_spans_become_raw_calls_that_need_no_escaping() {
    let rendered = render_prose("use ``df[\"x\"]`` here");
    assert!(rendered.contains("#raw("), "{rendered}");
    // The brackets and quotes live inside a string literal now, so they must
    // not also carry a markup escape.
    assert!(!rendered.contains("\\["), "{rendered}");
}

#[test]
fn an_unterminated_code_span_does_not_swallow_the_rest_of_the_prose() {
    let rendered = render_prose("a `b c");
    assert!(rendered.contains("b c"), "{rendered}");
}

// --- package resolution -------------------------------------------------

struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    fn write(&self, relative: &str, contents: &str) -> &Self {
        let path = self.dir.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
        self
    }

    fn package(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
}

#[test]
fn all_is_followed_through_a_re_export_chain() {
    let fixture = Fixture::new();
    fixture
        .write(
            "pkg/__init__.py",
            "from .core import Thing\n__all__ = [\"Thing\"]\n",
        )
        .write("pkg/core/__init__.py", "from .impl import Thing\n")
        .write("pkg/core/impl.py", "class Thing:\n    \"\"\"Doc.\"\"\"\n");

    let package = Package::open(&fixture.package("pkg")).unwrap();
    let resolution = package.resolve().unwrap();

    assert_eq!(resolution.items.len(), 1);
    assert_eq!(resolution.items[0].qualname(), "pkg.core.impl.Thing");
    assert!(resolution.unresolved.is_empty());
}

#[test]
fn an_import_alias_resolves_to_the_original_definition() {
    let fixture = Fixture::new();
    fixture
        .write(
            "pkg/__init__.py",
            "from .u import real as alias\n__all__ = [\"alias\"]\n",
        )
        .write("pkg/u.py", "def real(): ...\n");

    let resolution = Package::open(&fixture.package("pkg"))
        .unwrap()
        .resolve()
        .unwrap();
    assert_eq!(resolution.items.len(), 1);
    assert_eq!(resolution.items[0].name(), "real");
}

#[test]
fn names_that_cannot_be_traced_are_reported_rather_than_dropped() {
    let fixture = Fixture::new();
    fixture.write("pkg/__init__.py", "__all__ = [\"nowhere\"]\n");

    let resolution = Package::open(&fixture.package("pkg"))
        .unwrap()
        .resolve()
        .unwrap();
    assert!(resolution.items.is_empty());
    assert_eq!(resolution.unresolved.len(), 1);
    assert_eq!(resolution.unresolved[0].name, "nowhere");
}

#[test]
fn a_package_without_all_falls_back_to_its_public_definitions() {
    let fixture = Fixture::new();
    fixture.write("pkg/__init__.py", "def a(): ...\ndef _b(): ...\n");

    let resolution = Package::open(&fixture.package("pkg"))
        .unwrap()
        .resolve()
        .unwrap();
    assert_eq!(resolution.items.len(), 1);
    assert_eq!(resolution.items[0].name(), "a");
}

#[test]
fn a_circular_re_export_terminates_instead_of_hanging() {
    let fixture = Fixture::new();
    fixture
        .write("pkg/__init__.py", "from .a import X\n__all__ = [\"X\"]\n")
        .write("pkg/a.py", "from .b import X\n")
        .write("pkg/b.py", "from .a import X\n");

    let resolution = Package::open(&fixture.package("pkg"))
        .unwrap()
        .resolve()
        .unwrap();
    assert_eq!(resolution.unresolved.len(), 1);
}

// --- generation ---------------------------------------------------------

#[test]
fn generation_writes_a_page_per_definition_plus_an_index() {
    let fixture = Fixture::new();
    fixture
        .write(
            "pkg/__init__.py",
            "from .u import f, C\n__all__ = [\"f\", \"C\"]\n",
        )
        .write("pkg/u.py", "def f(): ...\nclass C: ...\n");

    let out = fixture.dir.path().join("out");
    let package = Package::open(&fixture.package("pkg")).unwrap();
    let report = generate(&package, &out, false).unwrap();

    assert!(out.join("pkg.u.f.typ").exists());
    assert!(out.join("pkg.u.C.typ").exists());
    assert!(out.join("index.typ").exists());
    assert!(report.template_written);
}

#[test]
fn regeneration_preserves_a_customized_template() {
    let fixture = Fixture::new();
    fixture.write("pkg/__init__.py", "def f(): ...\n");
    let out = fixture.dir.path().join("out");
    let package = Package::open(&fixture.package("pkg")).unwrap();

    generate(&package, &out, false).unwrap();
    fs::write(out.join("api.typ"), "// my styling\n").unwrap();
    let second = generate(&package, &out, false).unwrap();

    assert!(!second.template_written);
    assert_eq!(
        fs::read_to_string(out.join("api.typ")).unwrap(),
        "// my styling\n"
    );
}

#[test]
fn a_syntactically_invalid_module_fails_loudly() {
    let fixture = Fixture::new();
    fixture.write("pkg/__init__.py", "def f(:\n");
    let package = Package::open(&fixture.package("pkg")).unwrap();
    assert!(package.resolve().is_err());
}

#[test]
fn a_lazy_export_table_resolves_without_any_import_statement() {
    // PEP 562 packages map each export to `(module, attribute)` and resolve it
    // in `__getattr__`, so no `import` statement ever names the definition.
    let fixture = Fixture::new();
    fixture
        .write(
            "pkg/__init__.py",
            "_EXPORTS = {\n    \"thing\": (\"pkg.impl\", \"thing\"),\n}\n\
             __all__ = list(_EXPORTS.keys())\n",
        )
        .write("pkg/impl.py", "def thing(): ...\n");

    let resolution = Package::open(&fixture.package("pkg"))
        .unwrap()
        .resolve()
        .unwrap();
    assert_eq!(resolution.items.len(), 1);
    assert_eq!(resolution.items[0].qualname(), "pkg.impl.thing");
}

#[test]
fn a_lazy_export_may_rename_the_definition_it_points_at() {
    let fixture = Fixture::new();
    fixture
        .write(
            "pkg/__init__.py",
            "_E = {\"public\": (\"pkg.impl\", \"internal\")}\n__all__ = list(_E)\n",
        )
        .write("pkg/impl.py", "def internal(): ...\n");

    let resolution = Package::open(&fixture.package("pkg"))
        .unwrap()
        .resolve()
        .unwrap();
    assert_eq!(resolution.items[0].name(), "internal");
}

#[test]
fn a_lazy_export_naming_a_whole_module_is_reported_not_guessed() {
    // `(module, None)` re-exports the module itself, which is not a definition.
    let fixture = Fixture::new();
    fixture
        .write(
            "pkg/__init__.py",
            "_E = {\"sub\": (\"pkg.sub\", None)}\n__all__ = list(_E.keys())\n",
        )
        .write("pkg/sub.py", "def f(): ...\n");

    let resolution = Package::open(&fixture.package("pkg"))
        .unwrap()
        .resolve()
        .unwrap();
    assert!(resolution.items.is_empty());
    assert_eq!(resolution.unresolved.len(), 1);
}

// --- docstrings the definition does not literally carry ------------------

#[test]
fn a_decorator_supplied_docstring_is_used_when_the_body_has_none() {
    let module =
        parse_source("@doc(\"\"\"Set from a decorator.\"\"\")\ndef f(): ...\n".to_string())
            .unwrap();
    let items = calepin_docs::extract::public_items(&module, "m");
    assert_eq!(
        function_of(&items[0]).docstring.as_deref(),
        Some("Set from a decorator.")
    );
}

#[test]
fn a_literal_docstring_wins_over_a_decorator_one() {
    let module = parse_source(
        "@doc(\"From decorator.\")\ndef f():\n    \"\"\"From body.\"\"\"\n".to_string(),
    )
    .unwrap();
    let items = calepin_docs::extract::public_items(&module, "m");
    assert_eq!(
        function_of(&items[0]).docstring.as_deref(),
        Some("From body.")
    );
}

#[test]
fn a_decorator_consumed_as_documentation_is_not_also_listed() {
    // Rendering the whole docstring a second time as a decorator is noise.
    let module = parse_source("@doc(\"Text.\")\ndef f(): ...\n".to_string()).unwrap();
    let items = calepin_docs::extract::public_items(&module, "m");
    assert!(function_of(&items[0]).decorators.is_empty());
}

#[test]
fn decorators_that_are_not_documentation_are_still_listed() {
    let module = parse_source("@property\ndef f(): ...\n".to_string()).unwrap();
    let items = calepin_docs::extract::public_items(&module, "m");
    assert_eq!(function_of(&items[0]).decorators, vec!["property"]);
}

#[test]
fn a_doc_alias_borrows_the_docstring_of_the_function_it_wraps() {
    let module = parse_source(
        "def a():\n    \"\"\"Original.\"\"\"\ndef b(): ...\nb.__doc__ = a.__doc__\n".to_string(),
    )
    .unwrap();
    let items = calepin_docs::extract::public_items(&module, "m");
    let b = items.iter().find(|i| i.name() == "b").unwrap();
    assert_eq!(function_of(b).docstring.as_deref(), Some("Original."));
}

#[test]
fn a_self_referential_doc_alias_does_not_recurse_forever() {
    let module = parse_source("def a(): ...\na.__doc__ = a.__doc__\n".to_string()).unwrap();
    let items = calepin_docs::extract::public_items(&module, "m");
    assert!(function_of(&items[0]).docstring.is_none());
}

#[test]
fn shared_docstring_fragments_are_interpolated_from_module_constants() {
    let fixture = Fixture::new();
    fixture
        .write("pkg/__init__.py", "from .u import f\n__all__ = [\"f\"]\n")
        .write(
            "pkg/shared.py",
            "PARAM_X = \"x: the input value\"\nSHARED = {\"param_x\": PARAM_X}\n",
        )
        .write(
            "pkg/u.py",
            "@doc(\"\"\"Summary.\n\n{param_x}\"\"\")\ndef f(): ...\n",
        );

    let resolution = Package::open(&fixture.package("pkg"))
        .unwrap()
        .resolve()
        .unwrap();
    let docstring = match &resolution.items[0] {
        ApiItem::Function(f) => f.docstring.clone().unwrap(),
        _ => panic!("expected a function"),
    };
    assert!(docstring.contains("x: the input value"), "{docstring}");
    assert!(!docstring.contains("{param_x}"), "{docstring}");
}

#[test]
fn an_unknown_placeholder_is_left_intact_rather_than_blanked() {
    // Docstrings show dict literals and format strings; a failed lookup must
    // not eat them.
    let substitutions = std::collections::HashMap::new();
    let text = "example: {\"a\": 1} and {unknown}";
    assert_eq!(
        calepin_docs::resolve::substitute(text, &substitutions),
        text
    );
}

// --- markdown rendering --------------------------------------------------

#[test]
fn markdown_headings_and_lists_render_as_typst_constructs() {
    use calepin_docs::markdown::{looks_like_markdown, render_markdown};

    let source = "## Parameters\n\n- first\n- second\n";
    assert!(looks_like_markdown(source));

    let rendered = render_markdown(source);
    assert!(rendered.contains("#heading(level:"), "{rendered}");
    assert!(rendered.contains("#list("), "{rendered}");
    // The markers themselves must not survive as literal text.
    assert!(!rendered.contains("\\#\\#"), "{rendered}");
}

#[test]
fn nested_bullets_nest_rather_than_flatten() {
    use calepin_docs::markdown::render_markdown;

    let rendered = render_markdown("- outer\n    - inner\n");
    // A nested list appears inside the outer item's content block.
    let outer = rendered.find("#list(").unwrap();
    let inner = rendered.rfind("#list(").unwrap();
    assert!(inner > outer, "expected a nested list: {rendered}");
}

#[test]
fn fenced_code_becomes_a_raw_block_with_its_language() {
    use calepin_docs::markdown::render_markdown;

    let rendered = render_markdown("```py\nx = 1\n```\n");
    assert!(rendered.contains("block: true"), "{rendered}");
    // `py` is normalized to the name Typst knows.
    assert!(rendered.contains("lang: \"python\""), "{rendered}");
    assert!(rendered.contains("x = 1"), "{rendered}");
}

#[test]
fn prose_without_markdown_constructs_is_left_to_the_plain_escaper() {
    use calepin_docs::markdown::looks_like_markdown;
    assert!(!looks_like_markdown("Just a sentence about *args and _x."));
}
