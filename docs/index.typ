#import "/.calepin/calepin.typ" as calepin
#show: calepin.document

#set document(title: [Calepin])

#metadata((
  layout: "layouts/site-landing.html",
  pdf: true,
  tags: ("overview", "getting started"),
  summary: "Calepin turns standard Typst documents into computational notebooks and static websites, with executable R, Python, Julia, and shell chunks living directly in the .typ file.",
)) <website-metadata>

#let typst-logo-paths = (
  "m 7.88785,18.1481 c 0,1.1324 0.16248,1.8923 0.48745,2.2797 0.32496,0.3874 0.91581,0.5811 1.7725,0.5811 0.8863,0 2.0237,-0.447 3.4122,-1.341 l 0.8863,1.4751 c -2.5998,2.1754 -4.74159,3.2631 -6.42551,3.2631 -1.68392,0 -3.01334,-0.4023 -3.98824,-1.2069 C 3.05765,22.3648 2.5702,20.9046 2.5702,18.8186 V 7.10728 H 0.35451 L 0,5.45338 2.5702,4.64879 V 2.45849 L 7.88785,0 V 4.91699 L 13.1169,4.51469 12.6294,7.42018 7.88785,7.24138 Z",
  "m 17.5836,34.5977 h -0.7534 c -0.709,-1.2218 -1.2555,-2.9651 -1.6396,-5.2299 l 1.0192,-0.6258 c 0.8568,0.5364 1.6101,0.8046 2.26,0.8046 1.3885,0 2.4521,-0.6407 3.1906,-1.9221 l 1.5067,-3.5313 -6.78,-15.95782 c -0.3545,-0.8046 -0.6647,-1.3112 -0.9306,-1.5198 -0.2659,-0.2086 -0.7681,-0.3576 -1.5067,-0.447 L 13.6396,4.51469 c 2.3634,0.2384 4.0621,0.3576 5.0961,0.3576 1.034,0 2.8804,-0.1192 5.5392,-0.3576 l -0.1772,1.56449 c -0.0591,0.0298 -0.2216,0.0745 -0.4875,0.1341 -0.2363,0.0596 -0.4431,0.1192 -0.6204,0.1788 -0.1477,0.0298 -0.3249,0.0745 -0.5317,0.1341 -0.4136,0.1788 -0.6204,0.4023 -0.6204,0.6705 0,0.2086 0.0591,0.5215 0.1772,0.9387 l 3.8996,10.05742 3.811,-10.23623 c 0.0886,-0.2682 0.133,-0.52149 0.133,-0.75989 0,-0.5364 -0.7829,-0.894 -2.3487,-1.0728 l -0.0886,-1.5645 c 1.4476,0.2086 2.6588,0.31291 3.6337,0.31291 0.9749,0 2.585,-0.10431 4.8302,-0.31291 l -0.3102,1.6539 c -0.709,0.0298 -1.2112,0.1639 -1.5066,0.4023 -0.2659,0.2086 -0.5466,0.6556 -0.842,1.34099 L 23.9204,29.9042 c -1.3294,3.129 -3.4417,4.6935 -6.3368,4.6935 z",
  "m 49.1768,22.1264 c 1.1522,0 2.1271,-0.6407 2.9247,-1.9221 0.8272,-1.3112 1.2408,-3.2184 1.2408,-5.7215 0,-4.64883 -1.3442,-6.97322 -4.0325,-6.97322 -1.0931,0 -2.2896,0.6109 -3.5894,1.83269 V 19.6232 c 0.3249,0.7152 0.8124,1.3112 1.4623,1.788 0.65,0.4768 1.3147,0.7152 1.9941,0.7152 z M 49.2655,35 c -2.7475,-0.2682 -4.6087,-0.4023 -5.5836,-0.4023 -0.9749,0 -2.9247,0.1341 -5.8494,0.4023 l -0.4431,-1.6092 c 1.0635,-0.2384 1.8316,-0.5066 2.3043,-0.8046 0.4727,-0.298 0.709,-0.7599 0.709,-1.3857 V 8.89527 c 0,-1.01319 -0.384,-1.51979 -1.1521,-1.51979 l -2.0385,0.2682 -0.2215,-1.6986 c 2.5406,-1.13239 4.9483,-1.80289 7.2231,-2.01149 l 0.9306,1.02809 c 0,0.6556 0.1477,1.4006 0.4431,2.235 l 1.9498,-1.4751 c 1.359,-1.04299 2.585,-1.56449 3.6781,-1.56449 2.6883,0 4.6677,0.81949 5.938,2.45849 1.2703,1.63899 1.9055,3.99322 1.9055,7.06262 0,3.0395 -0.9601,5.5874 -2.8804,7.6436 -1.9203,2.0562 -4.3871,3.0843 -7.4004,3.0843 -0.9749,0 -1.9941,-0.3278 -3.0576,-0.9834 v 7.8672 c 0,0.5662 0.2363,1.0132 0.709,1.341 0.6795,0.298 1.5657,0.5364 2.6588,0.7152 l 0.4875,0.0894 z",
  "m 67.3337,22.2605 c 0.709,0 1.2999,-0.2235 1.7726,-0.6705 0.4726,-0.447 0.709,-0.9983 0.709,-1.6539 0,-0.6854 -0.0886,-1.192 -0.2659,-1.5198 -0.1772,-0.3576 -0.4727,-0.6854 -0.8863,-0.9834 -0.5908,-0.4768 -1.4328,-0.9833 -2.5258,-1.5197 -1.0636,-0.5364 -1.9055,-0.9983 -2.5259,-1.3857 -0.5909,-0.3874 -1.1817,-1.0132 -1.7726,-1.8774 -0.5613,-0.894 -0.8419,-1.9221 -0.8419,-3.08433 0,-1.66879 0.6499,-2.97999 1.9498,-3.93359 1.3294,-0.98339 2.969,-1.47509 4.9188,-1.47509 1.9498,0 4.1655,0.3427 6.6471,1.02809 -0.0887,1.937 -0.3398,3.57599 -0.7534,4.91702 l -1.7725,0.4023 c -0.709,-2.86082 -2.0532,-4.29122 -4.0326,-4.29122 -1.3294,0 -1.9941,0.7897 -1.9941,2.36909 0,0.6258 0.2511,1.1622 0.7533,1.60923 0.5318,0.447 1.2999,0.9238 2.3043,1.4304 1.0045,0.4768 1.7283,0.8493 2.1714,1.1175 0.4432,0.2384 1.034,0.6407 1.7726,1.2069 C 74.3205,15.0192 75,16.524 75,18.461 c 0,1.937 -0.7386,3.4121 -2.2157,4.4253 -1.4771,1.0132 -3.4269,1.5198 -5.8494,1.5198 -2.3929,0 -4.4905,-0.4023 -6.2926,-1.2069 -0.059,-0.5066 -0.0886,-1.3857 -0.0886,-2.6373 0,-1.2814 0.0296,-2.1754 0.0886,-2.682 l 2.1714,-0.4916 c 0.2364,1.4601 0.7534,2.6372 1.551,3.5312 0.7976,0.894 1.7873,1.341 2.969,1.341 z",
  "m 84.4416,18.1481 c 0,1.1324 0.1624,1.8923 0.4874,2.2797 0.325,0.3874 0.9158,0.5811 1.7726,0.5811 0.8862,0 2.0236,-0.447 3.4121,-1.341 L 91,21.143 c -2.5997,2.1754 -4.7416,3.2631 -6.4255,3.2631 -1.6839,0 -3.0133,-0.4023 -3.9882,-1.2069 -0.9749,-0.8344 -1.4624,-2.2946 -1.4624,-4.3806 V 7.10728 H 76.9082 L 76.5537,5.45338 79.1239,4.64879 V 2.45849 L 84.4416,0 v 4.91699 l 5.229,-0.4023 -0.4875,2.90549 -4.7415,-0.1788 z",
)

