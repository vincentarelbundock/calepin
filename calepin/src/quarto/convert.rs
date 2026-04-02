//! Convert serde_yaml::Value tree into calepin's Value/Table.

use crate::value::{Table, Value};

/// Recursively convert a serde_yaml::Value into calepin's Value.
pub fn from_yaml(yv: serde_yaml::Value) -> Value {
    match yv {
        serde_yaml::Value::Null => Value::Null,
        serde_yaml::Value::Bool(b) => Value::Bool(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                // Fallback: render as string
                Value::String(n.to_string())
            }
        }
        serde_yaml::Value::String(s) => Value::String(s),
        serde_yaml::Value::Sequence(seq) => {
            Value::Array(seq.into_iter().map(from_yaml).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let table: Table = map
                .into_iter()
                .filter_map(|(k, v)| {
                    let key = yaml_key_to_string(k)?;
                    Some((key, from_yaml(v)))
                })
                .collect();
            Value::Table(table)
        }
        serde_yaml::Value::Tagged(tagged) => from_yaml(tagged.value),
    }
}

/// Convert a YAML mapping key to a string. Non-string keys are dropped.
fn yaml_key_to_string(k: serde_yaml::Value) -> Option<String> {
    match k {
        serde_yaml::Value::String(s) => Some(s),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_types() {
        assert!(matches!(from_yaml(serde_yaml::Value::Bool(true)), Value::Bool(true)));
        assert!(matches!(from_yaml(serde_yaml::Value::String("hi".into())), Value::String(_)));
        assert!(matches!(from_yaml(serde_yaml::Value::Null), Value::Null));
    }

    #[test]
    fn test_number_types() {
        let int_val: serde_yaml::Value = serde_yaml::from_str("42").unwrap();
        let float_val: serde_yaml::Value = serde_yaml::from_str("3.14").unwrap();
        assert!(matches!(from_yaml(int_val), Value::Integer(42)));
        match from_yaml(float_val) {
            Value::Float(f) => assert!((f - 3.14).abs() < 0.001),
            _ => panic!("expected Float"),
        }
    }

    #[test]
    fn test_sequence() {
        let yaml: serde_yaml::Value = serde_yaml::from_str("[1, 2, 3]").unwrap();
        match from_yaml(yaml) {
            Value::Array(arr) => assert_eq!(arr.len(), 3),
            _ => panic!("expected Array"),
        }
    }

    #[test]
    fn test_mapping() {
        let yaml: serde_yaml::Value = serde_yaml::from_str("a: 1\nb: hello").unwrap();
        match from_yaml(yaml) {
            Value::Table(t) => {
                assert_eq!(t.len(), 2);
                assert!(matches!(t.get("a"), Some(Value::Integer(1))));
                assert!(matches!(t.get("b"), Some(Value::String(_))));
            }
            _ => panic!("expected Table"),
        }
    }

    #[test]
    fn test_nested_mapping() {
        let yaml: serde_yaml::Value = serde_yaml::from_str("a:\n  b: 1\n  c: hello").unwrap();
        match from_yaml(yaml) {
            Value::Table(t) => {
                let inner = t.get("a").unwrap().as_table().unwrap();
                assert_eq!(inner.len(), 2);
                assert!(matches!(inner.get("b"), Some(Value::Integer(1))));
            }
            _ => panic!("expected Table"),
        }
    }

    #[test]
    fn test_empty_mapping() {
        let yaml: serde_yaml::Value = serde_yaml::from_str("{}").unwrap();
        match from_yaml(yaml) {
            Value::Table(t) => assert!(t.is_empty()),
            _ => panic!("expected Table"),
        }
    }

    #[test]
    fn test_empty_sequence() {
        let yaml: serde_yaml::Value = serde_yaml::from_str("[]").unwrap();
        match from_yaml(yaml) {
            Value::Array(a) => assert!(a.is_empty()),
            _ => panic!("expected Array"),
        }
    }

    #[test]
    fn test_yaml_yes_no_are_strings() {
        // serde_yaml uses YAML 1.2 where yes/no/on/off are strings, not bools.
        // Only true/false are boolean in YAML 1.2.
        let yes: serde_yaml::Value = serde_yaml::from_str("yes").unwrap();
        let no: serde_yaml::Value = serde_yaml::from_str("no").unwrap();
        assert!(matches!(from_yaml(yes), Value::String(_)));
        assert!(matches!(from_yaml(no), Value::String(_)));
    }

    #[test]
    fn test_yaml_on_off_are_strings() {
        let on: serde_yaml::Value = serde_yaml::from_str("on").unwrap();
        let off: serde_yaml::Value = serde_yaml::from_str("off").unwrap();
        assert!(matches!(from_yaml(on), Value::String(_)));
        assert!(matches!(from_yaml(off), Value::String(_)));
    }

    #[test]
    fn test_numeric_mapping_key() {
        let yaml: serde_yaml::Value = serde_yaml::from_str("42: value").unwrap();
        match from_yaml(yaml) {
            Value::Table(t) => {
                assert!(t.get("42").is_some());
            }
            _ => panic!("expected Table"),
        }
    }

    #[test]
    fn test_bool_mapping_key() {
        let yaml: serde_yaml::Value = serde_yaml::from_str("true: value").unwrap();
        match from_yaml(yaml) {
            Value::Table(t) => {
                assert!(t.get("true").is_some());
            }
            _ => panic!("expected Table"),
        }
    }

    #[test]
    fn test_unquoted_float() {
        let yaml: serde_yaml::Value = serde_yaml::from_str("3.14").unwrap();
        match from_yaml(yaml) {
            Value::Float(f) => assert!((f - 3.14).abs() < 0.001),
            _ => panic!("expected Float"),
        }
    }
}
