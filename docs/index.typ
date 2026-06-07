#let hero(
  logo,
  subtitle,
  typst-logo,
) = {
  let target = sys.inputs.at("calepin-target", default: "paged");

  if target == "html" {
    html.elem("section", attrs: (class: "hero"))[
      #html.elem("div", attrs: (class: "hero-text"))[
        #html.elem("div", attrs: (class: "hero-title"))[
          #html.elem("img", attrs: (
            src: logo,
            alt: "Calepin logo",
            class: "hero-wordmark",
            "data-inline-svg": "1",
          ))
        ]
        #html.elem("div", attrs: (class: "hero-subtitle"))[
          #html.elem("span", attrs: (class: "hero-subtitle-text"))[#text(subtitle)]
          #html.elem("img", attrs: (src: typst-logo, alt: "Typst logo", class: "hero-typst-logo"))
        ]
        #html.elem("code", attrs: (class: "hero-command"))[calepin compile notebook.typ]
      ]
    ]
  } else if target == "paged" {
    align(center, image(logo, width: 40%, alt: "Calepin logo"))
  }
}

#hero(
  "assets/logo_long_2.svg",
  "Computational Notebooks in",
  "assets/logo_typst.svg",
)

#if sys.inputs.at("calepin-target", default: "paged") == "html" [
  #html.elem(
    "section",
    attrs: (class: "calepin-website-features"),
  )[
    #html.elem("article", attrs: (class: "calepin-website-feature-card"))[
      #html.elem("h4", [Typst-native])
      #html.elem("p", [Write notebooks in pure Typst, a simple, consistent, powerful, and elegant typsetting system. Not another _ad hoc_ markdown variant.])
    ]
    #html.elem("article", attrs: (class: "calepin-website-feature-card"))[
      #html.elem("h4", [Executable code])
      #html.elem("p", [Insert code directly in your `.typ` files. These chunks are executed and the results are automatically inserted back in your final document.])
    ]
    #html.elem("article", attrs: (class: "calepin-website-feature-card"))[
      #html.elem("h4", [Several formats])
      #html.elem("p", [Publish clean HTML, PDF, or image files from a single Typst source.])
    ]
    #html.elem("article", attrs: (class: "calepin-website-feature-card"))[
      #html.elem("h4", [Multi-lingual])
      #html.elem("p", [Embed code written in Python, R, Julia, Bash, Mermaid, TikZ, or any language supported by Jupyter.])
    ]
  ]
]

== What is Calepin?
<what-is-calepin>

==== Typst
<typst>

#link("https://typst.app/")[Typst] is a modern typesetting system that
compiles plain text markup into rich documents. Think of it as a simple
Markdown-like syntax, with the scriptability of LaTeX, and the ability
to export to PDF, SVG, and HTML files. Typst is ultra fast, easy to
learn, and expressive, which makes it a comfortable tool for anything
from short letters to full scientific manuscripts.

==== Computational notebooks
<computational-notebooks>

A computational notebook mixes prose with executable code. Figures,
tables, and numbers are computed when the document is rendered, rather
than pasted-in by hand. This allows analysts to create reproducible
reports in the tradition of
#link("https://en.wikipedia.org/wiki/Literate_programming")[literate programming].

Some notebook tools can act as frontends for Typst, but they often
superimpose their own syntax, language, and structure. For example,
#link("https://quarto.org/")[Posit's Quarto] supports an extended idiom
of Markdown, which can be translated to Typst and then rendered as PDF.

#if sys.inputs.at("calepin-target", default: "paged") == "html" [
  #html.elem("div", attrs: (class: "calepin-screenshot-block"))[
  #html.elem("button", attrs: (
    class: "calepin-screenshot-thumb",
    type: "button",
    "data-lightbox-dialog": "calepin-screenshot-dialog",
    "aria-label": "Open Calepin notebook screenshot",
  ))[
    #html.elem("img", attrs: (
      src: "assets/screenshot_notebook.png",
      alt: "Calepin notebook screenshot",
      class: "calepin-screenshot-thumb__media",
    ))
    #html.elem("span", attrs: (class: "calepin-screenshot-thumb__zoom", "aria-hidden": "true"))[↗]
  ]
]

#html.elem("dialog", attrs: (id: "calepin-screenshot-dialog", class: "calepin-screenshot-dialog"))[
  #html.elem("article")[
    #html.elem("header")[
      #html.elem("button", attrs: (
        rel: "prev",
        type: "button",
        "data-close-dialog": "",
        "aria-label": "Close screenshot preview",
      ))
    ]
    #html.elem("img", attrs: (
      class: "calepin-screenshot-dialog__media",
      src: "assets/screenshot_notebook.png",
      alt: "Calepin notebook screenshot",
    ))
  ]
]
]


==== Calepin
<calepin>

#emph[Calepin] is a Typst-native computational notebook. Rather than
hide Typst behind Markdown, it embeds executable code chunks directly
inside `.typ` documents. #emph[Calepin] scans a `.typ` file for inline
code or exectuable chunks, evaluates them, and lets Typst render the
final document with computed results in place.

#emph[Calepin] is language-agnostic. It can execute code in Python, R,
and any language with a Jupyter kernel. It can also render diagrams
using engines like Mermaid, Graphviz, TikZ, and D2.

#emph[Calepin] comes with extensions for popular editors like VS-Code,
Cursor, and Positron (via the Microsoft and VSX marketplaces).

