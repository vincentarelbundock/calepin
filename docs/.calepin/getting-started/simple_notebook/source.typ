#import "/.calepin/calepin.typ" as calepin
#set document(title: [Simple notebook])
#metadata((title: "Simple notebook", tags: ("getting started", "notebooks"))) <website-metadata>

#title()

A Calepin notebook is an ordinary Typst document containing executable code blocks and inline computations. This minimal example runs two Python chunks in the same persistent session:

````typ
#import "/.calepin/calepin.typ" as calepin

#calepin.setup(
  echo: true,
  results: "verbatim",
)

#let py = calepin.inline.with("python")

```python
x = 41
print(x + 1)
```

Variables are persistent across chunks:

```python
print(x + 2)
```

The inline answer is #py[`print(40 + 2)`].
````
