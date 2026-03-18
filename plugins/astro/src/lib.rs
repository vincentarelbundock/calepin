use std::collections::HashMap;

use extism_pdk::*;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Heading extraction from rendered HTML
// ---------------------------------------------------------------------------

struct Heading {
    depth: u8,
    slug: String,
    text: String,
}

/// Extract headings (h2–h4) from HTML for Starlight's TOC.
/// Looks for patterns like `<h2 id="slug">text</h2>`.
fn extract_headings(html: &str) -> Vec<Heading> {
    let mut headings = Vec::new();
    let mut pos = 0;
    let bytes = html.as_bytes();
    while pos < bytes.len() {
        // Find <h2, <h3, or <h4
        if let Some(idx) = html[pos..].find("<h") {
            let abs = pos + idx;
            let after_h = abs + 2;
            if after_h < bytes.len() {
                let depth_char = bytes[after_h];
                if matches!(depth_char, b'2' | b'3' | b'4') {
                    let depth = depth_char - b'0';
                    // Extract id attribute
                    if let Some(id_start) = html[after_h..].find("id=\"") {
                        let id_begin = after_h + id_start + 4;
                        if let Some(id_end) = html[id_begin..].find('"') {
                            let slug = &html[id_begin..id_begin + id_end];
                            // Find the closing > of the opening tag
                            if let Some(tag_close) = html[id_begin..].find('>') {
                                let text_start = id_begin + tag_close + 1;
                                // Find closing tag </hN>
                                let close_tag = format!("</h{}>", depth);
                                if let Some(text_end) = html[text_start..].find(&close_tag) {
                                    let raw_text = &html[text_start..text_start + text_end];
                                    // Strip any inner HTML tags to get plain text
                                    let text = strip_html_tags(raw_text);
                                    if !text.is_empty() {
                                        headings.push(Heading {
                                            depth,
                                            slug: slug.to_string(),
                                            text,
                                        });
                                    }
                                    pos = text_start + text_end;
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
            pos = abs + 1;
        } else {
            break;
        }
    }
    headings
}

/// Strip HTML tags from a string, returning plain text.
fn strip_html_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(c);
        }
    }
    result.trim().to_string()
}

/// Format headings as a JavaScript array literal for Starlight's headings prop.
fn format_headings_js(headings: &[Heading]) -> String {
    if headings.is_empty() {
        return "[]".to_string();
    }
    let items: Vec<String> = headings
        .iter()
        .map(|h| {
            format!(
                "  {{ depth: {}, slug: '{}', text: '{}' }}",
                h.depth,
                escape_js(&h.slug),
                escape_js(&h.text),
            )
        })
        .collect();
    format!("[\n{}\n]", items.join(",\n"))
}

// ---------------------------------------------------------------------------
// Shared protocol types (mirroring host)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SiteBuildContext {
    config: serde_json::Value,
    pages: Vec<RenderedPage>,
    syntax_css: String,
}

#[derive(Deserialize)]
struct RenderedPage {
    stem: String,
    title: String,
    body_html: String,
    raw_source: String,
    is_index: bool,
    vars: HashMap<String, String>,
}

#[derive(Serialize)]
struct SiteBuildResult {
    scaffold_command: Option<String>,
    output_dir: String,
    cleanup_dirs: Vec<String>,
    files: Vec<SiteFile>,
    copies: Vec<CopyRule>,
}

#[derive(Serialize)]
struct SiteFile {
    path: String,
    content: String,
}

#[derive(Serialize)]
struct CopyRule {
    from: String,
    to: Vec<String>,
}

// ---------------------------------------------------------------------------
// Embedded CSS
// ---------------------------------------------------------------------------

const ASTRO_CSS: &str = include_str!("../astro.css");

// ---------------------------------------------------------------------------
// Plugin entry point
// ---------------------------------------------------------------------------

#[plugin_fn]
pub fn build_site(Json(ctx): Json<SiteBuildContext>) -> FnResult<Json<SiteBuildResult>> {
    let config = parse_config(&ctx.config);
    let mut files = Vec::new();
    let mut copies = Vec::new();

    // Page titles from rendered pages
    let titles: HashMap<String, String> = ctx
        .pages
        .iter()
        .map(|p| (p.stem.clone(), p.title.clone()))
        .collect();

    // Generate page files
    for page in &ctx.pages {
        // HTML body
        files.push(SiteFile {
            path: format!("src/html/{}.html", page.stem),
            content: page.body_html.clone(),
        });

        // Raw source for split view
        files.push(SiteFile {
            path: format!("src/qmd/{}.qmd", page.stem),
            content: page.raw_source.clone(),
        });

        // Astro page wrapper
        let astro_page = if page.is_index {
            build_astro_index_page(&page.stem, &config, &page.vars, &page.body_html)
        } else {
            let plain_title = strip_markdown(&page.title);
            build_astro_page(&page.stem, &plain_title, &page.body_html)
        };
        files.push(SiteFile {
            path: format!("src/pages/{}.astro", page.stem),
            content: astro_page,
        });
    }

    // Placeholder doc so Starlight's content collection is not empty
    files.push(SiteFile {
        path: "src/content/docs/_placeholder.md".to_string(),
        content: "---\ntitle: Home\n---\n".to_string(),
    });

    // astro.config.mjs
    files.push(SiteFile {
        path: "astro.config.mjs".to_string(),
        content: build_astro_config(&config, &titles),
    });

    // calepin.css (component styles + syntax highlighting)
    files.push(SiteFile {
        path: "src/styles/calepin.css".to_string(),
        content: format!("{}\n{}", ASTRO_CSS, ctx.syntax_css),
    });

    // Copy rules for assets
    if let Some(ref logo) = config.logo {
        let filename = file_name(logo);
        copies.push(CopyRule {
            from: logo.clone(),
            to: vec![
                format!("src/assets/{}", filename),
                format!("public/{}", filename),
            ],
        });
        // Dark variant
        if let Some(dark) = dark_variant(logo) {
            let dark_filename = file_name(&dark);
            copies.push(CopyRule {
                from: dark,
                to: vec![
                    format!("src/assets/{}", dark_filename),
                    format!("public/{}", dark_filename),
                ],
            });
        }
    }

    if let Some(ref favicon) = config.favicon {
        let filename = file_name(favicon);
        copies.push(CopyRule {
            from: favicon.clone(),
            to: vec![format!("public/{}", filename)],
        });
    }

    for resource in &config.resources {
        copies.push(CopyRule {
            from: resource.clone(),
            to: vec![format!("public/{}", resource)],
        });
    }

    // Figure directories
    for page in &ctx.pages {
        copies.push(CopyRule {
            from: format!("{}_files", page.stem),
            to: vec![format!("public/{}_files", page.stem)],
        });
    }

    // Non-.qmd page files (e.g., .pdf) — copy to public/
    fn collect_non_qmd(entries: &[PageEntry], out: &mut Vec<String>) {
        for entry in entries {
            match entry {
                PageEntry::Page { href, .. } => {
                    if !href.ends_with(".qmd") {
                        out.push(href.clone());
                    }
                }
                PageEntry::Section { pages, .. } => collect_non_qmd(pages, out),
            }
        }
    }
    let mut non_qmd = Vec::new();
    collect_non_qmd(&config.pages, &mut non_qmd);
    for href in &non_qmd {
        copies.push(CopyRule {
            from: href.clone(),
            to: vec![format!("public/{}", href)],
        });
    }

    Ok(Json(SiteBuildResult {
        scaffold_command: Some(
            "npm create astro@latest -- --template starlight --no-install --yes _astro".to_string(),
        ),
        output_dir: "_astro".to_string(),
        cleanup_dirs: vec!["src/content/docs".to_string()],
        files,
        copies,
    }))
}

// ---------------------------------------------------------------------------
// Config parsing from JSON
// ---------------------------------------------------------------------------

struct AstroConfig {
    title: String,
    logo: Option<String>,
    favicon: Option<String>,
    navbar_right: Vec<NavItem>,
    pages: Vec<PageEntry>,
    resources: Vec<String>,
}

struct NavItem {
    text: String,
    href: String,
}

#[derive(Clone)]
enum PageEntry {
    Page { text: Option<String>, href: String },
    Section { title: String, pages: Vec<PageEntry> },
}

fn parse_config(val: &serde_json::Value) -> AstroConfig {
    let website = &val["website"];
    let title = website["title"].as_str().unwrap_or("Untitled").to_string();
    let logo = website["navbar"]["logo"].as_str().map(|s| s.to_string());
    let favicon = website["favicon"].as_str().map(|s| s.to_string());

    let navbar_right = website["navbar"]["right"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let text = item["text"].as_str()?.to_string();
                    let href = item["href"].as_str()?.to_string();
                    Some(NavItem { text, href })
                })
                .collect()
        })
        .unwrap_or_default();

    let pages = parse_page_entries(&website["pages"]);

    let resources = val["project"]["resources"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    AstroConfig {
        title,
        logo,
        favicon,
        navbar_right,
        pages,
        resources,
    }
}

