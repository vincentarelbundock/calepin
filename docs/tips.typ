#set document(title: [Tips & tricks])
#metadata((tags: ("tips", "notebooks", "websites"))) <website-metadata>

#title()

= Conditional output

Use `calepin-target` when a small piece of Typst should change between HTML and paged output:

```typ
#let target = sys.inputs.at("calepin-target", default: "paged")

#if target == "html" [
  This appears only in HTML.
] else [
  This appears in PDF, SVG, and PNG output.
]
```

= Using codly with executed chunks

#link("https://typst.app/universe/package/codly")[codly] installs its own `show raw:` rules. Calepin wraps echoed source in a labeled block *around* that `raw`, so without an override you get codly's frame nested inside Calepin's box.

One show rule fixes it. This is a complete document, using the default Calepin theme:

````typ
#import "/.calepin/calepin.typ" as calepin
#import "@preview/codly:1.3.0": *
#import "@preview/codly-languages:0.1.1": *

#show: calepin.document
#show: codly-init
#codly(languages: codly-languages)

// Hand the code to codly, and drop Calepin's own box from around it.
#show <calepin-input>: it => it.body

#calepin.setup(echo: true)

```python
import math
print(f"circumference: {2 * math.pi:.4f}")
```
````

Rendered result:

#let target = sys.inputs.at("calepin-target", default: "paged")

#if target == "html" [
  #html.elem("iframe", attrs: (
    src: "assets/codly.pdf",
    title: "Calepin chunk styled by codly",
    style: "display: block; width: 100%; height: min(40vh, 20rem); border: 1px solid var(--pico-muted-border-color); border-radius: var(--pico-border-radius);",
  ))[]
] else [
  #link("assets/codly.pdf")[Open the rendered PDF.]
]

Chunk output is untouched: it is not a `raw` element on the paged target, so codly never reaches it. The override reconstructs from `it.body` rather than re-emitting `it`; re-emitting would nest your rule around Calepin's default instead of replacing it. See #link("themes/styling.html")[Styling chunks] for the other labels and the full explanation.

Setting `theme = "typst"` removes Calepin's chrome everywhere at once, with no show rules at all. Chunks still execute either way.
