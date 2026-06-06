#let target = sys.inputs.at("calepin-target", default: "paged")

#if target == "html" {
  html.elem("section", attrs: (class: "calepin-website-404"))[
    #html.elem("p", attrs: (class: "calepin-website-404-code", "aria-hidden": "true"))[404]
    #html.elem("h1", attrs: (class: "calepin-website-404-title"))[Page not found]
    #html.elem("p", attrs: (class: "calepin-website-404-lede"))[
      Sorry, the page you are looking for does not exist or may have moved.
    ]
    #html.elem("p", attrs: (class: "calepin-website-404-actions"))[
      #html.elem("a", attrs: (href: "index.html", role: "button"))[Back to home]
    ]
    #html.elem("nav", attrs: (class: "calepin-website-404-links", "aria-label": "Helpful links"))[
      #html.elem("a", attrs: (href: "index.html"))[Home]
      #html.elem("a", attrs: (href: "cli.html"))[CLI reference]
      #html.elem("a", attrs: (href: "example.html"))[Example notebook]
    ]
  ]
} else {
  align(center + horizon)[
    #text(size: 5em, weight: "bold")[404]
    #v(0.1em)
    #text(size: 1.5em, weight: "bold")[Page not found]
    #v(0.4em)
    Sorry, the page you are looking for does not exist or may have moved.
  ]
}
