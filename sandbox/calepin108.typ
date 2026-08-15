// Issue #104: syntax highlighting was missing keywords, strings and comments.
//
// Try both targets:
//   calepin compile sandbox/calepin108.typ --format pdf
//   calepin compile sandbox/calepin108.typ --format html
//
// This reproduces the reporter's setup: codly renders `raw` itself, so it
// claims blocks before Calepin's show rule sees them. The `set raw(theme: ..)`
// below is what lines the two up -- point codly at the palette Calepin writes
// to `.calepin/syntax.tmTheme` on every build, and codly-rendered blocks use
// the same colors as executed chunks. Comment that line out to see them drift
// back apart.
//
// The executed chunk and the plain fenced block should agree, and both should
// color `import`, `"Sinus und Kosinus"` and the `#` comments.

#import "@preview/codly:1.3.0": *
#import "@preview/codly-languages:0.1.1": *
#import ".calepin/calepin.typ" as calepin

#show: calepin.document

#show: codly-init.with()
#codly(languages: codly-languages)
#set raw(theme: "/.calepin/syntax.tmTheme")

#calepin.setup(echo: true, theme: "typst")

= Issue #104

An executed chunk:

```python
import numpy as np
import matplotlib.pyplot as plt

# Comments, keywords and strings should all be colored.
x = np.linspace(0, 2 * np.pi, 200)
plt.figure(figsize=(5, 3.6))
plt.plot(x, np.sin(x), label="sin(x)")
plt.plot(x, np.cos(x), label="cos(x)")
plt.title("Sinus und Kosinus")
plt.legend()
plt.tight_layout()
plt.show()
```

A non-executed block in another language, for comparison:

```rust
// A comment.
use std::collections::BTreeMap;

fn main() {
    let greeting: String = String::from("Sinus und Kosinus");
    println!("{greeting}");
}
```