#let typst-logo() = html.elem("svg", attrs: (
  viewBox: "0 0 100 40",
  xmlns: "http://www.w3.org/2000/svg",
  fill: "#239dad",
  role: "img",
  "aria-label": "Typst",
  style: "display: inline-block; width: 3.3em; height: 1.32em; vertical-align: -0.50em; margin-inline-start: 0.05em;",
))[
  #html.elem("g", attrs: (transform: "translate(4.5,2.5)"))[
    #for path in typst-logo-paths [
      #html.elem("path", attrs: (d: path))[]
    ]
  ]
]

// hero
#let hero() = calepin.elements.target(
  html: () => [
    #html.elem("section", attrs: (class: "landing-hero"))[
      #html.elem("img", attrs: (
        src: "assets/logo_long.svg",
        alt: "Calepin",
        class: "landing-hero-wordmark",
        style: "display: block; margin-inline: auto; color: currentColor;",
        "data-inline-svg": "1",
      ))[]
      #html.elem("h2", attrs: (style: "font-size: 1.3em; font-weight: 700;"))[
        Computational Notebooks and Static Websites in #typst-logo()
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
  ],
  paged: () => [
    #align(center)[
      #image("/assets/logo_long.svg", width: 30%, alt: "Calepin logo")
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

  Write and evaluate Calepin documents without leaving your editor. The Calepin extension
  starts and stops watched code evaluation; use Tinymist or another extension independently
  for Typst language tooling and preview. Install Calepin from the VS Code Marketplace, or from Open VSX for Cursor,
  Positron, and other VSX-compatible editors. See #link("editors.html")[Editor integration].
]

#let editor-text() = calepin.elements.target(
  html: () => html.elem("section")[#editor-text-content()],
  paged: () => editor-text-content(),
)

#editor-text()

= A simple computational notebook

````typ
#import "/.calepin/calepin.typ" as calepin
#show: calepin.document

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
