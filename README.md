# Calepin

Experimental Typst preprocessor for executable code chunks.

Install the Rust CLI. The matching Typst runtime is embedded in the binary and
written to `.calepin/calepin.typ` when you compile or watch a document, so there
is no separate Typst Universe package to install.

```sh
cargo install calepin
calepin compile paper.typ paper.pdf
```

Executable paths are configured in `.calepin/config.toml`:

```toml
[executables]
python = ".venv/bin/python"
```

Documents import the embedded runtime that `calepin compile` writes into the
project:

````typ
#import ".calepin/calepin.typ"

#calepin.setup(lang: "python")

```python-chunk
print(42)
```
```python
print("also works with plain lang blocks when lang is configured")
```

Set `raw-chunks` to `false` (the default) if you want to require explicit
`...-chunk` language tags instead of plain language blocks.
````

For inline output, use `calepin.inline`:

```typ
#let py = calepin.inline.with("python")

The answer is #py[`print("42")`].
```

Supported engines are `r`, `python`, `julia`, `sh`, and `bash` as an alias for `sh`.
