= Navigation and listings

== Sidebar

Navigation can be explicit:

```toml
[sidebar]

[[sidebar.section]]
title = "Guide"

  [[sidebar.section.item]]
  path = "index.typ"
  label = "Home"

  [[sidebar.section.item]]
  path = "usage.typ"
  label = "Usage"
```

Each section can contain any number of `item` entries. An item can point to one file with `path`:

```toml
[[sidebar.section.item]]
path = "install.typ"
label = "Install"
```

An item can also include files with a simple glob:

```toml
[[sidebar.section.item]]
glob = "guide/*.typ"
```

If no sidebar is configured, _Calepin_ builds navigation from `.typ` files in the source directory. Hidden paths are skipped by default. Set `show_hidden = true` under `[sidebar]` to include hidden paths in manual navigation file discovery.

== Page titles

Navigation labels are resolved in this order:

1. the `label` configured on the sidebar item
2. the `title` entry of the page's `<website-metadata>` metadata
3. the file name, with `-` and `_` replaced by spaces

To set a page title in the document itself:

```typ
#metadata((title: "Getting started")) <website-metadata>
```

Page metadata is extracted while the page is preprocessed, from the same staged source used for rendering. Package imports such as `@preview/calepin` are already rewritten at that point, so metadata works in pages that do not compile standalone.

== Page listings

Pages can list other pages of the website, for example to build a blog index. During a website build, every page can call `calepin.pages()`:

```typ
#import "@preview/calepin:0.0.1" as calepin

#for post in calepin.pages()
  .filter(p => p.path.starts-with("blog/"))
  .sorted(key: p => p.meta.at("date", default: ""))
  .rev() [
  - #link(post.href)[#post.title] (#post.meta.at("date", default: ""))
]
```

Each entry is a dictionary:

- `path`: the source file, relative to the website source directory (`blog/first.typ`)
- `href`: a link to the page, already relative to the current page
- `title`: the resolved display title, matching the navigation
- `pdf`: a link to the page's PDF, or `none` when no PDF is rendered
- `meta`: the page's `<website-metadata>` dictionary, verbatim

_Calepin_ interprets only the `title`, `pdf`, and `hidden` keys of page metadata. Everything else in `meta` (dates, tags, authors, categories) is yours to define and filter on. Outside a website build, `calepin.pages()` returns an empty array, so pages compile standalone.

Blog posts usually should not appear in the sidebar. Include them in the build with a sidebar `glob`, then mark each post:

```typ
#metadata((title: "First Post", date: "2026-06-01", hidden: true)) <website-metadata>
```

`hidden: true` removes a page from the navigation while keeping it built, listed in `calepin.pages()`, and present in `sitemap.xml`.

When any page's metadata changes, _Calepin_ rebuilds the whole site during watch so that listing pages never go stale.
