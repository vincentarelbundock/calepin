#import "@preview/calepin:0.0.1" as calepin

#set document(title: [Code And Results])
#metadata((
  title: "Code And Results",
  translation_key: "code-and-results",
  kind: "post",
  date: "2026-06-09",
  tags: ("code", "notebook"),
  summary: "A post with executable code output for checking notebook styling.",
)) <website-metadata>

#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
  fenced-chunks: true,
)

#title()

#lorem(50)

```python
numbers = [2, 4, 6, 8]
print(sum(numbers) / len(numbers))
```

#lorem(70)
