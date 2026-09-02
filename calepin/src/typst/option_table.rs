// Single source of truth for the `calepin.chunk()` / `calepin.setup()` option
// vocabulary shared between Rust and the embedded Typst runtime.
//
// Several items here (`render_base_options_typst`, `is_bare_typst_identifier`,
// `BASE_OPTIONS_MARKER`, and `OptionSpec::side`) are only read by `build.rs`
// (a separate compilation of this same file) and by this module's own tests,
// so a plain `cargo build` of the crate sees them as unused.
#![allow(dead_code)]
//
// This file is included two different ways:
// - as an ordinary module inside the crate (`chunk_options.rs` builds its
//   native option list and aliases from `OPTION_TABLE`);
// - textually by `build.rs`, which generates the Typst `_base-options`
//   dictionary in `notebook/defaults.typ` from the same table.
//
// Because `build.rs` includes it before the crate is compiled, this file must
// not depend on anything outside `std`.

/// One entry in the chunk option vocabulary.
#[derive(Debug, Clone, Copy)]
pub struct OptionSpec {
    /// The option's name as written in Typst and in `results.json` (kebab-case).
    pub name: &'static str,
    /// A literal Typst expression used as this option's default in the
    /// generated `_base-options` dictionary. Must be valid Typst syntax.
    pub typst_default: &'static str,
    /// Which side of the pipeline actually reads this option.
    pub side: OptionSide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionSide {
    /// Consulted by the Typst runtime when rendering (echo, results mode,
    /// figure/table layout, captions, ...).
    Display,
    /// Consulted only by Rust when a chunk executes (eval, error tolerance,
    /// the figure device settings).
    Exec,
    /// Consulted by both sides (for example `fenced-chunks`, which gates
    /// whether the runtime treats a fenced block as a chunk at all, and is
    /// also validated by Rust).
    Both,
}

/// The full chunk option vocabulary, in the order the generated
/// `_base-options` dictionary should list them.
///
/// `store-get` and `store-set` are not chunk *options* here: they are
/// call-only arguments with no document-wide default, so they stay out of
/// `_base-options` and are added separately wherever the native argument
/// list is built.
pub const OPTION_TABLE: &[OptionSpec] = &[
    OptionSpec {
        name: "script",
        typst_default: "true",
        side: OptionSide::Exec,
    },
    OptionSpec {
        name: "echo",
        typst_default: "true",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "eval",
        typst_default: "true",
        side: OptionSide::Exec,
    },
    OptionSpec {
        name: "results",
        typst_default: "\"render\"",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "results-location",
        typst_default: "\"statement\"",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "warning",
        typst_default: "true",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "message",
        typst_default: "true",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "error",
        typst_default: "false",
        side: OptionSide::Exec,
    },
    OptionSpec {
        name: "fig-device-format",
        typst_default: "\"svg\"",
        side: OptionSide::Exec,
    },
    OptionSpec {
        name: "fig-device-dpi",
        typst_default: "150",
        side: OptionSide::Exec,
    },
    OptionSpec {
        name: "fig-device-width",
        typst_default: "6",
        side: OptionSide::Exec,
    },
    OptionSpec {
        name: "fig-device-height",
        typst_default: "auto",
        side: OptionSide::Exec,
    },
    OptionSpec {
        name: "fig-device-aspect",
        typst_default: "0.618",
        side: OptionSide::Exec,
    },
    OptionSpec {
        name: "fig-width",
        typst_default: "70%",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "fig-height",
        typst_default: "auto",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "fig-align",
        typst_default: "center",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "fig-responsive",
        typst_default: "true",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "fig-link",
        typst_default: "auto",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "fig-caption",
        typst_default: "none",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "fig-cap-location",
        typst_default: "auto",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "fig-alt-text",
        typst_default: "none",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "fig-subcaptions",
        typst_default: "none",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "fig-layout-columns",
        typst_default: "auto",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "fig-layout-rows",
        typst_default: "auto",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "tbl-caption",
        typst_default: "none",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "lst-caption",
        typst_default: "none",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "kind",
        typst_default: "auto",
        side: OptionSide::Display,
    },
    OptionSpec {
        name: "fenced-chunks",
        typst_default: "true",
        side: OptionSide::Both,
    },
];

/// Marker line in `notebook/defaults.typ` that `build.rs` replaces with the
/// generated `_base-options` dictionary.
pub const BASE_OPTIONS_MARKER: &str = "// calepin:generated-base-options";

/// Renders `OPTION_TABLE` as a Typst dictionary literal assigned to
/// `_base-options`.
pub fn render_base_options_typst() -> String {
    let mut out = String::from("#let _base-options = (\n");
    for spec in OPTION_TABLE {
        out.push_str("  ");
        if is_bare_typst_identifier(spec.name) {
            out.push_str(spec.name);
        } else {
            out.push('"');
            out.push_str(spec.name);
            out.push('"');
        }
        out.push_str(": ");
        out.push_str(spec.typst_default);
        out.push_str(",\n");
    }
    out.push_str(")\n");
    out
}

fn is_bare_typst_identifier(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && !name.as_bytes()[0].is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_options_dictionary_lists_every_table_entry() {
        let generated = render_base_options_typst();
        for spec in OPTION_TABLE {
            let needle = if is_bare_typst_identifier(spec.name) {
                format!("{}: {}", spec.name, spec.typst_default)
            } else {
                format!("\"{}\": {}", spec.name, spec.typst_default)
            };
            assert!(
                generated.contains(&needle),
                "generated `_base-options` is missing `{needle}`:\n{generated}"
            );
        }
    }

    // `notebook/options.typ` cannot read the table itself (it is plain Typst,
    // not generated), so `setup()` still hand-lists every option as a named
    // parameter with a `_base-options.at(...)` default. This test is the
    // "single source of truth" guarantee for that file: it fails whenever
    // `OPTION_TABLE` gains or loses an option that `setup()` does not.
    #[test]
    fn options_typ_setup_lists_every_table_option_as_a_parameter() {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let path = std::path::Path::new(&manifest_dir)
            .join("src/assets/typst-runtime/notebook/options.typ");
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));

        for spec in OPTION_TABLE {
            if spec.name == "fenced-chunks" {
                // `setup()` accepts `fenced-chunks` too, but with a plain
                // `true` default rather than one read from `_base-options`
                // (fenced-chunks is document-wide only and has no per-chunk
                // `auto` sense), so it is checked separately below.
                assert!(
                    source.contains("fenced-chunks: true,"),
                    "setup() is missing the `fenced-chunks` parameter"
                );
                continue;
            }
            let param = format!("{}: _base-options.at(\"{}\"),", spec.name, spec.name);
            assert!(
                source.contains(&param),
                "setup() is missing the parameter `{param}` (add it, or drop `{}` from \
                 OPTION_TABLE if the runtime should no longer accept it)",
                spec.name
            );
        }
    }
}