- VS Code:
  #link("https://marketplace.visualstudio.com/items?itemName=VincentArel-Bundock.calepin")[VincentArel-Bundock.calepin]
- Open VSX:
  #link("https://open-vsx.org/extension/VincentArel-Bundock/calepin")[VincentArel-Bundock/calepin]

#if sys.inputs.at("calepin-target", default: "paged") == "html" [
  Here is a short video of a live editing session in VS-Code.

  #html.elem("div", attrs: (class: "calepin-video-block"))[
    #html.elem("button", attrs: (
        class: "calepin-video-thumb",
        type: "button",
        "data-video-dialog": "calepin-video-dialog",
        "aria-label": "Open Calepin editor preview video",
      ))[
      #html.elem("video", attrs: (
        class: "calepin-video-thumb__media",
        src: "assets/calepin_vscode.mp4",
        muted: "",
        playsinline: "",
        preload: "metadata",
        poster: "assets/calepin_vscode-thumb.png",
      ))
      #html.elem("span", attrs: (class: "calepin-video-thumb__play", "aria-hidden": "true"))[▶]
    ]
  ]
  #html.elem("dialog", attrs: (id: "calepin-video-dialog", class: "calepin-video-dialog"))[
    #html.elem("article")[
      #html.elem("header")[
        #html.elem("button", attrs: (
          rel: "prev",
          type: "button",
          "data-close-dialog": "",
          "aria-label": "Close video preview",
        ))
      ]
      #html.elem("video", attrs: (
        class: "calepin-video-dialog__media",
        src: "assets/calepin_vscode.mp4",
        muted: "",
        autoplay: "",
        controls: "",
        playsinline: "",
        preload: "metadata",
      ))
    ]
  ]
]

== Example
<example>

_Calepin_ notebooks are standard Typst documents with a few extra features. 

=== Preamble
<preamble>

When a user calls `calepin` to compile a file, a hidden `.calepin/` directory is created to hold code artefacts and the special Typst functions and macros used to process code chunks. To build a notebook, we start by loading these functions into the document with `#import`. Then, we fix document-wide settings with `calepin.setup()`.

```typ
#import ".calepin/calepin.typ"
#calepin.setup(echo: true, eval: true,)
```

Now, we define a short alias for Python inline computation. This will be used to embed computation in prose (i.e., in the text rather than as a separate code block).

```typ
#let py = calepin.inline.with("python")
```

=== Chunks
<chunks>

A block chunk runs a piece of code and inserts its result. Start with a plain fenced block:

````typ
```python
x = 41
print(x + 1)
```

Variables are persistent across chunks:

```python
print(x + 2)
```
]
````

When you need extra control, use `#calepin.chunk` with options such as
labels, captions, hiding source code, or changing how results are shown.
If the body is a fenced block with a language, `#calepin.chunk` infers
the engine from the fence:

````typ
#calepin.chunk(label: "answer")[
```python
x = 41
print(x + 1)
```
````

=== Inline
<inline>

An inline expression drops a computed value into the surrounding prose.
It uses the same raw body contract and never takes a label.

```typ
The inline answer is #py[`print(40 + 2)`].
```

=== All together now
<all-together-now>

Here is one full document example.

````typ
#import ".calepin/calepin.typ"

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

== Render
<render>

Use `calepin compile` when you want to execute code chunks and render
the notebook. The command line shape is intentionally the same as
`typst compile`: same output-first/format-driven arguments, with `--`
pass-through for Typst flags, plus Calepin preprocessing.

```sh
calepin compile paper.typ --format pdf
calepin compile paper.typ --format html
calepin compile paper.typ {p}.svg --format svg

# explicit output path
calepin compile paper.typ path/to/paper.pdf --format pdf
```

Arguments after `--` are forwarded to Typst, so project-specific Typst
flags can stay in the same command.

```sh
# open pdf in system viewer
calepin compile paper.typ -- --open 

# set path to font directory
calepin compile paper.typ -- --font-path fonts
```

== Watch
<watch>

Use `calepin watch` while editing. It watches your source for changes,
re-runs preprocessing, and delegates recompilation and previewing to
`typst watch`. The command form is the same as `typst watch`: same
positional arguments and pass-through flags, with Calepin running its
preprocessing step first.

```sh
calepin watch paper.typ
calepin watch paper.typ --format html
calepin watch paper.typ {p}.svg --format svg

Stop a running watch from the same project.
calepin stop paper.typ
```

The default output format is PDF. Choose another format with `--format`,
or let the output extension select the format.

Arguments after `--` are passed through to `typst watch`. Typst's
`--open` flag opens the rendered output in the operating system's
default viewer, and Typst's `--port` flag chooses the HTML preview port.

```sh
calepin watch example.typ -- --open
calepin watch paper.typ paper.html --format html -- --port 3001 --open
```

==== PDF viewer auto-refresh
<pdf-viewer-auto-refresh>

Some PDF viewers do not automatically refresh a document when it is
regenerated on disk. For example, macOS Preview may keep showing an
older PDF until the window is focused, the file is reopened, or the
application is restarted.

For smoother live preview, use a PDF viewer that reloads the file when
it changes. On macOS, Skim is a good option. Other platforms have
similar auto-reloading viewers, which are useful when working with tools
that repeatedly rebuild PDFs.

