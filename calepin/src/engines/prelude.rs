// Generate language-native literal bindings for document-level variables.
//
// Calepin turns the variables declared in `calepin.setup(vars: (...))`, merged
// with `[vars]` config and CLI `--set vars.*` overrides, into a small prelude that is
// evaluated once, at engine startup, so user code can read a `vars` value
// without any string interpolation. The bindings are emitted as real source in
// each language (no JSON parser dependency on the runtime side), from the
// already-validated `serde_json::Value` Calepin holds.
//
// Only the v1 leaf types reach here (none, bool, int, float, str, array,
// dictionary); unsupported Typst values are rejected earlier, on the Typst side.

use serde_json::Value;

/// R prelude: `vars <- list("alpha" = 0.1, ...)`.
pub fn r_prelude(name: &str, vars: &Value) -> String {
    format!("{name} <- {}", r_value(vars))
}

/// Python prelude: `vars = {"alpha": 0.1, ...}`.
pub fn python_prelude(name: &str, vars: &Value) -> String {
    format!("{name} = {}", python_value(vars))
}

fn r_value(value: &Value) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::Bool(true) => "TRUE".to_string(),
        Value::Bool(false) => "FALSE".to_string(),
        Value::Number(_) => r_number(value),
        Value::String(text) => escape_string(text),
        Value::Array(items) => r_array(items),
        Value::Object(map) => {
            let fields: Vec<String> = map
                .iter()
                .map(|(key, val)| format!("{} = {}", escape_string(key), r_value(val)))
                .collect();
            format!("list({})", fields.join(", "))
        }
    }
}

fn r_number(value: &Value) -> String {
    if let Some(int) = value.as_i64() {
        // R integers are 32-bit; an `L` suffix outside that range coerces to NA
        // with a warning, so keep those as doubles.
        if i32::try_from(int).is_ok() {
            return format!("{int}L");
        }
        return int.to_string();
    }
    if let Some(uint) = value.as_u64() {
        return uint.to_string();
    }
    value.to_string()
}

/// JSON arrays become R vectors when every element is a non-null scalar of the
/// same kind (so `vars$years` is a numeric vector); otherwise a list, which is
/// the only R container that can hold nulls, nested values, or mixed types.
fn r_array(items: &[Value]) -> String {
    let rendered: Vec<String> = items.iter().map(r_value).collect();
    if !items.is_empty() && r_array_is_vector(items) {
        format!("c({})", rendered.join(", "))
    } else {
        format!("list({})", rendered.join(", "))
    }
}

fn r_array_is_vector(items: &[Value]) -> bool {
    fn kind(value: &Value) -> Option<u8> {
        match value {
            Value::Bool(_) => Some(0),
            Value::Number(_) => Some(1),
            Value::String(_) => Some(2),
            _ => None,
        }
    }
    let mut kinds = items.iter().map(kind);
    match kinds.next().flatten() {
        None => false,
        Some(first) => kinds.all(|k| k == Some(first)),
    }
}

fn python_value(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => escape_string(text),
        Value::Array(items) => {
            let rendered: Vec<String> = items.iter().map(python_value).collect();
            format!("[{}]", rendered.join(", "))
        }
        Value::Object(map) => {
            let fields: Vec<String> = map
                .iter()
                .map(|(key, val)| format!("{}: {}", escape_string(key), python_value(val)))
                .collect();
            format!("{{{}}}", fields.join(", "))
        }
    }
}

/// Escape into a double-quoted literal valid in both R and Python.
fn escape_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // R ----------------------------------------------------------------------

    #[test]
    fn r_prelude_binds_named_list() {
        let vars = json!({ "alpha": 0.1, "label": "baseline" });
        let out = r_prelude("vars", &vars);
        assert_eq!(
            out,
            r#"vars <- list("alpha" = 0.1, "label" = "baseline")"#
        );
    }

    #[test]
    fn r_integers_get_l_suffix_and_floats_do_not() {
        assert_eq!(r_value(&json!(25)), "25L");
        assert_eq!(r_value(&json!(0.1)), "0.1");
    }

    #[test]
    fn r_large_integers_drop_l_suffix_to_avoid_na_coercion() {
        // Above .Machine$integer.max; `L` would coerce to NA with a warning.
        assert_eq!(r_value(&json!(3_000_000_000i64)), "3000000000");
    }

    #[test]
    fn r_booleans_and_null() {
        assert_eq!(r_value(&json!(true)), "TRUE");
        assert_eq!(r_value(&json!(false)), "FALSE");
        assert_eq!(r_value(&Value::Null), "NULL");
    }

    #[test]
    fn r_strings_are_escaped() {
        assert_eq!(
            r_value(&json!("he said \"hi\"\n\tdone\\")),
            r#""he said \"hi\"\n\tdone\\""#
        );
    }

    #[test]
    fn r_homogeneous_scalar_arrays_become_vectors() {
        assert_eq!(
            r_value(&json!([2020, 2021, 2022])),
            "c(2020L, 2021L, 2022L)"
        );
        assert_eq!(r_value(&json!(["a", "b"])), r#"c("a", "b")"#);
        assert_eq!(r_value(&json!([true, false])), "c(TRUE, FALSE)");
    }

    #[test]
    fn r_heterogeneous_or_null_bearing_arrays_become_lists() {
        assert_eq!(r_value(&json!([1, "a"])), r#"list(1L, "a")"#);
        assert_eq!(r_value(&json!([1, null])), "list(1L, NULL)");
    }

    #[test]
    fn r_nested_arrays_and_objects() {
        assert_eq!(r_value(&json!([[1, 2], [3]])), "list(c(1L, 2L), c(3L))");
        assert_eq!(
            r_value(&json!({ "outer": { "inner": 1 } })),
            r#"list("outer" = list("inner" = 1L))"#
        );
    }

    #[test]
    fn r_empty_array_is_empty_list() {
        assert_eq!(r_value(&json!([])), "list()");
    }

    #[test]
    fn r_none_inside_object_is_kept_as_null() {
        // The list() constructor keeps NULL elements (unlike `x$a <- NULL`).
        assert_eq!(r_value(&json!({ "a": null })), r#"list("a" = NULL)"#);
    }

    // Python -----------------------------------------------------------------

    #[test]
    fn python_prelude_binds_dict() {
        // Object keys are emitted in serde_json's deterministic (sorted) order;
        // order is irrelevant for a dict accessed by key.
        let vars = json!({ "alpha": 0.1, "active": true });
        let out = python_prelude("vars", &vars);
        assert_eq!(out, r#"vars = {"active": True, "alpha": 0.1}"#);
    }

    #[test]
    fn python_scalars() {
        assert_eq!(python_value(&json!(25)), "25");
        assert_eq!(python_value(&json!(0.1)), "0.1");
        assert_eq!(python_value(&json!(true)), "True");
        assert_eq!(python_value(&json!(false)), "False");
        assert_eq!(python_value(&Value::Null), "None");
    }

    #[test]
    fn python_strings_are_escaped() {
        assert_eq!(
            python_value(&json!("he said \"hi\"\n\tdone\\")),
            r#""he said \"hi\"\n\tdone\\""#
        );
    }

    #[test]
    fn python_arrays_and_nested_objects() {
        assert_eq!(python_value(&json!([2020, 2021])), "[2020, 2021]");
        assert_eq!(
            python_value(&json!({ "years": [1, 2], "meta": { "k": "v" } })),
            r#"{"meta": {"k": "v"}, "years": [1, 2]}"#
        );
    }
}
