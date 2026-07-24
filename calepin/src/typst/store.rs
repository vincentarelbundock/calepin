use anyhow::{anyhow, Result};
use serde_json::{Map, Value};
use std::collections::HashSet;

pub const MAX_KEY_BYTES: usize = 256;
pub const MAX_KEYS: usize = 1_024;
pub const MAX_DEPTH: usize = 64;
pub const MAX_VALUE_BYTES: usize = 1024 * 1024;
pub const MAX_STORE_BYTES: usize = 8 * 1024 * 1024;

pub type Store = Map<String, Value>;

pub fn validate_key(key: &str) -> Result<()> {
    if key.is_empty() {
        return Err(anyhow!("store key must not be empty"));
    }
    if key.len() > MAX_KEY_BYTES {
        return Err(anyhow!("store key exceeds the {MAX_KEY_BYTES}-byte limit"));
    }
    if key.chars().any(char::is_control) {
        return Err(anyhow!("store key `{key}` contains a control character"));
    }
    Ok(())
}

pub fn parse_key_list(value: Option<&Value>, option: &str) -> Result<Vec<String>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let values = match value {
        Value::String(value) => vec![value.clone()],
        Value::Array(values) if !values.is_empty() => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| anyhow!("`{option}` array entries must be strings"))
            })
            .collect::<Result<Vec<_>>>()?,
        Value::Array(_) => return Err(anyhow!("`{option}` must contain at least one key")),
        _ => {
            return Err(anyhow!(
                "`{option}` must be a string or an array of strings"
            ))
        }
    };
    let mut seen = HashSet::new();
    for key in &values {
        validate_key(key)?;
        if !seen.insert(key) {
            return Err(anyhow!("`{option}` contains duplicate store key `{key}`"));
        }
    }
    Ok(values)
}

pub fn validate_value(value: &Value) -> Result<()> {
    validate_value_at(value, 0)?;
    if serde_json::to_vec(value)?.len() > MAX_VALUE_BYTES {
        return Err(anyhow!("store value exceeds the 1 MiB limit; use a file or ordinary display result for larger data"));
    }
    Ok(())
}

fn validate_value_at(value: &Value, depth: usize) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(anyhow!(
            "store value exceeds the maximum nesting depth of {MAX_DEPTH}"
        ));
    }
    match value {
        Value::Null | Value::Bool(_) | Value::String(_) => Ok(()),
        Value::Number(number) if number.is_i64() => Ok(()),
        Value::Number(number) if number.is_u64() => {
            Err(anyhow!("store integer is outside the signed 64-bit range"))
        }
        Value::Number(number) if number.as_f64().is_some_and(f64::is_finite) => Ok(()),
        Value::Number(_) => Err(anyhow!(
            "store number is non-finite or outside the signed 64-bit range"
        )),
        Value::Array(values) => values
            .iter()
            .try_for_each(|value| validate_value_at(value, depth + 1)),
        Value::Object(values) => values.iter().try_for_each(|(key, value)| {
            validate_key(key)?;
            validate_value_at(value, depth + 1)
        }),
    }
}

pub fn validate_store(store: &Store) -> Result<()> {
    if store.len() > MAX_KEYS {
        return Err(anyhow!("document store exceeds the {MAX_KEYS}-key limit"));
    }
    for (key, value) in store {
        validate_key(key)?;
        validate_value(value)?;
    }
    if serde_json::to_vec(store)?.len() > MAX_STORE_BYTES {
        return Err(anyhow!("document store exceeds the 8 MiB limit; use files or ordinary display results for larger data"));
    }
    Ok(())
}

pub fn validate_writer_values(values: &Store) -> Result<()> {
    validate_store(values)?;
    if serde_json::to_vec(values)?.len() > MAX_VALUE_BYTES {
        return Err(anyhow!(
            "store values from one writer exceed the 1 MiB limit; use a file or ordinary display result for larger data"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn store_rejects_unsigned_values_outside_i64() {
        let value = Value::Number(serde_json::Number::from(u64::MAX));
        assert!(validate_value(&value)
            .unwrap_err()
            .to_string()
            .contains("signed 64-bit"));
    }

    #[test]
    fn writer_limit_applies_to_the_aggregate_commit() {
        let mut values = Store::new();
        values.insert("first".to_string(), json!("a".repeat(MAX_VALUE_BYTES / 2)));
        values.insert("second".to_string(), json!("b".repeat(MAX_VALUE_BYTES / 2)));

        assert!(validate_writer_values(&values)
            .unwrap_err()
            .to_string()
            .contains("one writer"));
    }
}
