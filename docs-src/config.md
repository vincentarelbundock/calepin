---
title: Configuration
---

## `config.toml`

Calepin reads executable paths from `.calepin/config.toml` under the Typst project root. If the file is absent, Calepin uses the current defaults and resolves bare command names on `PATH`.

Relative path-like values are resolved from the project root. Bare command names such as `python3`, `Rscript`, or `typst` are left as command names and resolved by the operating system.

The `CALEPIN_PYTHON` environment variable can set the default Python executable when `python` is not configured in `.calepin/config.toml`. The VS Code extension uses this to pass the interpreter selected by the Python extension to Calepin Watch and Calepin Compile.

## Executable paths

Executable paths are configured in `.calepin/config.toml` under the project root. See [Configuration](config.html) for examples.

```toml
[executables]
python = ".venv/bin/python"
```

## Minimal Python Environment

Use this when a project keeps Python packages in a local virtual environment.

```toml
[executables]
python = ".venv/bin/python"
```

## Full Executable Set

Any omitted value falls back to the default shown here.

```toml
[executables]
typst = "typst"
python = "python3"
rscript = "Rscript"
julia = "julia"
shell = "/bin/sh"
mmdc = "mmdc"
dot = "dot"
tectonic = "tectonic"
dvisvgm = "dvisvgm"
pdf2svg = "pdf2svg"
d2 = "d2"
```

## Absolute Paths

Use absolute paths when executables are outside the project or should not depend on `PATH`.

```toml
[executables]
typst = "/opt/homebrew/bin/typst"
python = "/Users/me/projects/report/.venv/bin/python"
rscript = "/opt/homebrew/bin/Rscript"
```

## Diagram Tools

Diagram chunks use external command-line tools. Configure only the tools needed by the engines in your document.

```toml
[executables]
mmdc = "node_modules/.bin/mmdc"
dot = "/opt/homebrew/bin/dot"
d2 = "/opt/homebrew/bin/d2"
```

## Mermaid Chrome

If Mermaid cannot find Chrome through Puppeteer, set `chrome` explicitly.

```toml
[executables]
mmdc = "node_modules/.bin/mmdc"
chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
```

## Template Themes Directory

User-authored HTML themes live in a project directory, `themes/` by default. The
`themes_dir` key overrides that location. Relative values resolve from the
project root; absolute values are used as given.

```toml
themes_dir = "site/themes"
```

A template theme is a directory named after the value passed to `--template`. It holds a
required `layout.html` plus optional `partials/`, `styles/`, and `scripts/`
subdirectories. A user theme whose directory name matches a built-in (such as
`pico` or `basic`) takes precedence over the built-in. See
[Document Options](index.html) for how template themes are applied to HTML output.
