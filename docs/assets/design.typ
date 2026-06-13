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

#let _dialog-close(label) = html.elem("button", attrs: (
  rel: "prev",
  type: "button",
  "data-close-dialog": "",
  "aria-label": label,
))

#let screenshot-lightbox(
  id,
  src,
  alt,
  open-label: "Open screenshot preview",
  close-label: "Close screenshot preview",
) = {
  if target() == "html" {
    html.elem("div", attrs: (class: "calepin-screenshot-block"))[
      #html.elem("button", attrs: (
        class: "calepin-screenshot-thumb",
        type: "button",
        "data-lightbox-dialog": id,
        "aria-label": open-label,
      ))[
        #html.elem("img", attrs: (
          src: src,
          alt: alt,
          class: "calepin-screenshot-thumb__media",
        ))
        #html.elem("span", attrs: (
          class: "calepin-screenshot-thumb__zoom",
          "aria-hidden": "true",
        ))[↗]
      ]
    ]
    html.elem("dialog", attrs: (id: id, class: "calepin-screenshot-dialog"))[
      #html.elem("article")[
        #html.elem("header")[
          #_dialog-close(close-label)
        ]
        #html.elem("img", attrs: (
          class: "calepin-screenshot-dialog__media",
          src: src,
          alt: alt,
        ))
      ]
    ]
  }
}

#let video-lightbox(
  id,
  src,
  poster: none,
  open-label: "Open video preview",
  close-label: "Close video preview",
) = {
  let thumb-attrs = (
    class: "calepin-video-thumb__media",
    src: src,
    muted: "",
    playsinline: "",
    preload: "metadata",
  )
  if poster != none {
    thumb-attrs.insert("poster", poster)
  }

  if target() == "html" {
    html.elem("div", attrs: (class: "calepin-video-block"))[
      #html.elem("button", attrs: (
        class: "calepin-video-thumb",
        type: "button",
        "data-video-dialog": id,
        "aria-label": open-label,
      ))[
        #html.elem("video", attrs: thumb-attrs)
        #html.elem("span", attrs: (
          class: "calepin-video-thumb__play",
          "aria-hidden": "true",
        ))[▶]
      ]
    ]
    html.elem("dialog", attrs: (id: id, class: "calepin-video-dialog"))[
      #html.elem("article")[
        #html.elem("header")[
          #_dialog-close(close-label)
        ]
        #html.elem("video", attrs: (
          class: "calepin-video-dialog__media",
          src: src,
          muted: "",
          autoplay: "",
          controls: "",
          playsinline: "",
          preload: "metadata",
        ))
      ]
    ]
  }
}
