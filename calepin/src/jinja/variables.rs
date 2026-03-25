//! Jinja template context construction: meta, var, and env variables.

use minijinja::Value;

use crate::metadata::Metadata;

/// Build the full Jinja context for body processing.
pub(crate) fn build_context(metadata: &Metadata, format: &str) -> minijinja::Value {
    let meta_val = build_meta_map(metadata);
    let var_val = build_variables_map(metadata);

    minijinja::context! {
        engine => format,
        target => format,
        meta => meta_val,
        var => var_val,
        env => Value::from_object(LazyEnv),
    }
}

/// Build a serde_json::Value map from Metadata for the `meta` context variable.
fn build_meta_map(meta: &Metadata) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    if let Some(ref t) = meta.title {
        map.insert("title".into(), serde_json::Value::String(t.clone()));
    }
    if let Some(ref s) = meta.subtitle {
        map.insert("subtitle".into(), serde_json::Value::String(s.clone()));
    }
    {
        let names = meta.author_names();
        if !names.is_empty() {
            map.insert("author".into(), serde_json::json!(names));
        }
    }
    if let Some(ref d) = meta.date {
        map.insert("date".into(), serde_json::Value::String(d.clone()));
    }
    if let Some(ref abs) = meta.abstract_text {
        map.insert("abstract".into(), serde_json::Value::String(abs.clone()));
    }
    if !meta.keywords.is_empty() {
        map.insert("keywords".into(), serde_json::json!(meta.keywords));
    }
    serde_json::Value::Object(map)
}

/// Build the `var` context from extra front matter fields.
fn build_variables_map(metadata: &Metadata) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (key, value) in &metadata.var {
        map.insert(key.clone(), crate::value::to_json(value));
    }
    serde_json::Value::Object(map)
}

/// MiniJinja object that resolves `{{ env.VAR }}` on demand via std::env::var().
/// Avoids collecting the entire process environment on every process_body() call.
#[derive(Debug)]
struct LazyEnv;

impl minijinja::value::Object for LazyEnv {
    fn get_value(self: &std::sync::Arc<Self>, key: &Value) -> Option<Value> {
        let key_str = key.as_str()?;
        std::env::var(key_str).ok().map(Value::from)
    }

    fn repr(self: &std::sync::Arc<Self>) -> minijinja::value::ObjectRepr {
        minijinja::value::ObjectRepr::Map
    }
}
