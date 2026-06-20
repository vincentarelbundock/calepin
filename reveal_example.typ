#import "/.calepin/calepin.typ" as calepin

#calepin.setup(
  theme: "revealjs",
  eval: false,
)

#show raw.where(block: true): set text(size: .8em)

#heading(level: 1)[Reveal.js Theme Demo]

#lorem(12)

= Slide 2: A horizontal slide

`h1` headings become horizontal slides in the `revealjs` theme.

- Use normal markdown-style lists.
- Include code blocks for syntax highlighting.

```python
for i in range(1, 4):
    print(f"slide point {i}")
```

== Slide 2.1: A vertical slide

`h2` headings become vertical fragments within the previous `h1` slide.

```rust
fn main() {
    println!("Hello, reveal.js!");
}
```

== Slide 2.2: Another vertical slide

Use this to present a sequence of related ideas.

- Point one
- Point two
- Point three

= Slide 3: Code execution style

```python
value = 41
value + 1
```

`calepin.setup` is set to the `revealjs` built-in theme, so the output
fits a fullscreen slide deck by default.