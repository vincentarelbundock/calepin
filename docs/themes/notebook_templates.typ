#set document(title: [Notebook Typst layouts])
#import "/.calepin/calepin.typ" as calepin
#title()

`layouts/notebook.typ` is the Typst-side layout used by notebook outputs such as PDF and SVG.

```text
themes/my-theme/
  layouts/
    notebook.typ
```

Before Typst runs, Calepin renders this file with MiniJinja so the output is still valid Typst source. The file uses a `.typ` extension because it renders to Typst, even though it is a MiniJinja template.

Inside the layout, place notebook content with `document.body`:

```typ
#set page(
  paper: "us-letter",
  margin: (x: 1in, y: 0.85in),
  numbering: "1",
)

#set text(font: "Libertinus Serif", size: 10.5pt)

{{ document.body }}
```

Useful `layouts/notebook.typ` values:

- `theme`: local theme directory name
- `target`: `notebook`
- `document.path`: `.typ` input path relative to workspace
- `document.dir`: input directory relative to workspace
- `document.stem`: input filename without `.typ`
- `document.body`: notebook body, injected as a `#include`
- `document.meta`: values from `#metadata(...) <website-metadata>`
- `params`: CLI parameter map

If `document.body` is not referenced, Calepin appends the notebook body after the rendered layout.

`theme = "typst"` disables notebook-specific theming, and `extends = "typst"` creates a local theme with no inherited Calepin base. Use an empty `layouts/notebook.typ` for a minimal pass-through layout. `notebook.typ.jinja` and `paged.typ.jinja` are not supported.
