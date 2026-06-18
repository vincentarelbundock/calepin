#import "/.calepin/calepin.typ" as calepin

#set document(title: [Calepin])

#metadata((
  layout: "layouts/landing.html",
  pdf: true,
)) <website-metadata>

// hero
#let hero() = calepin.elements.target(
  html: () => [
    #html.elem("section", attrs: (class: "landing-hero"))[
      #html.elem("img", attrs: (
        src: "assets/logo_long.svg",
        alt: "Calepin",
        class: "landing-hero-wordmark",
        style: "display: block; width: 14em; max-width: 72vw; margin-inline: auto; color: currentColor;",
        "data-inline-svg": "1",
      ))[]
      #text(size: 1.35em, weight: "bold")[Computational Notebooks and Static Websites in Typst]
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
  ],
  paged: () => [
    #align(center)[
      #image("/assets/logo_long.svg", width: 30%)
      #text(size: 1.35em, weight: "bold")[Computational Notebooks and Static Websites in Typst]
    ]
  ],
)

#hero()


// features
#let notebook-card() = [
  #calepin.elements.card(
    class: "landing-feature-card",
    style: "height: 100%; width: 100%; flex: 1;",
  )[
    = Computational notebooks

    Write in Typst, execute code, and see results inline. Perfect for data analysis, reports, and publications with reproducible outputs, in the spirit of literate programming.

    - Typst-native authoring
    - Executable code chunks
    - Inline results and plots
    - Python, R, Julia, Bash, and all Jupyter kernels
    - Export to HTML and PDF

    #calepin.elements.lightbox-image(
      "calepin-notebook-screenshot-dialog",
      "assets/screenshot_notebook.png",
      "Calepin notebook screenshot",
      open-label: "Open Calepin notebook screenshot",
    )
  ]
]

#let website-card() = [
  #calepin.elements.card(
    class: "landing-feature-card",
    style: "height: 100%; width: 100%; flex: 1;",
  )[
    = Static website generator

    Build multi-page websites from Typst with ease. Great for docs, portfolios, and project sites.

    - Themes, templates and layouts
    - Reusable web components
    - Fast incremental builds
    - Live preview
    - Multi-lingual support
    - Blog listings and feeds
    - Search
    - ...and much more!

    #calepin.elements.lightbox-image(
      "calepin-website-screenshot-dialog",
      "assets/screenshot_website.png",
      "Calepin website screenshot",
      open-label: "Open Calepin website screenshot",
    )
  ]
]

#calepin.elements.columns(
  html-attrs: (style: "align-items: stretch;"),
  wrap: false,
  notebook-card(),
  website-card(),
)

= Pure Typst

Write notebooks in pure Typst, a simple, consistent, powerful, and elegant typesetting system. There is no special file format; notebooks and websites are just standard `.typ` documents. You do not need to "declare" your markup as Typst using special "fences." _Calepin_ does not push your text through a lossy Pandoc translation layer, and you do not need to learn yet another _ad hoc_ markdown variant.

#let editor-text-content() = [
  = Editor integration

  Write, execute, preview, and publish Calepin documents without leaving your editor.
  Install the Calepin extension from the VS Code Marketplace, or from Open VSX for Cursor,
  Positron, and other VSX-compatible editors.
]

#let editor-text() = calepin.elements.target(
  html: () => html.elem("section")[#editor-text-content()],
  paged: () => editor-text-content(),
)

#let editor-video() = [
  #calepin.elements.lightbox-video(
    "calepin-video-dialog",
    "assets/calepin_vscode.mp4",
    poster: "assets/calepin_vscode-thumb.png",
    open-label: "Open Calepin editor preview video",
  )
]

#calepin.elements.columns(
  html-attrs: (style: "align-items: stretch;"),
  wrap: false,
  editor-text(),
  editor-video(),
)

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

= Etymology and pronunciation

_Calepin_ comes from the French word for "notebook." You should, of course, feel free to say it however you like. The closest English sounds might be "cal-huh-pan," with "cal" as in "calendar," and "pan" like the cooking instrument. (The French would pronounce that last syllable more nasally, with a silent "n".)
