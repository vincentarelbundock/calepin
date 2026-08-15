// Issue #104: syntax highlighting was missing keywords, strings and comments.
//
// Try both targets:
//   calepin compile sandbox/calepin108.typ --format pdf
//   calepin compile sandbox/calepin108.typ --format html
//
// The executed chunk and the plain fenced block below it should agree, and both
// should color `import`, `"Sinus und Kosinus"` and the `#` comments.
//
// To see the codly case from the issue, uncomment the three codly lines. Note
// the `set raw(theme: ..)`: codly renders `raw` itself, so it never reaches
// Calepin's show rule and needs to be pointed at the generated palette.

#import ".calepin/calepin.typ" as calepin

#show: calepin.document

// #import "@preview/codly:1.3.0": *
// #import "@preview/codly-languages:0.1.1": *
// #show: codly-init.with()
// #codly(languages: codly-languages)
// #set raw(theme: "/.calepin/syntax.tmTheme")

#calepin.setup(echo: true)

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
