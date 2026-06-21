#set document(title: [Configuration])

#title()

= `config.toml`

`Calepin` does not auto-discover configuration. For `compile` and `watch`, pass the config path explicitly with `--config=PATH`:

```bash
calepin compile --config config.toml paper.typ
```

If `--config` is omitted, `Calepin` uses defaults.

Relative paths written inside the config resolve relative to the config file's own directory, not the document being compiled. Absolute paths are used as is.

`asset-dir` controls where Calepin writes generated runtime files and browser-facing assets, including theme CSS, JavaScript, and fonts. The default is `.calepin`, which works in local preview. Some hosts like Netlify do not serve files from dot-directories, so deployed pages can sometimes load without styling, search, dark-mode toggles, or other theme JavaScript even though they looked correct locally. For sites published to those hosts, set `asset-dir = "_calepin"` before building.

```toml
# config.toml

# Directory for generated Calepin runtime and website assets.
# Default: ".calepin". Use "_calepin" for Netlify or GitHub Pages.
asset-dir = ".calepin"

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
