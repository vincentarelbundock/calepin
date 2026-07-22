// Website pages index, provided by `calepin compile` during website builds.
#let _pages-index-path = sys.inputs.at("calepin-pages", default: "")
#let _current-page-href = sys.inputs.at("calepin-current-href", default: "")

// Relative URL prefix from the current page back to the site root.
#let _site-root-prefix() = {
  let depth = _current-page-href.split("/").filter(part => part != "").len() - 1
  if depth <= 0 { "" } else { "../" * depth }
}

// Returns one entry per page of the website: a dictionary with `path` (source
// file), `href` (link to the page, relative to the current page), `title`
// (resolved display title), `pdf` (link to the PDF twin, or none), and `meta`
// (the page's raw `<website-metadata>` dictionary, verbatim). Returns an
// empty array outside website builds.
#let _prefix-page-entry(entry, prefix) = {
  let entry = entry
  if type(entry.at("href", default: none)) == str {
    entry.insert("href", prefix + entry.href)
  }
  if type(entry.at("pdf", default: none)) == str {
    entry.insert("pdf", prefix + entry.pdf)
  }
  if type(entry.at("translations", default: none)) == dictionary {
    let translations = (:)
    for (language, href) in entry.translations {
      translations.insert(language, if type(href) == str { prefix + href } else { href })
    }
    entry.insert("translations", translations)
  }
  entry
}

#let pages() = {
  if _pages-index-path == "" { return () }
  let prefix = _site-root-prefix()
  json(_pages-index-path).map(entry => _prefix-page-entry(entry, prefix))
}
