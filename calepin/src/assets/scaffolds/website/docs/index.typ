#import "@preview/calepin:0.0.1" as calepin

#set document(title: [Example Site])
#metadata((title: "Home", translation_key: "home")) <website-metadata>

#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
  fenced-chunks: true,
)

#let target = sys.inputs.at("calepin-target", default: "paged")

#show: body => {
  if target == "html" {
    body
  } else {
    set page(columns: 2)
    body
  }
}

#title()

This scaffold is a compact website that exercises Calepin's bundled themes.
Switch `theme` in `calepin.toml` between `calepin` and `academic` to compare the
same content in different layouts.

= Reading Content

#if target == "html" {
  html.elem("img", "", attrs: (
    class: "calepin-float-right calepin-scaffold-portrait",
    src: "assets/portrait.jpg",
    alt: "Portrait photograph",
    width: "1280",
    height: "1920",
    loading: "lazy",
    decoding: "async",
  ))
} else {
  place(
    top + right,
    float: true,
    clearance: 1em,
    image("/assets/portrait.jpg", width: 32%),
  )
}

#lorem(45)

#lorem(75)

This sentence has a footnote.#footnote[Footnotes are useful for the default
theme and also help inspect essay-style reading layouts.] The next sentence
continues the sample paragraph in normal flow.

#lorem(35)

= Code And Output

```python
values = [1, 1, 2, 3, 5]
print(sum(values))
```

= A Small Table

#table(
  columns: 3,
  [Theme], [Best for], [What to check],
  [calepin], [Documentation], [Sidebar, page navigation, code],
  [academic], [Essays], [Footnotes, reading width],
)

= More Text

#lorem(80)
