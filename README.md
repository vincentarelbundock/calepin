# Calepin

Experimental Typst preprocessor for executable code chunks.

```sh
calepin preprocess paper.typ
calepin compile paper.typ paper.pdf
```

Documents import the runtime written by preprocessing:

```typ
#import ".calepin/calepin.typ"

#calepin.chunk(engine: "python", label: "answer")[`
print(42)
`]
```

Supported engines are `r`, `python`, `sh`, and `bash` as an alias for `sh`.
