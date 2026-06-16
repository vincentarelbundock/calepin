#import "/assets/design.typ": screenshot-lightbox, video-lightbox

#set document(title: [Calepin])

#metadata((
  layout: "layouts/landing.html",
  pdf: false,
)) <website-metadata>

#let checklist(..items) = html.elem("ul", attrs: (class: "landing-checklist"))[
  #for item in items.pos() {
    html.elem("li")[#item]
  }
]

#html.elem("section", attrs: (class: "landing-hero"))[
  #html.elem("img", attrs: (
    src: "assets/logo_long.svg",
    alt: "Calepin",
    class: "landing-hero-wordmark",
    style: "width: 14rem; max-width: 72vw;",
    "data-inline-svg": "1",
  ))[]
  #html.elem("h2", attrs: (style: "max-width: 34rem; margin-top: 1.5rem; font-size: 1.3rem; line-height: 1.22;"))[
    Computational Notebooks and Static Websites in
    #html.elem("span", attrs: (class: "landing-accent"))[typst]
  ]
  #html.elem("div", attrs: (class: "landing-command-row"))[
    #html.elem("code", "calepin compile notebook.typ")
    #html.elem("code", "calepin compile website/")
  ]
  #html.elem("div", attrs: (class: "landing-cta-row"))[
    #html.elem("a", attrs: (
      class: "landing-button landing-button-primary",
      href: "getting-started/install.html",
    ))[Documentation]
  ]
]

#html.elem("section", attrs: (class: "landing-primary-features", id: "features"))[
      #html.elem("article", attrs: (class: "landing-feature-card landing-feature-card-large"))[
        #html.elem("div", attrs: (class: "landing-feature-content"))[
          #html.elem("h2")[Computational notebooks]
          #html.elem("p")[Write in Typst, execute code, and see results inline. Perfect for data analysis, reports, and publications with reproducible outputs, in the spirit of literate programming.]
          #checklist(
            [Typst-native authoring],
            [Executable code chunks],
            [Inline results and plots],
            [Python, R, Julia, Bash, and all Jupyter kernels],
            [Export to HTML and PDF],
          )
        ]
        #screenshot-lightbox(
          "calepin-notebook-screenshot-dialog",
          "assets/screenshot_notebook.png",
          "Calepin notebook screenshot",
          open-label: "Open Calepin notebook screenshot",
        )
        #html.elem("footer")[
          #html.elem("span")[Reproducible reports.]
          #html.elem("strong")[HTML / PDF]
        ]
      ]

      #html.elem("article", attrs: (class: "landing-feature-card landing-feature-card-large"))[
        #html.elem("div", attrs: (class: "landing-feature-content"))[
          #html.elem("h2")[Static website generator]
          #html.elem("p")[Build multi-page websites from Typst with ease. Great for docs, portfolios, and project sites.]
          #checklist(
            [Themes, templates and layouts],
            [Reusable web components],
            [Fast incremental builds],
            [Live preview],
            [Multi-lingual support],
            [Blog listings and feeds],
            [Search],
            [...and much more!],
          )
        ]

        #linebreak()
        #screenshot-lightbox(
          "calepin-website-screenshot-dialog",
          "assets/screenshot_website.png",
          "Calepin website screenshot",
          open-label: "Open Calepin website screenshot",
        )

        #html.elem("footer")[
          #html.elem("span")[Clean, responsive, and search-friendly websites.]
          #html.elem("strong")[HTML]
        ]
      ]
]

= Pure Typst

Write notebooks in pure Typst, a simple, consistent, powerful, and elegant typesetting system. There is no special file format; notebooks and websites are just standard `.typ` documents. You do not need to "declare" your markup as Typst using special "fences." _Calepin_ does not push your text through a lossy Pandoc translation layer, and you do not need to learn yet another _ad hoc_ markdown variant.

#html.elem("section", attrs: (class: "landing-editor-integration"))[
      #html.elem("div", attrs: (class: "landing-editor-copy"))[
        #html.elem("h2")[Editor integration]
        #html.elem("p")[
          Write, execute, preview, and publish Calepin documents without leaving your editor.
          Install the Calepin extension from the VS Code Marketplace, or from Open VSX for Cursor,
          Positron, and other VSX-compatible editors.
        ]
      ]
      #html.elem("div", attrs: (class: "landing-editor-video"))[
        #video-lightbox(
          "calepin-video-dialog",
          "assets/calepin_vscode.mp4",
          poster: "assets/calepin_vscode-thumb.png",
          open-label: "Open Calepin editor preview video",
        )
      ]
]

= A simple computational notebook

````typ
#import "@preview/calepin:0.0.1" as calepin

#calepin.setup(
  echo: true,
  results: "verbatim",
)

#let py = calepin.inline.with("python")

```python
x = 41
print(x + 1)
```

Variables are persistent across chunks:

```python
print(x + 2)
```

The inline answer is #py[`print(40 + 2)`].
````
