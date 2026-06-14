# Zola Feature Inventory Not Already Available in Calepin

Items with direct Calepin equivalents have been removed. This keeps Zola features
that are still absent as built-in Calepin static-site-generator features.

- [ ] link checker
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
  - Supports language-specific taxonomies.
  - Allows taxonomy rendering to be disabled with `render = false`.
  - Does not slugify taxonomy names.
  - Slugifies taxonomy terms by default.
  - Treats taxonomy terms as case-insensitive when they produce the same slug.
  - Supports `taxonomy_root` URL prefixes.

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

## Roadmap notes (Typst eval)

Calepin could eventually replace its current multi-pass `typst query` flow with `typst eval --in` for structured document introspection.

The current flow uses three query passes for:

- setup and website metadata (`<calepin-config>`, `<website-metadata>`),
- executable chunk discovery (raw blocks, fence labels, `<calepin-chunk>`),
- page-sync anchors (`<calepin-page>`).

A structured single-call replacement could look like:

```sh
typst query input.typ '<calepin-chunk>' ...
typst eval --in input.typ 'query(<calepin-chunk>)' --format=json ...
```

For example:

```typ
(
  setup: query(<calepin-config>).map(it => it.value),
  page_meta: query(<website-metadata>).first().value,
  chunks: query(raw.where(block: true).or(<calepin-fence-label>).or(<calepin-chunk>)),
)
```

This could also simplify page sync by shaping results in Typst:

```typ
query(<calepin-page>).map(it => (
  label: it.value.label,
  page: it.location().page(),
))
```

Proposed migration steps:

1. Add a `typst_eval` helper alongside the existing `typst_query` helper.
2. Use `eval` only when the configured Typst executable supports it.
3. Migrate page-sync first.
4. Migrate setup/page metadata and chunk discovery to a single structured eval call.
5. Keep `typst query` as fallback until Typst 0.15+ is the minimum supported version.
