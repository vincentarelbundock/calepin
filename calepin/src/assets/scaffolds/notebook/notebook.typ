#import "@preview/calepin:0.0.1" as calepin

#set document(title: [Calepin example])

#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
  fenced-chunks: true,
)

#let py = calepin.inline.with("python")

#title()

Inline Python result: #py[`print(40 + 2)`].

```python
message = "hello from a code chunk"
print(message)
```
