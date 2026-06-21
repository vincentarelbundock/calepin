#import "/.calepin/calepin.typ" as calepin

#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
)

= Runtime path test

#calepin.chunk("python", label: "x", results: "hide")[
```python
print("hello from calepin")
```
]
