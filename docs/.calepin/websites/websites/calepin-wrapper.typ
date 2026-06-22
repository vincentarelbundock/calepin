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

#show raw.where(block: true): set text(size: .8em)

#show raw.where(block: true): it => {
  if it.theme != auto {
    it
  } else if it.lang != none and _raw-chunk-langs.contains(it.lang) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

#import "/.calepin/calepin.typ" as calepin
#import "/.calepin/calepin.typ" as calepin_runtime
#set document(title: [Static website generator])
#metadata((title: "Overview")) <website-metadata>

#title()

_Calepin_ can turn a directory of Typst documents into a website. In fact, the website you are reading now was entirely generated with _Calepin_.

= New website

Use this command to scaffold an example site:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin new website\n", block: true, lang: "sh"))

This creates a website source directory with enough structure to exercise the bundled themes:

- `calepin_website/calepin.toml`: the site configuration file, where you set the title, base URL, navigation, theme, and output options.
- `calepin_website/index.typ`: the home page source file, which builds to `index.html`.
- `calepin_website/404.typ`: the not-found page source file, used by static hosts such as GitHub Pages for missing routes.
- `calepin_website/about.typ`, `calepin_website/guide/*.typ`, and `calepin_website/fr/*.typ`: regular pages in two languages, with site menu and sidebar entries.
- `calepin_website/blog.typ` and `calepin_website/posts/*.typ`: a small blog index and post source files using `calepin.pages()`.

The scaffold uses the `calepin` theme by default. Use `--theme` to start from
another built-in theme:

```sh
calepin new website --theme academic
```

= Build

Compile a website directory with the same `compile` command used for a single Typst document:

```sh
calepin compile my_site/
```

This compiles the website in place: source files are read from `my_site/`, and generated outputs are written back into `my_site/`.

To compile the same source directory somewhere else, pass an output directory as the second path:

```sh
calepin compile my_site/ public/
```

When the input path is a directory, _Calepin_ looks for `calepin.toml` at the root of that directory. An explicit `--config <path>` supersedes the automatic lookup; if no config is found either way, the build fails.

By default, website pages render to HTML. Configure PDF output and site settings in #link("configuration.html")[Site configuration].

= Serve

Serve the built site locally with:

```sh
calepin serve my_site/
```

This is useful for a quick preview after `calepin compile`. By default, _Calepin_ uses `127.0.0.1` and the first available port from 8000.

Use `--host` and `--port` when you need a specific address:

```sh
calepin serve my_site/ --host 127.0.0.1 --port 8001
```

Add `--open` to launch the served site in your default browser:

```sh
calepin serve my_site/ --open
```

= Watch

During development, watch the website source directory for changes, and re-compile automatically:

```sh
calepin watch my_site/ my_site/
```

As with `compile`, the first positional argument is the source directory and the optional second positional argument is the output directory. The first build renders the whole site. After that, _Calepin_ hashes each edited page and rebuilds only the pages whose content actually changed; files that were touched but not modified are skipped.

Changes that can affect more than one page trigger a full rebuild instead. This includes edits to `calepin.toml`, themes, or assets, pages that are added, removed, or renamed, and edits that change the site navigation or page metadata.

Add `--serve` to run the local server while watching; it uses the same `--host`, `--port`, and `--open` options as `calepin serve`.

```sh
calepin watch my_site/ my_site/ --serve --open
```

= Features

- Directory builds from Typst source files.
- In-place or separate output directories.
- Sidebar and site menu navigation from configuration or discovered pages.
- HTML output with optional PDF twins.
- Page metadata and `calepin.pages()` for listings and indexes.
- Multilingual site navigation.
- Static file copying for assets, downloads, and host-specific files.
- Generated `sitemap.xml` and `robots.txt`, with template overrides for `robots.txt`.
- Optional Pagefind search index generation, with cached no-op rebuilds.
- Optional HTML minification, including inline CSS and JavaScript.
- Local serving and incremental watch builds.
