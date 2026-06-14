# Zola Feature Inventory Not Already Available in Calepin

Items with direct Calepin equivalents have been removed. This keeps Zola features
that are still absent as built-in Calepin static-site-generator features.

- [ ] link checker
- [ ] feed generation
  - feed filenames
  - item limit
  - atom.xml rss.xml
- [ ] calepin new notebook (with default author, date, etc. would be useful for new blog posts)
- [ ] syntax highlighting. Consistent in raw chunks and calepin processed chunks. PDF and HTML. Light vs. Dark

- **Pages**
  - slugification
  - word count.
  - reading time.
  - date formatted
    - Filenames starting with dates can set page dates automatically.
    - Date prefixes can be stripped from slugs.
  - breadcrumb
  - nice blog listing

- **Shortcodes**
  - Store shortcodes in `templates/shortcodes`.
  - Use shortcodes inside Markdown.
  - Inject complex HTML through shortcodes.
  - Reduce repetitive data-driven content with shortcodes.
  - Define HTML shortcodes.
  - Define Markdown shortcodes.
  - Allow Markdown shortcode headings to appear in the table of contents.
  - Use Tera macros as the template-side analogue.

- **Taxonomies**
  - taxonomies (we already have tags through #metadata)
  - Supports built-in-style taxonomy use cases such as tags.
  - Supports categories.
  - Supports authors.
  - Supports arbitrary custom taxonomy names.
  - Generates taxonomy list pages.
  - Generates taxonomy term pages.
  - Supports taxonomy term pagination.
  - Supports taxonomy term feeds.
  - Supports language-specific taxonomies.
  - Allows taxonomy rendering to be disabled with `render = false`.
  - Does not slugify taxonomy names.
  - Slugifies taxonomy terms by default.
  - Treats taxonomy terms as case-insensitive when they produce the same slug.
  - Supports `taxonomy_root` URL prefixes.

- **Feeds**
  - Generate site-wide feeds.
  - Enable feeds with `generate_feeds = true`.
  - Default feed filename is `atom.xml`.
  - Built-in Atom 1.0 template.
  - Built-in RSS 2.0 template.
  - Support custom feed filenames.
  - Support custom feed templates.
  - Configure feed item limit.
  - Include only dated pages in feeds.
  - Generate taxonomy term feeds when taxonomy `feed = true`.

- **Deployment documentation**
  - Deployment guide for Sourcehut Pages.
  - Deployment guide for Netlify.
  - Deployment guide for GitHub Pages.
  - Deployment guide for GitLab Pages.
  - Deployment guide for Codeberg Pages.
  - Deployment guide for Edgio.
  - Deployment guide for Vercel.
  - Deployment guide for Zeabur.
  - Deployment guide for Azure Static Web Apps.
  - Deployment guide for Cloudflare Pages.
  - Deployment guide for Cloudflare Workers.
  - Deployment guide for Fly.io.
  - Deployment guide for AWS S3 Bucket.
  - Deployment guide for Docker image.

- **Link checking**
  - Check external Markdown links with `zola check`.
  - Skip external link checking with `--skip-external-links`.
  - Configure skipped URL prefixes.
  - Configure skipped anchor-check prefixes.
  - Treat internal link failures as errors or warnings.
  - Treat external link failures as errors or warnings.

- **URL and slug control**
  - Slugify paths.
  - Slugify taxonomies.
  - Slugify anchors.
  - Use slugification strategy `on`.
  - Use slugification strategy `safe`.
  - Use slugification strategy `off`.
  - Preserve or strip date prefixes in paths with `paths_keep_dates`.
  - Use escaping when relaxed slugification affects internal links.
