use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};

use super::config::{LanguageConfig, WebsiteConfig};
use super::paths::join_normalized_under_root;
use super::url::{clean_url_prefix, is_safe_output_route};
use super::util::clean_optional_string;

#[derive(Debug, Clone)]
pub(super) struct LanguageInfo {
    pub(super) code: String,
    pub(super) label: String,
    pub(super) content_dir: PathBuf,
    pub(super) url_prefix: String,
    pub(super) is_default: bool,
}

pub(super) fn configured_languages(
    src_dir: &Path,
    config: &WebsiteConfig,
) -> Result<Option<Vec<LanguageInfo>>> {
    if config.languages.is_empty() {
        return Ok(None);
    }
    let default_language = default_language_code(config)?;
    let mut languages = config
        .languages
        .iter()
        .map(|(code, language)| language_info(src_dir, code, language, &default_language))
        .collect::<Result<Vec<_>>>()?;

    reject_duplicate_url_prefixes(&languages)?;
    reject_duplicate_content_dirs(&languages)?;
    sort_languages(&mut languages);
    Ok(Some(languages))
}

fn default_language_code(config: &WebsiteConfig) -> Result<String> {
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
            bail!("set default_language when more than one language is configured in [languages]")
        }
    };
    if !config.languages.contains_key(&default_language) {
        bail!("default_language `{default_language}` is not present in [languages]");
    }
    Ok(default_language)
}

fn language_info(
    src_dir: &Path,
    code: &str,
    language: &LanguageConfig,
    default_language: &str,
) -> Result<LanguageInfo> {
    let is_default = code == default_language;
    let content_dir = language
        .content_dir
        .clone()
        .unwrap_or_else(|| default_content_dir(code, is_default));
    let content_dir = language_content_dir(src_dir, code, content_dir)?;
    let url_prefix = language_url_prefix(code, language, is_default)?;

    Ok(LanguageInfo {
        code: code.to_string(),
        label: clean_optional_string(language.label.as_deref()).unwrap_or_else(|| code.to_string()),
        content_dir,
        url_prefix,
        is_default,
    })
}

fn default_content_dir(code: &str, is_default: bool) -> PathBuf {
    if is_default {
        PathBuf::from(".")
    } else {
        PathBuf::from(code)
    }
}

fn language_url_prefix(code: &str, language: &LanguageConfig, is_default: bool) -> Result<String> {
    let url_prefix = clean_optional_string(language.url_prefix.as_deref()).unwrap_or_else(|| {
        if is_default {
            String::new()
        } else {
            code.to_string()
        }
    });
    let url_prefix = clean_url_prefix(&url_prefix);
    if !is_safe_output_route(&url_prefix) {
        bail!(
            "url_prefix for language `{code}` must stay inside the output directory: `{url_prefix}`"
        );
    }
    Ok(url_prefix)
}

fn reject_duplicate_url_prefixes(languages: &[LanguageInfo]) -> Result<()> {
    let mut url_prefixes = BTreeMap::new();
    for language in languages {
        if let Some(previous) =
            url_prefixes.insert(language.url_prefix.as_str(), language.code.as_str())
        {
            bail!(
                "url_prefix for language `{}` duplicates language `{previous}` after cleaning: `{}`",
                language.code,
                language.url_prefix
            );
        }
    }
    Ok(())
}

fn reject_duplicate_content_dirs(languages: &[LanguageInfo]) -> Result<()> {
    let mut content_dirs = BTreeMap::new();
    for language in languages {
        if let Some(previous) =
            content_dirs.insert(language.content_dir.clone(), language.code.as_str())
        {
            bail!(
                "content_dir for language `{}` duplicates language `{previous}`: {}",
                language.code,
                language.content_dir.display()
            );
        }
    }
    Ok(())
}

fn sort_languages(languages: &mut [LanguageInfo]) {
    languages.sort_by(|left, right| {
        (!left.is_default, left.code.as_str()).cmp(&(!right.is_default, right.code.as_str()))
    });
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
    let what = invalid_content_dir(code, &content_dir).to_string();
    join_normalized_under_root(src_dir, &content_dir, &what)
}

fn invalid_content_dir(code: &str, content_dir: &Path) -> anyhow::Error {
    anyhow!(
        "content_dir for language `{code}` must stay inside the source directory: {}",
        content_dir.display()
    )
}
