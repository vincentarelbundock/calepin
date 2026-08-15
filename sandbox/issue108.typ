// Repro for issue #108: "Calepin overwrites code chunks with preceding".
//
//   calepin compile sandbox/issue108.typ issue108.pdf
//
// (output paths resolve from the document's own directory, so that lands in
// `sandbox/`)
//
// Before the fix, the Python block below rendered the *Rust* source: an
// unrunnable fence stepped the automatic `chunk-N` counter during the query
// pass but not during the render pass, so every later chunk picked up its
// predecessor's source. Both blocks should now show their own code, and the
// Python chunk should also show its output.
//
// Needs the `codly` packages from Typst Universe (the original report used
// them, and they are what made the mismatch obvious). Delete the three codly
// lines to run this offline.

#import "/.calepin/calepin.typ" as calepin
#import "@preview/codly:1.3.0": *
#import "@preview/codly-languages:0.1.1": *

#show: calepin.document
#show: codly-init
#codly(languages: codly-languages)

#calepin.setup(echo: true, theme: "typst")

= Heading 1

A code block Calepin does not execute, in a language it has no engine for. It
should keep its own source and its syntax highlighting, and it should not be
reported as a chunk:

```rust
pub fn main() {
    println!("Hello, world!");
}
```

The Python block below is a real chunk. Before the fix it repeated the Rust
source above; it should show its own code and plot:

```python
import numpy as np
import matplotlib.pyplot as plt

x = np.linspace(0, 2 * np.pi, 200)
plt.figure(figsize=(5, 3.6))
plt.plot(x, np.sin(x), label="sin(x)")
plt.plot(x, np.cos(x), label="cos(x)")
plt.title("Sinus und Kosinus")
plt.legend()
plt.tight_layout()
plt.show()
```

A third block, in a language with no kernel installed, to confirm the numbering
stays in step no matter how many unrunnable fences sit between chunks:

```json
{"not": "a chunk"}
```

```python
print("still my own source")
```
