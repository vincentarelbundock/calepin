use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};

use super::config::WebsiteConfig;
use super::util::{clean_optional_string, is_safe_output_route};

#[derive(Debug, Clone)]
pub(super) struct LanguageInfo {
    pub(super) code: String,
    pub(super) label: String,
    pub(super) content_dir: PathBuf,
    pub(super) url_prefix: String,
    pub(super) default: bool,
}

pub(super) fn configured_languages(
    src_dir: &Path,
    config: &WebsiteConfig,
) -> Result<Option<Vec<LanguageInfo>>> {
    if config.languages.is_empty() {
        return Ok(None);
    }
    let default_language = match config.default_language.as_deref() {
        Some(default_language) => default_language,
        None if config.languages.len() == 1 => {
            config.languages.keys().next().map(String::as_str).unwrap()
        }
        None => {
            return Err(anyhow!(
                "set default_language when more than one language is configured in [languages]"
            ))
        }
    };
    if !config.languages.contains_key(default_language) {
        return Err(anyhow!(
            "default_language `{default_language}` is not present in [languages]"
        ));
    }

    let mut languages = Vec::new();
    for (code, language) in &config.languages {
        let default = code == default_language;
        let content_dir = language.content_dir.clone().unwrap_or_else(|| {
            if default {
                PathBuf::from(".")
            } else {
                PathBuf::from(code)
            }
        });
        let content_dir = if content_dir == Path::new(".") {
            src_dir.to_path_buf()
        } else if content_dir.is_absolute() {
            content_dir
        } else {
            src_dir.join(content_dir)
        };
        let url_prefix = clean_optional_string(language.url_prefix.as_deref())
            .unwrap_or_else(|| if default { String::new() } else { code.clone() });
        let url_prefix = clean_url_prefix(&url_prefix);
        if !is_safe_output_route(&url_prefix) {
            bail!("url_prefix for language `{code}` must stay inside the output directory: `{url_prefix}`");
        }
        languages.push(LanguageInfo {
            code: code.clone(),
            label: clean_optional_string(language.label.as_deref()).unwrap_or_else(|| code.clone()),
            content_dir,
            url_prefix,
            default,
        });
    }
    languages.sort_by_key(|language| (!language.default, language.code.clone()));
    Ok(Some(languages))
}

fn clean_url_prefix(value: &str) -> String {
    value
        .trim()
        .trim_matches('/')
        .trim_start_matches("./")
        .to_string()
}
