#let target() = sys.inputs.at("calepin-target", default: "paged")

#let asset-path(path) = if path.starts-with("/") { path } else { "/" + path }

#let html-only(body) = {
  if target() == "html" {
    body
  }
}

#let hero(body) = {
  if target() == "html" {
    html.elem("section", attrs: (class: "hero"))[
      #body
    ]
  } else if target() == "paged" {
    align(center)[
      #body
    ]
    v(1.2em)
  }
}

#let feature-card(title, body) = {
  if target() == "html" {
    html.elem("article", attrs: (class: "calepin-website-feature-card"))[
      #html.elem("h4")[#title]
      #html.elem("p")[#body]
    ]
  } else {
    box(
      width: 100%,
      height: 8.2em,
      inset: 0.9em,
      radius: 4pt,
      stroke: rgb("#dfe3e8"),
      fill: rgb("#f8fafc"),
    )[
      #text(weight: "semibold")[#title]
      #v(0.35em)
      #body
    ]
  }
}

#let feature-card-grid(..cards) = {
  let cards = cards.pos()
  if target() == "html" {
    html.elem("section", attrs: (class: "calepin-website-features"))[
      #for card in cards {
        card
      }
    ]
  } else {
    grid(
      columns: (1fr, 1fr),
      gutter: 0.9em,
      ..cards,
    )
  }
}
