#let _meta(page, key, default: none) = {
  let meta = page.at("meta", default: (:))
  if type(meta) == dictionary {
    meta.at(key, default: default)
  } else {
    default
  }
}

#let _date(page) = _meta(page, "date", default: "")

#let margin-note(body) = {
  if sys.inputs.at("calepin-target", default: "paged") == "html" {
    html.elem("span", attrs: (class: "marginnote"), body)
  } else {
    body
  }
}

#let entries(source, kind: none, language: none) = {
  source
    .filter(page => kind == none or _meta(page, "kind") == kind)
    .filter(page => language == none or page.at("language", default: none) == language)
    .filter(page => _meta(page, "draft", default: false) != true)
    .sorted(key: _date)
    .rev()
}

#let listing(title, source, kind: "post", language: none) = {
  let items = entries(source, kind: kind, language: language)
  if title != none {
    heading(level: 2)[#title]
  }
  if items.len() == 0 {
    [No posts yet.]
  } else {
    for page in items {
      block(width: 100%, below: 1em)[
        #link(page.href)[*#page.title*]
        #let date = _meta(page, "date")
        #if date != none [
          #linebreak()
          #text(size: 0.85em, fill: luma(45%))[#date]
        ]
        #let summary = _meta(page, "summary")
        #if summary != none [
          #parbreak()
          #summary
        ]
      ]
    }
  }
}
