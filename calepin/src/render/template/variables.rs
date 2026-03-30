//! Jinja template context construction: cfg, clp, tpl, and env variables.

use minijinja::Value;

use crate::config::Metadata;

/// Build the full Jinja context for body processing.
///
/// - `cfg.*` -- user-authored: standard metadata fields (title, author, date,
///   etc.) plus custom front matter keys, plus the selected target.
/// - `clp.*` -- engine-computed: the resolved writer format.
/// - `env.*` -- system environment variables (kept as a separate helper).
pub(crate) fn build_context(metadata: &Metadata, format: &str) -> minijinja::Value {
    let config_val = build_config_map(metadata, format);
    let calepin_val = minijinja::context! {
        writer => format,
    };

    let tpl_val = Value::from_serialize(&metadata.tpl);

    minijinja::context! {
        cfg => config_val,
        clp => calepin_val,
        tpl => tpl_val,
        env => Value::from_object(LazyEnv),
    }
}

/// Build the `cfg` context object from metadata.
///
/// Merges standard metadata fields (title, author, date, etc.) with custom
/// front matter keys from `metadata.var` into a single flat-ish object.
fn build_config_map(meta: &Metadata, format: &str) -> Value {
    let mut map = serde_json::Map::new();

    // User-selected target (defaults to format when no explicit target)
    let target = crate::paths::get_active_target().unwrap_or_else(|| format.to_string());
    map.insert("target".into(), serde_json::Value::String(target));

    // Standard metadata fields
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
    // User-selected target
    // (injected by caller as format; updated to actual target name when available)
    // Target is set separately in the context; standard fields above suffice here.

    // Custom front matter keys (formerly `var.*`)
    if !meta.var.is_empty() {
        let json = crate::value::to_json(&crate::value::Value::Table(
            meta.var.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        ));
        if let serde_json::Value::Object(custom) = json {
            for (k, v) in custom {
                // Don't overwrite standard fields
                map.entry(k).or_insert(v);
            }
        }
    }

    Value::from_serialize(&serde_json::Value::Object(map))
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
