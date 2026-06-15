#let html(
  title: none,
  lang: "en",
  body,
) = {
  let calepin-target = sys.inputs.at("calepin-target", default: "paged")
  if calepin-target == "html" {
    std.html.elem("html", attrs: (lang: lang))[
      #std.html.elem("head")[
        #std.html.elem("meta", attrs: (charset: "utf-8"))
        #std.html.elem("meta", attrs: (
          name: "viewport",
          content: "width=device-width, initial-scale=1",
        ))

        #if title != none {
          std.html.elem("title")[#title]
        }
      ]

      #std.html.elem("body")[
        #std.html.elem("main", attrs: (class: "container"))[
          #body
        ]
      ]
    ]
  } else {
    body
  }
}
