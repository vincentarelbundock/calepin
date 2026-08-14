#import "/.calepin/calepin.typ" as calepin
#import "@preview/codly:1.3.0": *
#import "@preview/codly-languages:0.1.1": *

#show: calepin.document
#show: codly-init
#codly(languages: codly-languages)

// Hand the code to codly, and drop Calepin's own box from around it.
#show <calepin-input>: it => it.body

#set page(width: 15cm, height: auto, margin: 1cm)
#set text(font: "Libertinus Serif", size: 11pt)

#calepin.setup(echo: true)

```python
import math
print(f"circumference: {2 * math.pi:.4f}")
```
