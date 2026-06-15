use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use minijinja::{AutoEscape, Environment};
use serde::{Deserialize, Serialize};

use super::{
    absolute_site_url, clean_optional_string, is_safe_output_route, templates::read_template_files,
    xml_escape, SiteMetadata, WebsiteConfig, ROBOTS_TEMPLATE_DIR,
};

const DEFAULT_FEED_FILE: &str = "atom.xml";
const DEFAULT_ATOM_TEMPLATE_NAME: &str = "__calepin_builtin_atom.xml";
const DEFAULT_RSS_TEMPLATE_NAME: &str = "__calepin_builtin_rss.xml";
const DEFAULT_ATOM_TEMPLATE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>{{ feed.title }}</title>
  <id>{{ feed.site_url }}</id>
  <link href="{{ feed.site_url }}"/>
  <link href="{{ feed.feed_url }}" rel="self"/>
  <updated>{{ feed.updated }}</updated>
  {% if feed.description %}<subtitle>{{ feed.description }}</subtitle>{% endif %}
  {% for item in items %}
  <entry>
    <title>{{ item.title }}</title>
    <id>{{ item.url }}</id>
    <link href="{{ item.url }}"/>
    <updated>{{ item.updated }}</updated>
    {% if item.author %}<author><name>{{ item.author }}</name></author>{% endif %}
    {% if item.summary %}<summary>{{ item.summary }}</summary>{% endif %}
  </entry>
  {% endfor %}
</feed>
"#;
const DEFAULT_RSS_TEMPLATE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>{{ feed.title }}</title>
    <link>{{ feed.site_url }}</link>
    {% if feed.description %}<description>{{ feed.description }}</description>{% endif %}
    {% if feed.rss_updated %}<lastBuildDate>{{ feed.rss_updated }}</lastBuildDate>{% endif %}
    {% for item in items %}
    <item>
      <title>{{ item.title }}</title>
      <link>{{ item.url }}</link>
      <guid>{{ item.url }}</guid>
      <pubDate>{{ item.rss_date }}</pubDate>
      {% if item.author %}<author>{{ item.author }}</author>{% endif %}
      {% if item.summary %}<description>{{ item.summary }}</description>{% endif %}
    </item>
    {% endfor %}
  </channel>
