#import "/.calepin/query-html.typ" as html

#set document(title: [Configuration])

#title()

= `config.toml`

`Calepin` does not auto-discover configuration. For `compile` and `watch`, pass the config path explicitly with `--config=PATH`:

```bash
calepin compile --config config.toml paper.typ
```

If `--config` is omitted, `Calepin` uses defaults.

Relative paths written inside the config resolve relative to the config file's
own directory, not the document being compiled. Absolute paths are used as is.

```toml
# config.toml

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
```

If `python` is not configured, set `CALEPIN_PYTHON` in the environment to
pick it. If that is absent, Calepin uses a local `.venv` interpreter when
available.
