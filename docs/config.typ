== `config.toml`

`Calepin` does not auto-discover configuration. For `compile` and `watch`, pass the config path explicitly with `--config=PATH`:

```bash
calepin compile --config .calepin/config.toml paper.typ
```

If `--config` is omitted, `Calepin` uses defaults.

If provided, `PATH` can be relative to the project root or absolute.

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