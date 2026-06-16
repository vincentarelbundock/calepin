#import "@preview/calepin:0.0.1" as calepin

#let _meta(page, key, default: none) = {
  let meta = page.at("meta", default: (:))
  if type(meta) == dictionary {
    meta.at(key, default: default)
  } else {
    default
  }
}

#let _date(page) = _meta(page, "date", default: "")

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
    let cells = ()
    for (index, page) in items.enumerate() {
      let date = _meta(page, "date", default: "")
      cells.push(html.elem("span", attrs: (style: "white-space: nowrap"))[
        #text(size: 0.9em, fill: luma(45%))[#date]
      ])
      cells.push(link(page.href)[#page.title])
      if index < items.len() - 1 {
        cells.push(table.hline(stroke: 0.5pt + luma(80%)))
      }
    }
    table(
      columns: (auto, 1fr),
      stroke: none,
      inset: (x: 0pt, y: 0.45em),
      ..cells,
    )
  }
}

#set document(title: [Blog])
#metadata((title: "Blog", translation_key: "blog")) <website-metadata>

#title()

All posts, sorted from newest to oldest.

#listing("Posts", calepin.pages(), kind: "post", language: "en")
