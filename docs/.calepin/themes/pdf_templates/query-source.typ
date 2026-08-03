#set document(title: [PDF])
#import "/.calepin/calepin.typ" as calepin
#show: calepin.document
#metadata((tags: ("themes", "PDF", "templates"))) <website-metadata>
#title()

`layouts/pdf.typ` is the Typst-side layout used by PDF and SVG notebook outputs.

```text
themes/my-theme/
  layouts/
    pdf.typ
```

Before Typst runs, Calepin renders this file with MiniJinja, the same engine used for HTML layouts (see #link("templating.html")[Templating]), so the output is still valid Typst source. The file uses a `.typ` extension because it renders to Typst, even though it is a MiniJinja template.

Inside the layout, place notebook content with `doc.body`:

```typ
#set page(
  paper: "us-letter",
  margin: (x: 1in, y: 0.85in),
  numbering: "1",
)

#set text(font: "Libertinus Serif", size: 10.5pt)

{{ doc.body }}
```

Useful `layouts/pdf.typ` values:

- `theme`: local theme directory name
- `target`: `paged`
- `doc.path`: `.typ` input path relative to workspace
- `doc.dir`: input directory relative to workspace
- `doc.stem`: input filename without `.typ`
- `doc.title`: document title, or an empty string
- `doc.body`: notebook body, injected as a `#include`
- `doc.meta`: values from `#metadata(...) <website-metadata>`
- `store`: the completed document store from `[store]`, `calepin.store.set()`, successful `store-set` chunks, and CLI `--set store.<name>=...` overrides

`doc.body` must be included explicitly if you want the notebook body in the rendered output.

`theme = "typst"` disables notebook-specific theming, and `extends = "typst"` creates a local theme with no inherited Calepin base. Use an empty `layouts/pdf.typ` for a minimal pass-through layout.
