
<div class="calepin-hero" markdown="0">
{{ inline_svg("docs-src/assets/logo_long_2.svg", "calepin-hero__logo") }}
<p class="calepin-hero__tagline">Computational Notebooks in {{ inline_svg("docs-src/assets/logo_typst.svg", "calepin-hero__typst") }}</p>
</div>

## What is Calepin?

#### Typst

[Typst](https://typst.app/) is a modern typesetting system that compiles plain text markup into rich documents. Think of it as a simple Markdown-like syntax, with the scriptability of LaTeX, and the ability to export to PDF, SVG, and HTML files. Typst is ultra fast, easy to learn, and expressive, which makes it a comfortable tool for anything from short letters to full scientific manuscripts.

#### Computational notebooks

A computational notebook mixes prose with executable code. Figures, tables, and numbers are computed when the document is rendered, rather than pasted-in by hand. This allows analysts to create reproducible reports in the tradition of [literate programming](https://en.wikipedia.org/wiki/Literate_programming). 

Some notebook tools can act as frontends for Typst, but they often superimpose their own syntax, language, and structure. For example, [Posit's Quarto](https://quarto.org/) supports an extended idiom of Markdown, which can be translated to Typst and then rendered as PDF.

#### Calepin

*Calepin* is a Typst-native computational notebook. Rather than hide Typst behind Markdown, it embeds executable code chunks directly inside `.typ` documents. *Calepin* scans a `.typ` file for inline code or exectuable chunks, evaluates them, and lets Typst render the final document with computed results in place. 

*Calepin* is language-agnostic. It can execute code in Python, R, and any language with a Jupyter kernel. It can also render diagrams using engines like Mermaid, Graphviz, TikZ, and D2.

*Calepin* comes with extensions for popular editors like VS-Code, Cursor, and Positron (via the Microsoft and VSX marketplaces).

- VS Code: [VincentArel-Bundock.calepin](https://marketplace.visualstudio.com/items?itemName=VincentArel-Bundock.calepin)
- Open VSX: [VincentArel-Bundock/calepin](https://open-vsx.org/extension/VincentArel-Bundock/calepin)

Here is a short video of a live editing session in VS-Code.

<div class="calepin-video-block" markdown="0">
  <a class="calepin-video-thumb" href="#calepin-video-lightbox" aria-label="Open Calepin editor preview video">
    <video class="calepin-video-thumb__media" src="assets/calepin_vscode.mp4" muted preload="metadata" playsinline></video>
    <span class="calepin-video-thumb__play" aria-hidden="true">▶</span>
  </a>
  <div id="calepin-video-lightbox" class="calepin-video-lightbox">
    <a class="calepin-video-lightbox__backdrop" href="#" aria-label="Close video preview"></a>
    <div class="calepin-video-lightbox__panel">
      <a class="calepin-video-lightbox__close" href="#" aria-label="Close video preview">×</a>
      <video class="calepin-video-lightbox__media" src="assets/calepin_vscode.mp4" muted autoplay controls playsinline preload="metadata"></video>
    </div>
  </div>
</div>

## Install calepin

MacOS and Linux:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/vincentarelbundock/calepin/releases/latest/download/calepin-installer.sh | sh
```

Windows via powershell:

```sh
powershell -ExecutionPolicy Bypass -c "irm https://github.com/vincentarelbundock/calepin/releases/latest/download/calepin-installer.ps1 | iex"
```

## Supported languages

Calepin has built-in engines for **Python** and **R**. Diagram engines (**Mermaid**, **Graphviz DOT**, **TikZ**, **D2**) are also built in.

Any language with a [Jupyter kernel](https://github.com/jupyter/jupyter/wiki/Jupyter-kernels) works as an engine: use the kernel name as the chunk language. Popular examples include **Bash** (`bash`), **Julia** (`julia`), **Octave** (`octave`), **Gnuplot** (`gnuplot`), **Ruby** (`ruby`), and **JavaScript** (`javascript`).

To use a Jupyter kernel, install `jupyter_client` first:

```sh
pip install jupyter_client
```

Most kernels then install with a single `pip install`:

```sh
pip install bash_kernel       # Bash
pip install octave_kernel     # GNU Octave
pip install gnuplot_kernel    # Gnuplot
```

Some kernels are installed from their language's own package manager:

```sh
# Julia
julia -e 'using Pkg; Pkg.add("IJulia")'

# JavaScript (Node.js)
npm install -g ijavascript && ijsinstall
```

Run `jupyter kernelspec list` to see what is registered. Whatever name appears in that list can be used as an engine name directly in a chunk:

````typ
```bash
echo "hello from bash"
```
````

## Render

Use `calepin compile` when you want to execute code chunks and render the notebook.
The command line shape is intentionally the same as `typst compile`: same output-first/format-driven arguments, with `--` pass-through for Typst flags, plus Calepin preprocessing.

```sh
calepin compile paper.typ --format pdf
calepin compile paper.typ --format html
calepin compile paper.typ {p}.svg --format svg

# explicit output path
calepin compile paper.typ path/to/paper.pdf --format pdf
```

Arguments after `--` are forwarded to Typst, so project-specific Typst flags can stay in the same command.

```sh
# open pdf in system viewer
calepin compile paper.typ -- --open 

# set path to font directory
calepin compile paper.typ -- --font-path fonts
```

## Watch

Use `calepin watch` while editing. It watches your source for changes, re-runs preprocessing, and delegates recompilation and previewing to `typst watch`.
The command form is the same as `typst watch`: same positional arguments and pass-through flags, with Calepin running its preprocessing step first.

```sh
calepin watch paper.typ
calepin watch paper.typ --format html
calepin watch paper.typ {p}.svg --format svg

Stop a running watch from the same project.
calepin stop paper.typ
```

The default output format is PDF. Choose another format with `--format`, or let the output extension select the format. 

Arguments after `--` are passed through to `typst watch`. Typst's `--open` flag opens the rendered output in the operating system's default viewer, and Typst's `--port` flag chooses the HTML preview port.

```sh
calepin watch example.typ -- --open
calepin watch paper.typ paper.html --format html -- --port 3001 --open
```

#### PDF viewer auto-refresh

Some PDF viewers do not automatically refresh a document when it is regenerated on disk. For example, macOS Preview may keep showing an older PDF until the window is focused, the file is reopened, or the application is restarted.

For smoother live preview, use a PDF viewer that reloads the file when it changes. On macOS, Skim is a good option. Other platforms have similar auto-reloading viewers, which are useful when working with tools that repeatedly rebuild PDFs.

## Example

Every document uses the runtime file *Calepin* writes to `.calepin/calepin.typ`. `calepin compile` and `calepin watch` run code chunks first, so the runtime and computed outputs are ready when Typst builds the document.

### Preamble

Start by importing the runtime and setting document-wide defaults with `calepin.setup()`.

````typ
#import ".calepin/calepin.typ"

#calepin.setup(
  echo: true,
  eval: true,
)
````

Define a short alias for Python inline computation.

````typ
#let py = calepin.inline.with("python")
````

### Chunks

A block chunk runs a piece of code and inserts its result. Start with a plain fenced block:

````typ
```python
x = 41
print(x + 1)
```
````

When you need extra control, use `#calepin.chunk` with options such as labels, captions, hiding source code, or changing how results are shown. If the body is a fenced block with a language, `#calepin.chunk` infers the engine from the fence:

````typ
#calepin.chunk(label: "answer")[
```python
x = 41
print(x + 1)
```
]
````

### Inline

An inline expression drops a computed value into the surrounding prose. It uses the same raw body contract and never takes a label.

```typ
The inline answer is #py[`print(40 + 2)`].
```

### All together now

Here is one full document example.

````typ
#import ".calepin/calepin.typ"

#calepin.setup(
  echo: true,
  results: "verbatim",
)

#let py = calepin.inline.with("python")

```python
x = 41
print(x + 1)
```

The inline answer is #py[`print(40 + 2)`].
````
