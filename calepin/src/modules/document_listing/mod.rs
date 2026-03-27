//! Document listing module: renders `::: {.listing}` divs as lists of documents.
//!
//! ```markdown
//! ::: {.listing contents="posts/*/index.qmd" sort="date desc" type="default"}
//! :::
//! ```
//!
//! Attributes:
//!   - `contents` -- glob pattern for documents to list (required)
//!   - `sort` -- sort spec: "date desc", "title asc", etc. (optional)
//!   - `type` -- display type: "default", "grid", "table" (default: "default")

use crate::collection::discover::{ListingConfig, DocumentInfo};
use crate::modules::registry::ModuleContext;
use crate::render::convert::render_inline;

/// Render a `.listing` div as a document listing.
pub fn render(ctx: &ModuleContext) -> String {
    let contents = match ctx.attrs.get("contents") {
        Some(c) => c.clone(),
        None => return String::new(),
    };

    let sort = ctx.attrs.get("sort").cloned();
    let listing_type = ctx.attrs.get("type")
        .cloned()
        .unwrap_or_else(|| "default".to_string());

    let listing_config = ListingConfig {
        contents,
        r#type: listing_type.clone(),
        sort,
        fields: Vec::new(),
        page_size: 0,
    };

    let base_dir = crate::paths::get_project_root();
    let pages = match crate::collection::discover::discover_listing_documents(
        &listing_config, &base_dir, &[], "html",
    ) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Warning: listing discovery failed: {}", e);
            return String::new();
        }
    };

    render_listing_html(&pages, &listing_type)
}

fn render_listing_html(pages: &[DocumentInfo], listing_type: &str) -> String {
    let items: Vec<ListingItemHtml> = pages.iter().map(|p| {
        let title = p.meta.title.as_deref()
            .map(|t| render_inline(t, "html"))
            .unwrap_or_else(|| "Untitled".to_string());
        let date = p.meta.date.as_deref()
            .map(|d| crate::utils::date::format_date_display(d, None));
        let description = p.meta.description.clone();
        let image = p.meta.image.clone();
        let url = p.url.clone();
        ListingItemHtml { title, date, description, image, url }
    }).collect();

    match listing_type {
        "table" => render_table(&items),
        "grid" => render_grid(&items),
        _ => render_default(&items),
    }
}

struct ListingItemHtml {
    title: String,
    date: Option<String>,
    description: Option<String>,
    image: Option<String>,
    url: String,
}

fn render_default(items: &[ListingItemHtml]) -> String {
    let mut out = String::from("<div class=\"listing-default\">\n");
    for item in items {
        out.push_str(&format!("  <a href=\"{}\" class=\"listing-item\">\n", item.url));
        out.push_str("    <div class=\"listing-item-content\">\n");
        out.push_str(&format!("      <h3>{}</h3>\n", item.title));
        if let Some(ref date) = item.date {
            out.push_str(&format!("      <div class=\"date\">{}</div>\n", date));
        }
        if let Some(ref desc) = item.description {
            out.push_str(&format!("      <div class=\"description\">{}</div>\n", desc));
        }
        out.push_str("    </div>\n");
        out.push_str("  </a>\n");
    }
    out.push_str("</div>");
    out
}

fn render_grid(items: &[ListingItemHtml]) -> String {
    let mut out = String::from("<div class=\"listing-grid\">\n");
    for item in items {
        out.push_str(&format!("  <a href=\"{}\" class=\"listing-item\">\n", item.url));
        if let Some(ref img) = item.image {
            out.push_str(&format!("    <img class=\"listing-item-image\" src=\"{}\" alt=\"\">\n", img));
        }
        out.push_str("    <div class=\"listing-item-content\">\n");
        out.push_str(&format!("      <h3>{}</h3>\n", item.title));
        if let Some(ref date) = item.date {
            out.push_str(&format!("      <div class=\"date\">{}</div>\n", date));
        }
        if let Some(ref desc) = item.description {
            out.push_str(&format!("      <div class=\"description\">{}</div>\n", desc));
        }
        out.push_str("    </div>\n");
        out.push_str("  </a>\n");
    }
    out.push_str("</div>");
    out
}

fn render_table(items: &[ListingItemHtml]) -> String {
    let mut out = String::from("<table class=\"listing-table\">\n  <thead>\n    <tr><th>Title</th><th>Date</th><th>Description</th></tr>\n  </thead>\n  <tbody>\n");
    for item in items {
        out.push_str(&format!(
            "    <tr><td><a href=\"{}\">{}</a></td><td>{}</td><td>{}</td></tr>\n",
            item.url,
            item.title,
            item.date.as_deref().unwrap_or(""),
            item.description.as_deref().unwrap_or(""),
        ));
    }
    out.push_str("  </tbody>\n</table>");
    out
}
