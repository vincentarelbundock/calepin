#import "/.calepin/calepin.typ" as calepin
#show: calepin.document

#set document(title: [Tags and taxonomies])
#metadata((title: "Tags", tags: ("websites", "metadata", "navigation"))) <website-metadata>

#title()

A taxonomy groups pages by shared metadata such as tags, categories, authors, or topics. You can build one without special configuration: add an array to each page's `<website-metadata>`, then filter the entries returned by `calepin.pages()`.

For example, documentation pages on this site declare tags alongside their other metadata:

```typ
#metadata((
  title: "Pages",
  tags: ("websites", "metadata"),
)) <website-metadata>
```

Create a regular page for the taxonomy index. The following generic function collects the values of any array-valued metadata field, then lists the matching pages under each value:

```typ
#import "/.calepin/calepin.typ" as calepin
#show: calepin.document

#let taxonomy(field) = {
  let pages = calepin.pages()
    .filter(page => page.meta.at(field, default: ()) != ())
  let terms = pages
    .map(page => page.meta.at(field))
    .flatten()
    .dedup()
    .sorted(key: term => lower(term))

  for term in terms {
    heading(term, level: 2)
    for page in pages.filter(
      page => term in page.meta.at(field)
    ) [
      - #link(page.href)[#page.title]
    ]
  }
}

#taxonomy("tags")
```

This approach keeps taxonomies ordinary Typst code. You control the page's route and presentation, and the same function works for fields such as `categories` or `authors`. Add sorting, counts, summaries, or pagination using the usual Typst array methods.

= Tags used in this documentation

#let taxonomy(field) = {
  let pages = calepin.pages()
    .filter(page => page.meta.at(field, default: ()) != ())
  let terms = pages
    .map(page => page.meta.at(field))
    .flatten()
    .dedup()
    .sorted(key: term => lower(term))

  for term in terms {
    heading(term, level: 2)
    for page in pages.filter(
      page => term in page.meta.at(field)
    ) [
      - #link(page.href)[#page.title]
    ]
  }
}

#taxonomy("tags")
