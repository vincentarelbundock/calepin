//! Generates `docs-src/reference/generated.md`: a fragment of hard facts
//! (reserved input keys, config keys, the results schema version, engine
//! names, theme entry files) pulled from the actual Rust source rather than
//! retyped by hand in CLAUDE.md or the docs. `CLAUDE.md` and the docs site
//! reference the checked-in fragment; the `#[cfg(test)]` test below fails
//! when it drifts from source and regenerates it when `CALEPIN_UPDATE_DOCS=1`
//! is set.
//!
//! This module only exists for that test: it is not wired into any CLI
//! surface.

#![cfg(test)]

use crate::config::ExecutablePaths;
use crate::engines::diagram::is_known_diagram_engine_name;
use crate::typst::model::{EngineName, RESULT_SCHEMA_VERSION};

const RUN_RS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/typst/run.rs"));
const CONFIG_RS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/config.rs"));
const THEME_MOD_RS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/theme/mod.rs"));

const GENERATED_FRAGMENT_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../docs-src/reference/generated.md"
);

/// Diagram engine names asserted below against `is_known_diagram_engine_name`
/// and against `EngineName::from_name`. If a diagram engine is added or
/// renamed in `engines/diagram.rs`, update this list too: the assertions
/// only catch a name going stale, not one going undocumented.
const DIAGRAM_ENGINE_NAMES: &[&str] = &["mermaid", "dot", "tikz", "d2"];

/// Pull `pub const NAME: &str = "value";` definitions out of `source` into a
/// lookup table, so an identifier array (like `RESERVED_INPUT_KEYS`) can be
/// resolved to the literal strings it names.
fn str_const_table(source: &str) -> std::collections::HashMap<&str, &str> {
    let mut table = std::collections::HashMap::new();
    for line in source.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("pub const ") else {
            continue;
        };
        let Some(colon) = rest.find(':') else {
            continue;
        };
        let name = rest[..colon].trim();
        let Some(eq) = rest.find('=') else { continue };
        let after_eq = rest[eq + 1..].trim().trim_end_matches(';');
        if let Some(value) = after_eq.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
            table.insert(name, value);
        }
    }
    table
}

/// Extract the comma-separated identifiers or string literals inside the
/// first `[ ... ]` array literal that starts after `anchor` in `source`.
fn items_in_next_bracket(source: &str, anchor: &str) -> Vec<String> {
    let anchor_at = source
        .find(anchor)
        .unwrap_or_else(|| panic!("anchor {anchor:?} not found in source"));
    let after = &source[anchor_at..];
    // Skip past the first `=` so a `&[&str]` type annotation before the
    // value does not get mistaken for the array literal itself.
    let eq = after.find('=').expect("no `=` after anchor");
    let after_eq = &after[eq..];
    let open = after_eq.find('[').expect("no `[` after `=`");
    let close = after_eq[open..].find(']').expect("no `]` after `[`");
    let body = &after_eq[open + 1..open + close];
    body.split(',')
        .map(|s| s.trim().trim_matches('"'))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// The ten reserved `--input` keys, resolved from `typst/run.rs`.
fn reserved_input_keys() -> Vec<String> {
    let consts = str_const_table(RUN_RS);
    items_in_next_bracket(RUN_RS, "pub const RESERVED_INPUT_KEYS")
        .into_iter()
        .map(|ident| {
            consts
                .get(ident.as_str())
                .unwrap_or_else(|| panic!("no `pub const {ident}: &str = ...` found"))
                .to_string()
        })
        .collect()
}

/// The `[executables]` keys, from `ExecutablePaths`'s `Debug` field order
/// (`config.rs`). `Debug` is derived, so a renamed, added, or removed field
/// changes this list without any change here.
fn executables_keys() -> Vec<String> {
    let pretty = format!("{:#?}", ExecutablePaths::defaults());
    pretty
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let colon = line.find(':')?;
            let key = &line[..colon];
            (!key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
                .then(|| key.to_string())
        })
        .collect()
}

/// The other top-level `.calepin/config.toml` keys (everything in
/// `RawCalepinConfig` besides `executables`, which `executables_keys` above
/// covers on its own), from the struct body in `config.rs`.
fn other_config_keys() -> Vec<String> {
    let marker = "struct RawCalepinConfig {";
    let start = CONFIG_RS
        .find(marker)
        .expect("RawCalepinConfig struct not found")
        + marker.len();
    let end = CONFIG_RS[start..]
        .find("\n}")
        .expect("RawCalepinConfig struct has no closing brace")
        + start;
    let body = &CONFIG_RS[start..end];

    let mut keys = Vec::new();
    let mut pending_rename: Option<String> = None;
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("#[serde(rename = \"") {
            let value = rest.split('"').next().expect("malformed rename attribute");
            pending_rename = Some(value.to_string());
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let field = line
            .split(':')
            .next()
            .expect("field line has no name")
            .trim();
        let key = pending_rename
            .take()
            .unwrap_or_else(|| field.replace('_', "-"));
        if field != "executables" {
            keys.push(key);
        }
    }
    keys
}

