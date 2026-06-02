use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::Metadata;

#[derive(Debug, Clone, Default)]
pub struct ChunkOptions {
    pub inner: HashMap<String, OptionValue>,
    pub metadata: Metadata,
}

#[derive(Debug, Clone)]
pub enum OptionValue {
    Bool(bool),
    String(String),
    Number(f64),
}

impl ChunkOptions {
    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        match self.inner.get(key) {
            Some(OptionValue::Bool(value)) => *value,
            Some(OptionValue::String(value)) => {
                !value.is_empty() && value != "FALSE" && value != "false"
            }
            _ => default,
        }
    }

    pub fn get_string(&self, key: &str, default: &str) -> String {
        match self.inner.get(key) {
            Some(OptionValue::String(value)) => value.clone(),
            Some(OptionValue::Bool(value)) => value.to_string(),
            Some(OptionValue::Number(value)) => value.to_string(),
            _ => default.to_string(),
        }
    }

    pub fn get_number(&self, key: &str, default: f64) -> f64 {
        match self.inner.get(key) {
            Some(OptionValue::Number(value)) => *value,
            Some(OptionValue::String(value)) => value.parse().unwrap_or(default),
            _ => default,
        }
    }

    pub fn get_opt_string(&self, key: &str) -> Option<String> {
        match self.inner.get(key) {
            Some(OptionValue::String(value)) => Some(value.clone()),
            Some(OptionValue::Bool(value)) => Some(value.to_string()),
            Some(OptionValue::Number(value)) => Some(value.to_string()),
            None => None,
        }
    }

    pub fn eval(&self) -> bool {
        let default = self
            .metadata
            .execute
            .as_ref()
            .and_then(|config| config.eval)
            .unwrap_or(true);
        self.get_bool("eval", default)
    }

    pub fn warning(&self) -> bool {
        let default = self
            .metadata
            .execute
            .as_ref()
            .and_then(|config| config.warning)
            .unwrap_or(true);
        self.get_bool("warning", default)
    }

    pub fn message(&self) -> bool {
        let default = self
            .metadata
            .execute
            .as_ref()
            .and_then(|config| config.message)
            .unwrap_or(true);
        self.get_bool("message", default)
    }

    pub fn engine(&self) -> String {
        self.get_opt_string("engine")
            .expect("engine must be set for executable chunks")
    }

    pub fn fig_width(&self) -> f64 {
        let default = self
            .metadata
            .figure
            .as_ref()
            .and_then(|figure| figure.fig_width)
            .unwrap_or(6.0);
        self.get_number("fig_width", default)
    }

    pub fn fig_height(&self) -> f64 {
        if let Some(value) = self.get_opt_string("fig_height") {
            return value
                .parse()
                .unwrap_or_else(|_| self.fig_width() * self.fig_asp());
        }
        if let Some(default) = self
            .metadata
            .figure
            .as_ref()
            .and_then(|figure| figure.fig_height)
        {
            return default;
        }
        self.fig_width() * self.fig_asp()
    }

    pub fn fig_asp(&self) -> f64 {
        let default = self
            .metadata
            .figure
            .as_ref()
            .and_then(|figure| figure.fig_asp)
            .unwrap_or(0.618);
        self.get_number("fig_asp", default)
    }

    pub fn dev(&self) -> String {
        let default = self
            .metadata
            .figure
            .as_ref()
            .and_then(|figure| figure.device.clone())
            .unwrap_or_else(|| "png".to_string());
        self.get_string("dev", &default)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ChunkResult {
    Source(Vec<String>),
    Output(String),
    Warning(String),
    Message(String),
    Error(String),
    Plot(PathBuf),
    Asis(String),
    Preamble(String),
}
