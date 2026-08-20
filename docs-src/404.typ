#set document(title: [Page not found])

#let target = sys.inputs.at("calepin-target", default: "paged")
#let base-url = sys.inputs.at("calepin-base-url", default: none)

#let site-link(path) = {
  if base-url == none {
    path
  } else {
    base-url.trim(regex("/+$")) + "/" + path
  }
}

#if target == "html" [
  #html.elem("section", attrs: (class: "calepin-website-404"))[
    #html.elem("p", attrs: (class: "calepin-website-404-code", "aria-hidden": "true"))[404]
    #html.elem("h1", attrs: (class: "calepin-website-404-title"))[Page not found]
    #html.elem("p", attrs: (class: "calepin-website-404-lede"))[
      Sorry, the page you are looking for does not exist or may have moved.
    ]
    #html.elem("p", attrs: (class: "calepin-website-404-actions"))[
      #html.elem("a", attrs: (href: site-link("index.html"), role: "button"))[Back to home]
    ]
    #html.elem("nav", attrs: (class: "calepin-website-404-links", "aria-label": "Helpful links"))[
      #html.elem("a", attrs: (href: site-link("index.html")))[Home]
      #html.elem("a", attrs: (href: site-link("reference/cli.html")))[CLI reference]
      #html.elem("a", attrs: (href: site-link("notebooks/diagrams.html")))[Example diagrams]
    ]
  ]
] else [
  align(center + horizon)[
    #text(size: 5em, weight: "bold")[404]
    #v(0.1em)
    #text(size: 1.5em, weight: "bold")[Page not found]
    #v(0.4em)
    Sorry, the page you are looking for does not exist or may have moved.
  ]
]
