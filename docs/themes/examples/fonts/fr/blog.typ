#import "/.calepin/calepin.typ" as calepin

#let _meta(page, key, default: none) = {
  let meta = page.at("meta", default: (:))
  if type(meta) == dictionary {
    meta.at(key, default: default)
  } else {
    default
  }
}

#let _date(page) = _meta(page, "date", default: "")

#let _listing-thumbnail(page) = {
  let thumbnail = _meta(page, "thumbnail", default: none)
  if thumbnail != none {
    thumbnail
  } else {
    let path = page.at("path", default: none)
    if type(path) != str or not path.ends-with(".typ") {
      none
    } else {
      let stem = path.slice(0, path.len() - 4)
      let results-path = "/.calepin/" + stem + "/results.json"
      let results-doc = json(results-path)
      let image-paths = ()
      for (_, chunk) in results-doc.at("chunks", default: (:)).pairs() {
        for item in chunk.at("items", default: ()) {
          let data = item.at("data", default: none)
          if type(data) == dictionary {
            for (mime, value) in data.pairs() {
              if type(mime) == str and mime.starts-with("image/") {
                let image-path = value.at("path", default: none)
                if type(image-path) == str {
                  image-paths.push(image-path)
                }
              }
            }
          }
        }
      }
      if image-paths.len() == 0 {
        none
      } else {
        image-paths.first()
      }
    }
  }
}

#let _listing-asset(path) = {
  if type(path) != str {
    none
  } else if path.starts-with("http://") or path.starts-with("https://") {
    path
  } else if path.starts-with("/") {
    calepin._resolve-asset-path(path)
  } else {
    calepin._resolve-asset-path("/" + path)
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
    [Aucun billet.]
  } else {
    let cells = ()
    for (index, page) in items.enumerate() {
      let thumbnail = _listing-thumbnail(page)
      let date = _meta(page, "date", default: "")
      if thumbnail != none {
        cells.push(image(_listing-asset(thumbnail), width: 1.2cm))
      } else {
        cells.push([])
      }
      cells.push(text(size: 0.9em, fill: luma(45%))[#date])
      cells.push(link(page.href)[#page.title])
      if index < items.len() - 1 {
        cells.push(table.hline(stroke: 0.5pt + luma(80%)))
      }
    }
    table(
      columns: (1.8cm, auto, 1fr),
      stroke: none,
      inset: (x: 0pt, y: 0.45em),
      ..cells,
    )
  }
}

#set document(title: [Blogue])
#metadata((title: "Blogue", translation_key: "blog", slug: "blogue")) <website-metadata>

#title()

Tous les billets, du plus récent au plus ancien.

#listing("Billets", calepin.pages(), kind: "post", language: "fr")
