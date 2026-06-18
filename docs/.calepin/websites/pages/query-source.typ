#set document(title: [Pages and metadata])
#metadata((title: "Pages")) <website-metadata>

#title()

= Include or exclude pages

Use `[pages]` for Typst pages that should be built but should not appear in navigation, such as blog posts or legal pages:

```toml
[pages]
include = ["blog/*.typ", "legal/privacy.typ"]
exclude = ["drafts/**"]
```

Put the page title in the document and keep website metadata for fields used by listings, routing, and output options:

```typ
#set document(title: [First post])

#metadata((
  date: "2026-06-10",
  tags: ("release", "website"),
)) <website-metadata>

#title()
```

`index.typ` and `404.typ` are always built when present. If `404.typ` exists, _Calepin_ writes `404.html`; if PDF rendering is enabled for that page, it also writes `404.pdf`.

= Page metadata

Add arbitrary page metadata with Typst's `#metadata` function and the `<website-metadata>` label. _Calepin_ reads this dictionary while building the site and attaches it to the page entry returned by `calepin.pages()`.

```typ
#set document(title: [First post])

#metadata((
  date: "2026-06-10",
  tags: ("release", "website"),
  author: "Ada Lovelace",
  summary: "A short release note for the new website.",
  draft: false,
)) <website-metadata>

#title()
```

Use `calepin.pages()` to get structured information about every built page, including its metadata, and process it with normal Typst functions and methods. This is useful for lists, indexes, feeds, publication pages, course pages, and any page that needs to organize other pages in the site.

```typ
#import "/.calepin/calepin.typ" as calepin

#let posts = calepin.pages()
  .filter(p => p.path.starts-with("blog/"))
  .filter(p => not p.meta.at("draft", default: false))
  .sorted(key: p => p.meta.at("date", default: ""))
  .rev()

#for post in posts [
  - #link(post.href)[#post.title] \
    #post.meta.at("summary", default: [])
]
```

`calepin.pages()` returns one dictionary per built page, excluding `404.typ`. _Calepin_ creates these entry fields:

- `path`: source path relative to the website root
- `href`: rendered HTML path relative to the current page
- `title`: resolved page title
- `language`: language code, or `none` when languages are not configured
- `translation_key`: resolved key used to connect translated pages
- `translations`: matching pages in other languages, or an empty dictionary
- `pdf`: PDF path, or `none` when the page has no PDF output
- `meta`: the page's `<website-metadata>` dictionary, or an empty dictionary

Only `meta` comes from the page's `#metadata` value. _Calepin_ interprets a few optional metadata keys: `title`, `pdf`, `translation_key`, `slug`, and `url`. All other keys are left untouched for your own Typst code.

There is no required schema for custom metadata. Common keys include `date`, `tags`, `author`, `authors`, `category`, `venue`, `summary`, and `draft`, but you can use any key your page list or template expects. Since `calepin.pages()` returns a Typst array of dictionaries, you can use Typst functions and methods such as `filter`, `map`, `sorted`, `rev`, `at`, and `contains` to select and format the pages you need.
