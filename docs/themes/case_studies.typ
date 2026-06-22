#set document(title: [Case studies])
#import "/.calepin/calepin.typ" as calepin
#title()

= Tufte

This case study shows how to build a local theme on top of `academic` to
replicate the Tufte CSS article style: serif typography, warm paper colors,
restrained accents, sidenotes, margin figures, and code/output surfaces that
match the page.

Reference files and rendered output can be viewed here:

- #link("examples/tufte/calepin.toml")[calepin.toml]
- #link("examples/tufte/themes/tufte/css/tufte.css")[tufte.css]
- #link("examples/tufte/tufte.typ")[tufte.typ]
- #link("examples/tufte/tufte.html")[HTML]
- #link("examples/tufte/tufte.pdf")[PDF]

The source tree is intentionally small:

```text
project/
  calepin.toml
  tufte.typ
  tufte/
    theme.toml
    css/
      tufte.css
```

`calepin.toml` points `calepin` at the local theme directory:

```toml
theme = "tufte"
```

The theme directory itself declares its base theme:

```toml
# tufte/theme.toml
extends = "academic"
```

That keeps all of the built-in `academic` theme structure: the single-document
HTML wrapper, the theme toggle, sidenotes, side figures, code styling, and
dark-mode support. The local `tufte/css/tufte.css` file is intentionally small.
It overrides the public `--calepin-*` tokens instead of targeting private theme
internals:

```css
:root, html[data-theme="light"] {
  --calepin-font-body: Palatino, "Palatino Linotype", "Book Antiqua", Georgia, serif;
  --calepin-font-heading: Palatino, "Palatino Linotype", "Book Antiqua", Georgia, serif;
  --calepin-surface-code: #fffdf0;
  --calepin-surface-output: #fffdf0;
  --calepin-surface: #fffdf0;
  --calepin-surface-muted: #fff8eb;
  --calepin-color-background: #fffff8;
  --calepin-color-text: #12110f;
  --calepin-color-muted: #5a5650;
  --calepin-color-accent: #7a2e2a;
  --calepin-color-accent-hover: #5f2520;
  --calepin-color-link: #7a2e2a;
  --calepin-color-link-hover: #5f2520;
}

html[data-theme="dark"] {
  --calepin-color-background: #18130d;
  --calepin-color-text: #f6efe8;
  --calepin-color-muted: #c5baa7;
  --calepin-color-accent: #d58a7f;
  --calepin-color-accent-hover: #f0b7ab;
  --calepin-color-link: #d58a7f;
  --calepin-color-link-hover: #f0b7ab;
  --calepin-surface-code: #1f1a14;
  --calepin-surface-output: #1f1a14;
  --calepin-surface: #1f1a14;
  --calepin-surface-muted: #2b241b;
}
```

The document source can then use the normal academic-theme elements:
`calepin.elements.sidenote` for margin notes,
`calepin.elements.sidefigure` for margin figures, and regular executable code
chunks for computed output. The stylesheet changes the feel of those elements,
but the layout behavior still comes from the built-in theme.

From the project root, render the HTML and PDF with:

```sh
calepin compile tufte.typ --config calepin.toml --format html
calepin compile tufte.typ --config calepin.toml --format pdf
```

The config path matters because `theme = "tufte"` is resolved relative to
`calepin.toml`. Keeping the config, local theme, and document together makes the
example portable: copy the directory, run the same commands, and the academic
theme plus Tufte overlay are applied in both rendered outputs.

= Classicthesis

This case study shows how to use a local theme with `layouts/pdf.typ` to wrap a
Typst document in the `classicthesis` template.

The project can stay small:

```text
project/
  calepin.toml
  book.typ
  theme/
    theme.toml
    layouts/
      pdf.typ
```

`calepin.toml` points Calepin at the local theme directory:

```toml
theme = "./theme"
```

The theme manifest disables inherited Calepin styling so the PDF layout comes
entirely from the template:

```toml
# theme/theme.toml
extends = "typst"
```

Then put the `classicthesis` template in `theme/layouts/pdf.typ`:

```typ
#import "@preview/classicthesis:0.1.0": *

#show: classicthesis.with(
  title: "My Book Title",
  subtitle: "A Subtitle",
  author: "Author Name",
  date: "2025",
  dedication: [To my readers.],
  abstract: [This book explores...],
)

#part("Part One", preamble: [Introduction to the topic.])

= Chapter One

Your content here...

== Section

More content...
```

`layouts/pdf.typ` is rendered before Typst compiles the notebook, so `doc.body`
is where Calepin injects the document source. Here, the `classicthesis`
template provides the page design and chapter structure, while the notebook body
still comes from your `.typ` file.

From the project root, render the PDF with:

```sh
calepin compile book.typ --config calepin.toml --format pdf
```

If you want the same document to use a different PDF layout later, swap the
contents of `theme/layouts/pdf.typ` without changing the document itself.

= Website fonts

This case study shows how to override a website's fonts using Google Fonts API. We start by creating an example website scaffold based on the default `calepin` theme. The following command will create a new directory called `calepin_website`:


```sh
calepin new website --theme calepin
```

Then, we add simple local theme directory with a manifest `theme.toml` and a single `fonts.css` file:

```text
calepin_website/

  theme/            # theme directory
    theme.toml
    css/
      fonts.css

  calepin.toml      # configuration file

  index.typ         # website content
  404.typ
  assets/
  posts/
```

The configuration file, `calepin_website/calepin.toml`, must point to the local theme:

```toml
theme = "./theme"
```

The theme manifest, `calepin_website/theme/theme.toml`, inherits from the built-in `calepin` theme:

```toml
extends = "calepin"
```

We add a single CSS file which imports Google Fonts and overrides the public font tokens:

```css
@import url("https://fonts.googleapis.com/css2?family=Rubik+Moonrocks&family=Space+Grotesk:wght@400;500;700&family=IBM+Plex+Mono:wght@400;500&display=swap");

:root {
  --calepin-font-body: "Space Grotesk", system-ui, sans-serif;
  --calepin-font-heading: "Rubik Moonrocks", "Space Grotesk", sans-serif;
  --calepin-font-mono: "IBM Plex Mono", ui-monospace, monospace;
}
```

This keeps the layout, navigation, and page behavior from the built-in `calepin` theme. The local theme only changes typography, so the result is easy to reason about: same site structure, different font stack.

Finally, we eender and serve the website:

```sh
calepin compile calepin_website
calepin serve calepin_website --open
```
