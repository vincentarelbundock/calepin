#import "/.calepin/calepin.typ": *



#let _raw-chunk-langs = ("python", "r", "mermaid", "dot", "tikz", "d2", "sh")
#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("d2", it) }
#show raw.where(block: true, lang: "sh", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("sh", it) }

#show raw.where(block: true, theme: auto): it => {
  if _is-query() {
    it
  } else if _disable-raw-chunk-transforms.get() {
    _html-themed-raw-block(it)
  } else if it.has("lang") and it.lang != none and _raw-chunk-langs.contains(it.lang) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

// Notebook theme
#import "/.calepin/calepin.typ": _html-themed-raw-block, chunk_from_raw_plain

// Body text size, captured below at document-body level. Code blocks are sized
// relative to this rather than to `1em`, which would compound: a literal
// ```typ block is rendered by replacing its source `raw` element, so it renders
// inside Typst's already-reduced raw text context, whereas executed chunks are
// emitted as ordinary calls at body size. Anchoring to the captured body size
// gives both paths a single, matching reduction instead of shrinking twice.
#let _calepin-body-size = std.state("calepin-body-size", 11pt)

#show raw.where(block: true): it => {
  if it.theme != auto {
    context {
      set text(size: _calepin-body-size.get() * 0.8)
      it
    }
  } else if it.lang != none and _raw-chunk-langs.contains(it.lang) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

#context _calepin-body-size.update(text.size)

#import "/.calepin/calepin.typ" as calepin
#import "/.calepin/calepin.typ" as calepin_runtime
#set document(title: [Calepin is now a static website generator])
#metadata((
  title: "Calepin is now a static website generator",
  date: "2026-06-19",
  tags: ("release", "websites", "typst"),
  description: "The second major Calepin feature: building fast, flexible static websites from standard Typst files.",
  pdf: true,
)) <website-metadata>

#title()

Two weeks ago, I introduced _Calepin_, a tool that could turn a standard Typst file into a computational notebook, in the spirit of Quarto or Jupyter. Today, I am pleased to introduce the second *major* feature of _Calepin_: static website generation.

This means that _Calepin_ can now take a directory of ordinary `.typ` files and turn it into a complete website: HTML pages, optional PDF versions, navigation, feeds, search, assets, and all the small files that static hosts expect.

= What is static website generation?

A static website generator builds a site ahead of time. Instead of running a web application on a server for every visitor, it converts source files into plain HTML, CSS, JavaScript, images, and other assets. The result can be served from almost anywhere: GitHub Pages, Netlify, Cloudflare Pages, an S3 bucket, or a simple web server.

Static sites are fast, easy to host, and pleasantly robust. There is no database to maintain, no application server to patch, and no runtime dependency between your writing and your readers. You write the source files, run a build command, and publish the generated directory.

With _Calepin_, the source files are Typst files. This is the important part. The same language you use for a paper, report, lecture note, slide deck, or computational notebook can now be used for a project website or documentation site.

= What can it do?

A _Calepin_ website starts with a directory and a `calepin.toml` file:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin new website my_site/\ncalepin compile my_site/\n", block: true, lang: "sh"))

From there, you can write pages in Typst and let _Calepin_ handle the website machinery. Pages can share a theme, appear in menus and sidebars, expose metadata for listings, and be rendered to HTML, PDF, or both.

This website was built that way. The documentation pages, the landing page, the navigation, the theme, and the feeds all come from Typst sources.

= Best features

- *Pure Typst authoring.* There is no Markdown layer and no Pandoc translation step. Your pages are `.typ` files, so you can use Typst's layout, styling, math, figures, bibliographies, and scripting directly.

- *One tool for notebooks and websites.* A page can be a document, a notebook, or both. You can execute code chunks, capture figures and tables, and publish the results as part of a website.

- *HTML with optional PDF twins.* Website pages render to HTML by default, and any page can also produce a matching PDF. This is useful for documentation, course notes, handouts, reports, and anything readers may want to print or cite.

- *Themes and layouts.* _Calepin_ ships with built-in themes, and themes can be customized or ejected into your project. This keeps the default path simple without hiding the HTML and CSS from people who want control.

- *Navigation from configuration or pages.* Menus and sidebars can be declared in `calepin.toml`, while page metadata and `calepin.pages()` make it easy to build indexes, publication lists, blog archives, course schedules, and custom navigation.

- *Blog-friendly metadata and feeds.* Pages can carry dates, tags, descriptions, authors, drafts, and any other metadata you need. _Calepin_ can generate Atom and RSS feeds for sites that publish updates.

- *Multilingual websites.* Pages can be grouped across languages, and themes can expose translation links so readers can move between versions of the same page.

- *Static assets and host files.* Images, downloads, favicons, `robots.txt`, `sitemap.xml`, and `404.html` fit into the same build. The output is ready for common static hosting services.

- *Fast local development.* `calepin watch` rebuilds changed pages incrementally, and `calepin serve` previews the site locally. Use both together while writing:

#calepin_runtime.chunk_from_raw_plain("sh", raw("  calepin watch my_site/ my_site/ --serve --open\n", block: true, lang: "sh"))

- *Search and minification.* For larger sites, _Calepin_ can build a Pagefind search index and minify the generated HTML, CSS, and JavaScript.

= Why this matters

I like Typst because it is a coherent writing system. It is pleasant for prose, serious enough for technical documents, and programmable when the document needs structure. The notebook feature made Typst useful for computation-heavy documents. Static website generation extends the same idea to publishing.

A _Calepin_ project can now be a research note, a report, a course website, a software manual, a blog, a slide deck, and a computational notebook, all with the same source language and the same command-line tool.

That is the goal: fewer formats, fewer conversions, and more time spent writing.