/// The theme bundle entry files accepted by `validate_theme_dir`
/// (`theme/mod.rs`).
fn theme_entry_files() -> Vec<String> {
    items_in_next_bracket(THEME_MOD_RS, "fn validate_theme_dir")
}

fn render_fragment() -> String {
    // Type-checked facts that do not need text scanning: these fail to
    // compile, rather than fail at test time, if the underlying constant or
    // enum variant disappears.
    assert_eq!(EngineName::from_name("r"), EngineName::R);
    assert_eq!(EngineName::from_name("python"), EngineName::Python);
    for name in DIAGRAM_ENGINE_NAMES {
        assert!(
            is_known_diagram_engine_name(name),
            "{name} is listed as a diagram engine but is_known_diagram_engine_name disagrees"
        );
        assert_eq!(
            EngineName::from_name(name),
            EngineName::Diagram((*name).to_string())
        );
    }
    assert!(!is_known_diagram_engine_name("julia"));
    assert!(!is_known_diagram_engine_name("sh"));
    // `julia` and `sh` ride the Jupyter bridge with no special-casing, and
    // `bash` is a different kernel request than `sh`, not an alias for it.
    assert_eq!(
        EngineName::from_name("julia"),
        EngineName::Jupyter("julia".to_string())
    );
    assert_eq!(
        EngineName::from_name("sh"),
        EngineName::Jupyter("sh".to_string())
    );
    assert_ne!(EngineName::from_name("bash"), EngineName::from_name("sh"));

    let reserved = reserved_input_keys();
    let executables = executables_keys();
    let other_config = other_config_keys();
    let theme_entries = theme_entry_files();

    let mut out = String::new();
    out.push_str("<!-- Generated by `cargo test docs_fragment --manifest-path calepin/Cargo.toml`. Do not edit by hand; edit the source it reads and rerun with CALEPIN_UPDATE_DOCS=1. -->\n\n");
    out.push_str("# Generated facts\n\n");

    out.push_str("## Results schema version\n\n");
    out.push_str(&format!("`{RESULT_SCHEMA_VERSION}`\n\n"));

    out.push_str("## Reserved `--input` keys\n\n");
    for key in &reserved {
        out.push_str(&format!("- `{key}`\n"));
    }
    out.push('\n');

    out.push_str("## `[executables]` keys\n\n");
    for key in &executables {
        out.push_str(&format!("- `{key}`\n"));
    }
    out.push('\n');

    out.push_str("## Other `.calepin/config.toml` keys\n\n");
    for key in &other_config {
        out.push_str(&format!("- `{key}`\n"));
    }
    out.push('\n');

    out.push_str("## Supported engine names\n\n");
    out.push_str("- `r`, `python` (native engines)\n");
    for name in DIAGRAM_ENGINE_NAMES {
        out.push_str(&format!("- `{name}` (diagram engine, always emits SVG)\n"));
    }
    out.push_str(
        "- any other name (for example `julia`, `sh`) rides the Jupyter bridge; there is no `bash` alias for `sh`, and kernel names must match an installed Jupyter kernel\n\n",
    );

    out.push_str("## Theme entry files\n\n");
    for entry in &theme_entries {
        out.push_str(&format!("- `{entry}`\n"));
    }
    out.push('\n');

    out
}

#[test]
fn generated_docs_fragment_matches_source() {
    let expected = render_fragment();
    let on_disk = std::fs::read_to_string(GENERATED_FRAGMENT_PATH).unwrap_or_default();
    if on_disk == expected {
        return;
    }
    if std::env::var_os("CALEPIN_UPDATE_DOCS").is_some() {
        std::fs::write(GENERATED_FRAGMENT_PATH, &expected)
            .expect("failed to write docs-src/reference/generated.md");
        return;
    }
    panic!(
        "docs-src/reference/generated.md is stale. Rerun with \
         CALEPIN_UPDATE_DOCS=1 cargo test generated_docs_fragment_matches_source \
         --manifest-path calepin/Cargo.toml to regenerate it."
    );
}
