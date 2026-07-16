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