fn parse_page_entries(val: &serde_json::Value) -> Vec<PageEntry> {
    let Some(seq) = val.as_array() else {
        return Vec::new();
    };
    seq.iter()
        .filter_map(|item| {
            if let Some(s) = item.as_str() {
                Some(PageEntry::Page {
                    text: None,
                    href: s.to_string(),
                })
            } else if item["section"].is_string() {
                let title = item["section"].as_str().unwrap().to_string();
                let pages = parse_page_entries(&item["pages"]);
                Some(PageEntry::Section { title, pages })
            } else if item["href"].is_string() {
                let href = item["href"].as_str().unwrap().to_string();
                let text = item["text"].as_str().map(|s| s.to_string());
                Some(PageEntry::Page { text, href })
            } else {
                None
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Astro page templates
// ---------------------------------------------------------------------------

/// Compute the relative prefix to get from `src/pages/<stem>.astro` back to `src/`.
/// A flat stem like "basics" needs "../", a nested stem like "code/r" needs "../../".
fn relative_prefix(stem: &str) -> String {
    let depth = stem.matches('/').count() + 1;
    "../".repeat(depth)
}

fn build_astro_page(stem: &str, title: &str, body_html: &str) -> String {
    let prefix = relative_prefix(stem);
    let headings = extract_headings(body_html);
    let headings_js = format_headings_js(&headings);
    format!(
        r##"---
import StarlightPage from '@astrojs/starlight/components/StarlightPage.astro';
import body from '{prefix}html/{stem}.html?raw';
import source from '{prefix}qmd/{stem}.qmd?raw';
const headings = {headings_js};
---
<StarlightPage frontmatter={{{{ title: "{title}" }}}} headings={{headings}}>
  <div class="split-wrapper">
    <div class="split-rendered">
      <Fragment set:html={{body}} />
    </div>
    <pre class="split-source"><code>{{source}}</code></pre>
  </div>
</StarlightPage>
{scripts}"##,
        prefix = prefix,
        stem = stem,
        headings_js = headings_js,
        title = escape_astro_string(title),
        scripts = ASTRO_SCRIPTS,
    )
}

fn build_astro_index_page(
    stem: &str,
    config: &AstroConfig,
    vars: &HashMap<String, String>,
    body_html: &str,
) -> String {
    // Logo with light/dark variants
    let logo_html = config
        .logo
        .as_ref()
        .map(|logo| {
            let filename = file_name(logo);
            let alt = escape_astro_string(&config.title);

            if let Some(dark) = dark_variant(logo) {
                let dark_filename = file_name(&dark);
                format!(
                    r#"<img src="/{filename}" alt="{alt}" class="hero-logo hero-logo-light" /><img src="/{dark_filename}" alt="{alt}" class="hero-logo hero-logo-dark" />"#,
                )
            } else {
                format!(r#"<img src="/{filename}" alt="{alt}" class="hero-logo" />"#)
            }
        })
        .unwrap_or_default();

    let subtitle = vars.get("subtitle-block").cloned().unwrap_or_default();
    let author = vars.get("author-block").cloned().unwrap_or_default();
    let date = vars.get("date-block").cloned().unwrap_or_default();
    let abstract_block = vars.get("abstract-block").cloned().unwrap_or_default();
    let plain_title = vars
        .get("plain-title")
        .cloned()
        .unwrap_or_else(|| config.title.clone());

    let prefix = relative_prefix(stem);
    let headings = extract_headings(body_html);
    let headings_js = format_headings_js(&headings);
    format!(
        r##"---
import StarlightPage from '@astrojs/starlight/components/StarlightPage.astro';
import body from '{prefix}html/{stem}.html?raw';
import source from '{prefix}qmd/{stem}.qmd?raw';
const headings = {headings_js};
---
<StarlightPage frontmatter={{{{ title: "{title}" }}}} headings={{headings}}>
  <header class="hero-header">
    {logo_html}
    <div class="hero-subtitle">{subtitle}</div>
    <div class="hero-meta">{author}</div>
    <div class="hero-meta">{date}</div>
    <div class="hero-abstract">{abstract_block}</div>
  </header>
  <div class="split-wrapper">
    <div class="split-rendered">
      <Fragment set:html={{body}} />
    </div>
    <pre class="split-source"><code>{{source}}</code></pre>
  </div>
</StarlightPage>

<style>
  .hero-header {{
    text-align: center;
    margin-bottom: 2rem;
  }}
  .hero-header .hero-logo {{
    height: 10rem;
    margin-bottom: 0.5rem;
  }}
  .hero-subtitle {{
    font-size: 1.2rem;
    color: var(--sl-color-gray-2);
    margin-bottom: 0.5rem;
  }}
  .hero-subtitle :global(h2) {{
    font-size: inherit;
    color: inherit;
    margin: 0;
    font-weight: normal;
  }}
  .hero-meta {{
    font-size: 0.95rem;
    color: var(--sl-color-gray-3);
  }}
  .hero-meta :global(h3) {{
    font-size: inherit;
    color: inherit;
    margin: 0;
    font-weight: normal;
  }}
  .hero-abstract {{
    max-width: 40rem;
    margin: 1rem auto 0;
    font-size: 0.95rem;
    font-style: italic;
    color: var(--sl-color-gray-2);
  }}
  .hero-abstract :global(.abstract) {{
    border: none;
    padding: 0;
    margin: 0;
    font-family: inherit;
    font-size: inherit;
  }}
  .hero-abstract :global(.abstract::before) {{
    display: none;
  }}
  .hero-logo-dark {{ display: none; }}
  :global([data-theme='dark']) .hero-logo-light {{ display: none; }}
  :global([data-theme='dark']) .hero-logo-dark {{ display: inline; }}
  :global(h1#_top) {{
    display: none;
  }}
  :global([data-content-title]) {{
    text-align: center;
  }}
</style>
{scripts}"##,
        prefix = prefix,
        stem = stem,
        headings_js = headings_js,
        title = escape_astro_string(&plain_title),
        logo_html = logo_html,
        subtitle = subtitle,
        author = author,
        date = date,
        abstract_block = abstract_block,
        scripts = ASTRO_SCRIPTS,
    )
}

const ASTRO_SCRIPTS: &str = r##"
<script is:inline>
MathJax = {
  tex: { inlineMath: [['$','$'], ['\\(','\\)']], displayMath: [['$$','$$'], ['\\[','\\]']] },
  options: { ignoreHtmlClass: 'nodollar' },
  svg: { fontCache: 'global' }
};
</script>
<script is:inline src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-svg.js"></script>

<script>
document.querySelectorAll('.panel-tabset .nav-link').forEach(function(btn) {
  btn.addEventListener('click', function() {
    var tabset = btn.closest('.panel-tabset');
    var tabId = btn.getAttribute('data-tab');
    var group = tabset.getAttribute('data-group');
    var targets = group
      ? document.querySelectorAll('.panel-tabset[data-group="' + group + '"]')
      : [tabset];
    targets.forEach(function(ts) {
      ts.querySelectorAll('.nav-link').forEach(function(b) {
        b.classList.toggle('active', b.getAttribute('data-tab') === tabId);
      });
      ts.querySelectorAll('.tab-pane').forEach(function(p) {
        p.classList.toggle('active', p.getAttribute('data-tab') === tabId);
      });
    });
  });
});
document.querySelectorAll('.footnote-ref a').forEach(function(a) {
  var id = a.getAttribute('href');
  if (!id) return;
  var fn = document.querySelector(id);
  if (!fn) return;
  var text = fn.textContent.replace(/\s*↩\s*$/, '').trim();
  if (!text) return;
  var tip = document.createElement('span');
  tip.className = 'fn-preview';
  tip.textContent = text;
  a.parentElement.appendChild(tip);
});

// Split view toggle
(function() {
  var btn = document.createElement('button');
  btn.className = 'split-toggle';
  btn.textContent = '</>';
  btn.title = 'Toggle source view';
  btn.addEventListener('click', function() {
    document.body.classList.toggle('split-active');
    btn.classList.toggle('active');
  });
  var target = document.querySelector('.right-group') || document.querySelector('header');
  if (target) target.prepend(btn);
})();
</script>
"##;

// ---------------------------------------------------------------------------
// Astro config generation
// ---------------------------------------------------------------------------

fn build_astro_config(config: &AstroConfig, titles: &HashMap<String, String>) -> String {
    // Logo config
    let logo_config = config
        .logo
        .as_ref()
        .map(|logo| {
            let filename = file_name(logo);
            if let Some(dark) = dark_variant(logo) {
                let dark_filename = file_name(&dark);
                format!(
                    "\n      logo: {{ replacesTitle: true, light: './src/assets/{}', dark: './src/assets/{}' }},",
                    filename, dark_filename,
                )
            } else {
                format!(
                    "\n      logo: {{ replacesTitle: true, src: './src/assets/{}' }},",
                    filename
                )
            }
        })
        .unwrap_or_default();

    // Favicon config
    let favicon_config = config
        .favicon
        .as_ref()
        .map(|fav| {
            let filename = file_name(fav);
            format!("\n      favicon: '{}',", filename)
        })
        .unwrap_or_default();

    // Social links
    let social_links: Vec<String> = config
        .navbar_right
        .iter()
        .map(|item| {
            let icon = guess_social_icon(&item.text, &item.href);
            format!(
                "        {{ icon: '{}', label: '{}', href: '{}' }}",
                icon,
                escape_js(&item.text),
                escape_js(&item.href),
            )
        })
        .collect();
    let social_config = if social_links.is_empty() {
        String::new()
    } else {
        format!(
            "\n      social: [\n{}\n      ],",
            social_links.join(",\n")
        )
    };

    // Sidebar
    let sidebar_items = build_sidebar_config(&config.pages, titles);

    format!(
        r#"// @ts-check
import {{ defineConfig }} from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({{
  integrations: [
    starlight({{
      title: '{title}',{logo}{favicon}{social}
      customCss: ['./src/styles/calepin.css'],
      sidebar: [
{sidebar}
      ],
    }}),
  ],
  trailingSlash: 'never',
}});
"#,
        title = escape_js(&config.title),
        logo = logo_config,
        favicon = favicon_config,
        social = social_config,
        sidebar = sidebar_items,
    )
}

fn stem_to_link(stem: &str) -> String {
    if stem == "index" {
        "/".to_string()
    } else {
        format!("/{stem}")
    }
}

/// Convert an href to a sidebar link. Non-.qmd files link directly to the file.
fn href_to_link(href: &str) -> String {
    if href.ends_with(".qmd") {
        let stem = href.strip_suffix(".qmd").unwrap();
        stem_to_link(stem)
    } else {
        format!("/{}", href)
    }
}

fn page_display_title(
    href: &str,
    explicit_text: Option<&str>,
    titles: &HashMap<String, String>,
) -> String {
    if let Some(text) = explicit_text {
        return text.to_string();
    }
    // Try stem as key (titles map uses stems)
    let stem = href
        .strip_suffix(".qmd")
        .unwrap_or(href);
    if let Some(title) = titles.get(stem) {
        return title.clone();
    }
    title_from_filename(href)
}

fn title_from_filename(href: &str) -> String {
    let stem = href.strip_suffix(".qmd").unwrap_or(href);
    stem.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    format!("{}{}", upper, chars.collect::<String>())
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_sidebar_config(entries: &[PageEntry], titles: &HashMap<String, String>) -> String {
    let items: Vec<String> = entries
        .iter()
        .map(|entry| match entry {
            PageEntry::Page { text, href } => {
                let display =
                    strip_markdown(&page_display_title(href, text.as_deref(), titles));
                let link = href_to_link(href);
                format!(
                    "        {{ label: '{}', link: '{}' }}",
                    escape_js(&display),
                    link,
                )
            }
            PageEntry::Section { title, pages } => {
                let children = build_sidebar_items(pages, titles);
                format!(
                    "        {{\n          label: '{}',\n          items: [\n{}\n          ]\n        }}",
                    escape_js(title),
                    children,
                )
            }
        })
        .collect();
    items.join(",\n")
}

fn build_sidebar_items(entries: &[PageEntry], titles: &HashMap<String, String>) -> String {
    let items: Vec<String> = entries
        .iter()
        .map(|entry| match entry {
            PageEntry::Page { text, href } => {
                let display =
                    strip_markdown(&page_display_title(href, text.as_deref(), titles));
                let link = href_to_link(href);
                format!(
                    "            {{ label: '{}', link: '{}' }}",
                    escape_js(&display),
                    link,
                )
            }
            PageEntry::Section { title, pages } => {
                let children = build_sidebar_items(pages, titles);
                format!(
                    "            {{ label: '{}', items: [\n{}\n            ] }}",
                    escape_js(title),
                    children,
                )
            }
        })
        .collect();
    items.join(",\n")
}

fn guess_social_icon(text: &str, href: &str) -> &'static str {
    let lower = text.to_lowercase();
    let href_lower = href.to_lowercase();
    if lower.contains("github") || href_lower.contains("github.com") {
        "github"
    } else if lower.contains("discord") || href_lower.contains("discord") {
        "discord"
    } else if lower.contains("twitter")
        || lower.contains("x.com")
        || href_lower.contains("twitter.com")
        || href_lower.contains("x.com")
    {
        "x.com"
    } else if lower.contains("mastodon") || href_lower.contains("mastodon") {
        "mastodon"
    } else if lower.contains("bluesky") || href_lower.contains("bsky") {
        "blueSky"
    } else if lower.contains("linkedin") || href_lower.contains("linkedin") {
        "linkedin"
    } else if lower.contains("youtube") || href_lower.contains("youtube") {
        "youtube"
    } else {
        "github"
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn escape_js(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

fn escape_astro_string(s: &str) -> String {
    s.replace('"', "&quot;")
        .replace('{', "&#123;")
        .replace('}', "&#125;")
}

fn strip_markdown(text: &str) -> String {
    text.replace('*', "").replace('_', "")
}

/// Extract filename from a path string.
fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

/// Given "logo.png", return Some("logo_dark.png"). Returns None if no extension.
fn dark_variant(path: &str) -> Option<String> {
    let filename = file_name(path);
    let dot = filename.rfind('.')?;
    let stem = &filename[..dot];
    let ext = &filename[dot..];
    // Reconstruct with directory prefix if any
    let dir = if path.contains('/') {
        let last_slash = path.rfind('/').unwrap();
        &path[..=last_slash]
    } else {
        ""
    };
    Some(format!("{}{}_dark{}", dir, stem, ext))
}
