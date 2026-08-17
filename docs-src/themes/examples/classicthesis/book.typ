#show: classicthesis.with(
    title: "a great title",
    subtitle: "With a catchy subtitle",
    author: "Vincent Arel-Bundock",
    date: "2026-06-22",
    abstract: [This #strong[fascinating] thesis explores...],
)

// Let calepin own code styling. classicthesis frames every block `raw` with a
// 1em-inset box; calepin already renders chunk code in its own block. Re-emit
// calepin's pre-rendered code (a non-`auto` syntax theme) as inline raw so
// classicthesis's `raw.where(block: true)` rule no longer matches it.
#show raw.where(block: true): it => if it.theme != auto {
  raw(it.text, block: false, lang: it.at("lang", default: none), theme: it.theme)
} else { it }

= Top level

```python
def f(x):
    return x + 4

print(f(3))
```

#lorem(60)
