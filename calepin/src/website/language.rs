use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, bail, Result};

use super::config::WebsiteConfig;
use super::paths::normalize_path;
use super::url::{clean_url_prefix, is_safe_output_route};
use super::util::clean_optional_string;

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
    for code in config.languages.keys() {
        validate_language_code(code)?;
    }
    let default_language = match config.default_language.as_deref().map(str::trim) {
        Some("") => bail!("default_language must not be empty"),
        Some(default_language) => {
            validate_language_code(default_language)?;
            default_language.to_string()
        }
        None if config.languages.len() == 1 => config.languages.keys().next().cloned().unwrap(),
        None => {
            return Err(anyhow!(
                "set default_language when more than one language is configured in [languages]"
            ))
        }
    };
    if !config.languages.contains_key(&default_language) {
        return Err(anyhow!(
            "default_language `{default_language}` is not present in [languages]"
        ));
    }

    let mut languages = Vec::new();
    let mut url_prefixes = BTreeMap::new();
    for (code, language) in &config.languages {
        let default = code == &default_language;
        let content_dir = language.content_dir.clone().unwrap_or_else(|| {
            if default {
                PathBuf::from(".")
            } else {
                PathBuf::from(code)
            }
        });
        let content_dir = language_content_dir(src_dir, code, content_dir)?;
        let url_prefix = clean_optional_string(language.url_prefix.as_deref())
            .unwrap_or_else(|| if default { String::new() } else { code.clone() });
        let url_prefix = clean_url_prefix(&url_prefix);
        if !is_safe_output_route(&url_prefix) {
            bail!("url_prefix for language `{code}` must stay inside the output directory: `{url_prefix}`");
        }
        if let Some(previous) = url_prefixes.insert(url_prefix.clone(), code.clone()) {
            bail!(
                "url_prefix for language `{code}` duplicates language `{previous}` after cleaning: `{url_prefix}`"
            );
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

fn validate_language_code(code: &str) -> Result<()> {
    if code.trim().is_empty() {
        bail!("language code must not be empty");
    }
    if code != code.trim() {
        bail!("language code `{code}` must not start or end with whitespace");
    }
    if matches!(code, "." | "..") || code.contains('/') || code.contains('\\') {
        bail!("language code `{code}` must be a single path segment");
    }
    Ok(())
}

fn language_content_dir(src_dir: &Path, code: &str, content_dir: PathBuf) -> Result<PathBuf> {
    if content_dir == Path::new(".") {
        return Ok(src_dir.to_path_buf());
    }
    if content_dir.as_os_str().is_empty()
        || content_dir.is_absolute()
        || content_dir
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        bail!(
            "content_dir for language `{code}` must stay inside the source directory: {}",
            content_dir.display()
        );
    }

    let src_dir = normalize_path(src_dir);
    let content_dir = normalize_path(&src_dir.join(content_dir));
    if !content_dir.starts_with(&src_dir) {
        bail!(
            "content_dir for language `{code}` must stay inside the source directory: {}",
            content_dir.display()
        );
    }
    Ok(content_dir)
}
