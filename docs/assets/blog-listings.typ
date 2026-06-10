// Standalone helper for rendering article listings from an explicit post list
// or a JSON/TOML manifest. Typst can read files but cannot enumerate directory
// entries, so the directory scan must happen outside Typst or be represented as
// a manifest.

#let _clean(value) = {
  if value == none {
    none
  } else {
    let value = str(value).trim()
    if value == "" { none } else { value }
  }
}

#let _get(item, key, default: none) = {
  if type(item) == dictionary {
    item.at(key, default: default)
  } else {
    default
  }
}

#let post(
  path,
  title,
  date: none,
  description: none,
  tags: (),
  image: none,
  draft: false,
  href: none,
) = (
  path: path,
  href: if href == none { path.replace(regex("\.typ$"), ".html") } else { href },
  title: title,
  date: _clean(date),
  description: _clean(description),
  tags: tags,
  image: _clean(image),
  draft: draft,
)

#let _manifest-posts(value) = {
  if type(value) == array {
    value
  } else if type(value) == dictionary and value.at("posts", default: none) != none {
    value.posts
  } else {
    panic("blog listings manifest must be an array or a dictionary with a `posts` array")
  }
}

#let _read-posts(source) = {
  if type(source) == array {
    source
  } else if type(source) == str and source.ends-with(".json") {
    _manifest-posts(json(source))
  } else if type(source) == str and source.ends-with(".toml") {
    _manifest-posts(toml(source))
  } else if type(source) == str {
    panic("blog listings source must be a .json or .toml manifest path")
  } else {
    panic("blog listings source must be an array or manifest path")
  }
}

#let _normalize(item) = {
  let path = _get(item, "path")
  if path == none {
    panic("blog listing entries require `path`")
  }
  let title = _get(item, "title", default: path)
  let href = _get(item, "href", default: path.replace(regex("\.typ$"), ".html"))
  (
    path: path,
    href: href,
    title: title,
    date: _clean(_get(item, "date")),
    description: _clean(_get(item, "description")),
    tags: _get(item, "tags", default: ()),
    image: _clean(_get(item, "image")),
    draft: _get(item, "draft", default: false),
  )
}

#let _wildcard-match(pattern, value) = {
  if pattern == none or pattern == "" {
    return true
  }
  if pattern == value {
    return true
  }
  let parts = pattern.split("*")
  let pos = 0
  let rest-parts = parts
  if not pattern.starts-with("*") {
    let first = parts.first()
    if not value.starts-with(first) {
      return false
    }
    pos = first.len()
    rest-parts = parts.slice(1)
  }
  for part in rest-parts {
    if part == "" {
      continue
    }
    let found = value.slice(pos).position(part)
    if found == none {
      return false
    }
    pos += found + part.len()
  }
  if not pattern.ends-with("*") {
    let last = parts.last()
    if last != "" and not value.ends-with(last) {
      return false
    }
  }
  true
}

#let _sort-key(item, sort) = {
  if sort == "date-asc" or sort == "date-desc" {
    if item.date == none { "" } else { item.date }
  } else if sort == "title-asc" or sort == "title-desc" {
    str(item.title)
  } else {
    item.path
  }
}

#let _sort(posts, sort) = {
  if sort == none or sort == "none" {
    posts
  } else {
    let sorted = posts.sorted(key: item => _sort-key(item, sort))
    if sort == "date-desc" or sort == "title-desc" {
      sorted.rev()
    } else {
      sorted
    }
  }
}

#let listing-entry(item) = block(width: 100%, below: 0.9em)[
  #link(item.href)[*#item.title*]
  #if item.date != none [
    #linebreak()
    #text(size: 0.85em, fill: luma(45%))[#item.date]
  ]
  #if item.description != none [
    #parbreak()
    #item.description
  ]
]

#let listings(
  source,
  pattern: none,
  include-drafts: false,
  sort: "date-desc",
  limit: none,
  empty: [No articles found.],
  render: listing-entry,
) = {
  let items = _read-posts(source)
    .map(_normalize)
    .filter(item => include-drafts or item.draft != true)
    .filter(item => _wildcard-match(pattern, item.path))
  items = _sort(items, sort)
  if limit != none {
    items = items.slice(0, calc.min(limit, items.len()))
  }
  if items.len() == 0 {
    empty
  } else {
    for item in items {
      render(item)
    }
  }
}
