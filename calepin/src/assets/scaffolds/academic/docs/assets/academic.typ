#let _html() = sys.inputs.at("calepin-target", default: "paged") == "html"
#let _pages-index-path = sys.inputs.at("calepin-pages", default: "")
#let _current-page-href = sys.inputs.at("calepin-current-href", default: "")

#set document(title: [Academic widgets])

#metadata((pdf: false)) <website-metadata>

#let _site-root-prefix() = {
  let depth = _current-page-href.split("/").filter(part => part != "").len() - 1
  if depth <= 0 { "" } else { "../" * depth }
}

#let _pages() = {
  if _pages-index-path == "" { return () }
  let prefix = _site-root-prefix()
  json(_pages-index-path).map(entry => {
    let entry = entry
    if type(entry.at("href", default: none)) == str {
      entry.insert("href", prefix + entry.href)
    }
    if type(entry.at("pdf", default: none)) == str {
      entry.insert("pdf", prefix + entry.pdf)
    }
    entry
  })
}

#let _div(class, body) = {
  if _html() {
    html.elem("div", attrs: (class: class))[#body]
  } else {
    body
  }
}

#let _p(class, body) = {
  if _html() {
    html.elem("p", attrs: (class: class))[#body]
  } else {
    par(body)
  }
}

#let _link-button(label, url) = {
  if url == none or url == "" {
    none
  } else if _html() {
    html.elem("a", attrs: (class: "academic-button", href: url))[#label]
  } else {
    link(url)[#label]
  }
}

#let _meta-line(parts) = {
  let parts = parts.filter(part => part != none and part != "")
  if parts.len() == 0 { "" } else { parts.join(" · ") }
}

#let profile(
  name,
  title: none,
  affiliation: none,
  photo: none,
  links: (),
  body,
) = _div("academic-profile")[
  #_div("academic-profile-text")[
    #if _html() {
      html.elem("h1")[#name]
    } else {
      heading(level: 1)[#name]
    }
    #if title != none { _p("academic-profile-title")[#title] }
    #if affiliation != none { _p("academic-profile-affiliation")[#affiliation] }
    #body
    #if links.len() > 0 {
      _div("academic-link-row")[
        #for item in links {
          _link-button(item.at("label"), item.at("url"))
          [ ]
        }
      ]
    }
  ]
  #if photo != none {
    if _html() {
      html.elem("img", attrs: (class: "academic-profile-photo", src: photo, alt: name))
    }
  }
]

#let section(title, more: none, body) = _div("academic-section")[
  #_div("academic-section-header")[
    #if _html() { html.elem("h2")[#title] } else { heading(level: 2)[#title] }
    #if more != none { link(more.at("url"))[#more.at("label", default: "More")] }
  ]
  #body
]

#let panels(..items) = _div("academic-two-column")[
  #for item in items.pos() {
    _div("academic-panel")[#item]
  }
]

#let _entry(page) = {
  let meta = page.meta
  let venue = meta.at("venue", default: none)
  let year = meta.at("year", default: meta.at("date", default: ""))
  let authors = meta.at("authors", default: none)
  let event = meta.at("event", default: none)
  let term = meta.at("term", default: none)
  let summary = meta.at("summary", default: meta.at("abstract", default: none))
  let line = _meta-line((authors, venue, event, term, year))
  _div("academic-entry")[
    #_p("academic-entry-title")[#link(page.href)[#page.title]]
    #if line != "" { _p("academic-entry-meta")[#line] }
    #if summary != none { _p("academic-entry-summary")[#summary] }
    #_div("academic-entry-actions")[
      #for button in (
        (key: "url_pdf", label: "PDF"),
        (key: "doi", label: "DOI"),
        (key: "url_code", label: "Code"),
        (key: "url_data", label: "Data"),
        (key: "url_slides", label: "Slides"),
        (key: "url_video", label: "Video"),
        (key: "url_bibtex", label: "Cite"),
      ) {
        let url = meta.at(button.key, default: none)
        if button.key == "doi" and url != none and not url.starts-with("http") {
          url = "https://doi.org/" + url
        }
        _link-button(button.label, url)
        [ ]
      }
    ]
  ]
}

#let listing(
  title,
  kind: none,
  featured: none,
  count: none,
  more: none,
) = {
  let pages = _pages()
  if kind != none {
    pages = pages.filter(p => p.meta.at("kind", default: "") == kind)
  }
  if featured != none {
    pages = pages.filter(p => p.meta.at("featured", default: false) == featured)
  }
  pages = pages.sorted(key: p => p.meta.at("date", default: "")).rev()
  if count != none {
    pages = pages.slice(0, calc.min(count, pages.len()))
  }
  section(title, more: more)[
    #_div("academic-list")[
      #if pages.len() == 0 [
        No entries yet.
      ] else {
        for page in pages {
          _entry(page)
        }
      }
    ]
  ]
}

#let selected-publications(count: 5) = listing(
  "Selected publications",
  kind: "publication",
  featured: true,
  count: count,
  more: (label: "All publications", url: "publications/index.html"),
)

#let recent-posts(count: 3) = listing(
  "Recent posts",
  kind: "post",
  count: count,
  more: (label: "All posts", url: "posts/index.html"),
)