</rss>
"#;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
#[serde(default, deny_unknown_fields)]
pub(super) struct FeedsConfig {
    pub(super) limit: Option<usize>,
    pub(super) filenames: Vec<String>,
    pub(super) file: Vec<FeedFileConfig>,
    pub(super) atom_template: Option<String>,
    pub(super) rss_template: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
#[serde(default, deny_unknown_fields)]
pub(super) struct FeedFileConfig {
    pub(super) filename: String,
    pub(super) format: Option<FeedFormat>,
    pub(super) template: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum FeedFormat {
    Atom,
    Rss,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FeedTarget {
    pub(super) filename: String,
    pub(super) format: FeedFormat,
    pub(super) template: Option<String>,
}

#[derive(Debug, Clone)]
struct FeedBuildItem {
    sort_date: String,
    item: FeedTemplateItem,
}

#[derive(Serialize)]
struct FeedTemplateContext<'a> {
    config: &'a WebsiteConfig,
    feed: FeedTemplateInfo,
    items: Vec<FeedTemplateItem>,
}

#[derive(Debug, Clone, Serialize)]
struct FeedTemplateInfo {
    title: String,
    description: Option<String>,
    site_url: String,
    feed_url: String,
    updated: String,
    rss_updated: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct FeedTemplateItem {
    pub(super) title: String,
    pub(super) url: String,
    id: String,
    date: String,
    pub(super) updated: String,
    pub(super) rss_date: String,
    summary: Option<String>,
    pub(super) author: Option<String>,
}

pub(super) fn feed_targets(config: &WebsiteConfig) -> Result<Vec<FeedTarget>> {
    if !config.feeds_enabled() {
        return Ok(Vec::new());
    }

    let feeds = config.feeds.as_ref();
    let mut targets = Vec::new();
    if let Some(feeds) = feeds {
        for filename in &feeds.filenames {
            let filename = clean_feed_filename(filename)?;
            let format = infer_feed_format(&filename);
            let template = feed_format_template(feeds, format);
            targets.push(FeedTarget {
                filename,
                format,
                template,
            });
        }
        for file in &feeds.file {
            let filename = clean_feed_filename(&file.filename)?;
            let format = file.format.unwrap_or_else(|| infer_feed_format(&filename));
            targets.push(FeedTarget {
                filename,
                format,
                template: file
                    .template
                    .as_deref()
                    .and_then(|value| clean_optional_string(Some(value)))
                    .or_else(|| feed_format_template(feeds, format)),
            });
        }
    }

    if targets.is_empty() {
        targets.push(FeedTarget {
            filename: DEFAULT_FEED_FILE.to_string(),
            format: FeedFormat::Atom,
            template: feeds.and_then(|feeds| feed_format_template(feeds, FeedFormat::Atom)),
        });
    }

    let mut seen = BTreeSet::new();
    for target in &targets {
        if !seen.insert(target.filename.clone()) {
            bail!("duplicate feed filename `{}`", target.filename);
        }
    }
    Ok(targets)
}

fn clean_feed_filename(value: &str) -> Result<String> {
    let Some(filename) = clean_optional_string(Some(value)) else {
        bail!("feed filename cannot be empty");
    };
    if filename.ends_with('/') || !is_safe_output_route(&filename) {
        bail!("feed filename must stay inside the output directory: `{filename}`");
    }
    Ok(filename)
}

fn feed_format_template(feeds: &FeedsConfig, format: FeedFormat) -> Option<String> {
    match format {
        FeedFormat::Atom => feeds
            .atom_template
            .as_deref()
            .and_then(|value| clean_optional_string(Some(value))),
        FeedFormat::Rss => feeds
            .rss_template
            .as_deref()
            .and_then(|value| clean_optional_string(Some(value))),
    }
}

pub(super) fn infer_feed_format(filename: &str) -> FeedFormat {
    let lower = filename.to_ascii_lowercase();
    let basename = Path::new(&lower)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(lower.as_str());
    if basename == "rss.xml" || basename.ends_with(".rss") {
        FeedFormat::Rss
    } else {
        FeedFormat::Atom
    }
}

pub(super) fn write_feeds(
    out_dir: &Path,
    src_dir: &Path,
    config: &WebsiteConfig,
    base_url: Option<&str>,
    metadata: &SiteMetadata,
    pages_index: &serde_json::Value,
    targets: &[FeedTarget],
) -> Result<()> {
    if !config.feeds_enabled() {
        return Ok(());
    }
    let Some(base_url) = base_url else {
        bail!("generate_feeds = true requires base_url so feed links can be absolute");
    };

    let limit = config.feeds.as_ref().and_then(|feeds| feeds.limit);
    let items = feed_items_from_pages(pages_index, base_url, limit);
    let mut env = Environment::new();
    env.set_auto_escape_callback(|_| AutoEscape::None);
    env.add_template(DEFAULT_ATOM_TEMPLATE_NAME, DEFAULT_ATOM_TEMPLATE)
        .map_err(|error| anyhow!("feed template: {error}"))?;
    env.add_template(DEFAULT_RSS_TEMPLATE_NAME, DEFAULT_RSS_TEMPLATE)
        .map_err(|error| anyhow!("feed template: {error}"))?;

    let template_dir = src_dir.join(ROBOTS_TEMPLATE_DIR);
    if template_dir.is_dir() {
        for (name, source) in read_template_files(&template_dir)? {
            env.add_template_owned(name, source)
                .map_err(|error| anyhow!("feed template: {error}"))?;
        }
    }

    for target in targets {
        let template_name = target.template.as_deref().unwrap_or(match target.format {
            FeedFormat::Atom => DEFAULT_ATOM_TEMPLATE_NAME,
            FeedFormat::Rss => DEFAULT_RSS_TEMPLATE_NAME,
        });
        let template = env
            .get_template(template_name)
            .map_err(|error| anyhow!("feed template `{template_name}`: {error}"))?;
        let feed_url = absolute_site_url(base_url, &target.filename);
        let updated = items
            .first()
            .map(|item| item.updated.clone())
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
        let rss_updated = items
            .first()
            .map(|item| item.rss_date.clone())
            .unwrap_or_else(|| "Thu, 01 Jan 1970 00:00:00 GMT".to_string());
        let contents = template
            .render(FeedTemplateContext {
                config,
                feed: FeedTemplateInfo {
                    title: xml_escape(metadata.title.as_deref().unwrap_or("Feed")),
                    description: metadata.description.as_deref().map(xml_escape),
                    site_url: xml_escape(base_url),
                    feed_url: xml_escape(&feed_url),
                    updated,
                    rss_updated,
                },
                items: items.clone(),
            })
            .map_err(|error| anyhow!("feed template `{template_name}`: {error}"))?;
        let path = out_dir.join(&target.filename);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, contents)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

pub(super) fn feed_items_from_pages(
    pages_index: &serde_json::Value,
    base_url: &str,
    limit: Option<usize>,
) -> Vec<FeedTemplateItem> {
    let mut items = pages_index
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| feed_item_from_page(entry, base_url))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| {
        right
            .sort_date
            .cmp(&left.sort_date)
            .then_with(|| left.item.title.cmp(&right.item.title))
    });
    if let Some(limit) = limit {
        items.truncate(limit);
    }
    items.into_iter().map(|entry| entry.item).collect()
}

fn feed_item_from_page(entry: &serde_json::Value, base_url: &str) -> Option<FeedBuildItem> {
    let meta = entry.get("meta")?.as_object()?;
    let date = clean_optional_string(meta.get("date")?.as_str())?;
    let href = clean_optional_string(entry.get("href")?.as_str())?;
    let url = absolute_site_url(base_url, &href);
    let title = entry
        .get("title")
        .and_then(|title| title.as_str())
        .and_then(|title| clean_optional_string(Some(title)))
        .unwrap_or_else(|| href.clone());
    let summary = meta
        .get("summary")
        .or_else(|| meta.get("description"))
        .and_then(|summary| summary.as_str())
        .and_then(|summary| clean_optional_string(Some(summary)))
        .map(|summary| xml_escape(&summary));
    let author = feed_author(meta).map(|author| xml_escape(&author));
    let updated = normalize_feed_date(&date);
    let rss_date = rss_feed_date(&date);
    Some(FeedBuildItem {
        sort_date: date.clone(),
        item: FeedTemplateItem {
            title: xml_escape(&title),
            url: xml_escape(&url),
            id: xml_escape(&url),
            date: xml_escape(&date),
            updated,
            rss_date,
            summary,
            author,
        },
    })
}

fn feed_author(meta: &serde_json::Map<String, serde_json::Value>) -> Option<String> {
    if let Some(author) = meta
        .get("author")
        .and_then(|author| author.as_str())
        .and_then(|author| clean_optional_string(Some(author)))
    {
        return Some(author);
    }
    let authors = meta.get("authors")?.as_array()?;
    let names = authors
        .iter()
        .filter_map(|author| author.as_str())
        .filter_map(|author| clean_optional_string(Some(author)))
        .collect::<Vec<_>>();
    (!names.is_empty()).then(|| names.join(", "))
}

fn normalize_feed_date(date: &str) -> String {
    let date = date.trim();
    if date.contains('T') {
        date.to_string()
    } else {
        format!("{date}T00:00:00Z")
    }
}

pub(super) fn rss_feed_date(date: &str) -> String {
    parse_iso_date(date)
        .map(|(year, month, day)| {
            let weekday = weekday_name(year, month, day);
            let month = month_name(month);
            format!("{weekday}, {day:02} {month} {year:04} 00:00:00 GMT")
        })
        .unwrap_or_else(|| date.trim().to_string())
}

fn parse_iso_date(date: &str) -> Option<(i32, u32, u32)> {
    let date = date.trim();
    if date.len() < 10 {
        return None;
    }
    let bytes = date.as_bytes();
    if bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-') {
        return None;
    }
    let year = date.get(0..4)?.parse::<i32>().ok()?;
    let month = date.get(5..7)?.parse::<u32>().ok()?;
    let day = date.get(8..10)?.parse::<u32>().ok()?;
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    Some((year, month, day))
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn weekday_name(year: i32, month: u32, day: u32) -> &'static str {
    const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let days = days_from_civil(year, month, day);
    WEEKDAYS[(days + 4).rem_euclid(7) as usize]
}

fn month_name(month: u32) -> &'static str {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    MONTHS[(month - 1) as usize]
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month as i32;
    let day = day as i32;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    i64::from(era * 146_097 + doe - 719_468)
}
