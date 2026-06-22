#import "/.calepin/calepin.typ" as calepin_runtime
#set document(title: [Case studies])
#import "/.calepin/calepin.typ" as calepin
#title()

= Tufte


In this case study, we build on top of the `academic` theme to replicate
features of the popular Tufte CSS article style: serif typography, warm paper
colors, restrained accents, sidenotes, margin figures, and code/output surfaces
that match the page.

Reference files and rendered output can be viewed here:

- #link("examples/tufte/calepin.toml")[calepin.toml]
- #link("examples/tufte/themes/tufte/css/tufte.css")[tufte.css]
- #link("examples/tufte/tufte.typ")[tufte.typ]
- #link("examples/tufte/tufte.html")[HTML]
- #link("examples/tufte/tufte.pdf")[PDF]

Start with a small `calepin.toml` next to the document:

```toml
theme = "themes/tufte"
```

The local theme extends `academic`:

```toml
# themes/tufte/theme.toml
extends = "academic"
```

That keeps all of the built-in `academic` theme structure: the single-document
HTML wrapper, the theme toggle, sidenotes, side figures, code styling, and
dark-mode support. The local `themes/tufte/css/tufte.css` file is intentionally
small. It overrides the public `--calepin-*` tokens instead of targeting private
theme internals:

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

From the case-study directory, render the HTML and PDF with:

#calepin_runtime.chunk_from_raw_plain("sh", raw("cd docs/themes/examples/tufte\ncalepin compile tufte.typ --config calepin.toml --format html\ncalepin compile tufte.typ --config calepin.toml --format pdf\n", block: true, lang: "sh"))

The config path matters because `theme = "themes/tufte"` is resolved relative to
`calepin.toml`. Keeping the config, local theme, and document together makes the
example portable: copy the directory, run the same commands, and the academic
theme plus Tufte overlay are applied in both rendered outputs.

