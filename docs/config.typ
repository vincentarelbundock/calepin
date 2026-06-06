== `config.toml`

`Calepin` looks for `.calepin/config.toml` in the project root to override:

- executable paths (`[executables]`)
- optional `themes_dir` for user HTML templates

Anything omitted falls back to Calepin defaults. Relative paths are resolved from
your project root; plain names like `python3` or `typst` keep using `PATH`.

```toml
# .calepin/config.toml

[executables]
typst = "typst"                    # Typst binary (can be name or full path)
python = ".venv/bin/python"         # Python interpreter
rscript = "Rscript"                # R executable
mmdc = "mmdc"                      # Mermaid
#dot = "/opt/homebrew/bin/dot"     # optional, if needed
#d2 = "d2"                          # optional
#tectonic = "tectonic"              # optional
#dvisvgm = "dvisvgm"                # optional
#pdf2svg = "pdf2svg"                # optional

themes_dir = "themes"               # folder for user template themes
```

If `python` is not configured, set `CALEPIN_PYTHON` in the environment to
pick it. If that is absent, Calepin uses a local `.venv` interpreter when
available.
