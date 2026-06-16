#import "@preview/calepin:0.0.1" as calepin
#import "/assets/site.typ" as site

#set document(title: [Example Site])
#metadata((title: "Home", translation_key: "home")) <website-metadata>

#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
  fenced-chunks: true,
)

#title()

This scaffold is a compact website that exercises Calepin's bundled themes.
Switch `theme` in `calepin.toml` between `calepin`, `academic`, and `tufte` to
compare the same content in different layouts.

= Reading Content

#lorem(45)

This sentence has a footnote.#footnote[Footnotes are useful for the default
theme and also help inspect Tufte-style reading layouts.] The next sentence
places a short note in the margin when the active theme supports it.
#site.margin-note[A margin note gives the Tufte
theme something to place in its side rail. Other themes keep it readable in the
normal flow.]

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
  [academic], [Profile sites], [Top navigation, lists, posts],
  [tufte], [Essays], [Footnotes, margin notes, reading width],
)

= More Text

#lorem(80)
