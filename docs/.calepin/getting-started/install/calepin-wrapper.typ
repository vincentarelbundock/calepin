#let _calepin-document-element = document
#import "/.calepin/calepin.typ": *
#let document = _calepin-document-element



#let _raw-chunk-langs = ("python", "r", "mermaid", "dot", "tikz", "d2", "sh")
#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("d2", it) }
#show raw.where(block: true, lang: "sh", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("sh", it) }

#show raw.where(block: true, theme: auto): it => {
  if _is-query() {
    it
  } else if _disable-raw-chunk-transforms.get() {
    _html-themed-raw-block(it)
  } else if it.has("lang") and it.lang != none and _raw-chunk-langs.contains(it.lang) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

#show heading: it => {
  if _is-html() and "label" in it.fields() {
    std.html.elem("calepin-heading-anchor", attrs: (data-id: str(it.label)))
  }
  it
}

// Notebook theme
#import "/.calepin/calepin.typ": _html-themed-raw-block, chunk_from_raw_plain

// Body text size, captured below at document-body level. Code blocks are sized
// relative to this rather than to `1em`, which would compound: a literal
// ```typ block is rendered by replacing its source `raw` element, so it renders
// inside Typst's already-reduced raw text context, whereas executed chunks are
// emitted as ordinary calls at body size. Anchoring to the captured body size
// gives both paths a single, matching reduction instead of shrinking twice.
#let _calepin-body-size = std.state("calepin-body-size", 11pt)

#show raw.where(block: true): it => {
  if it.theme != auto {
    context {
      set text(size: _calepin-body-size.get() * 0.8)
      it
    }
  } else if it.lang != none and _raw-chunk-langs.contains(it.lang) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

#context _calepin-body-size.update(text.size)

#import "/.calepin/calepin.typ" as calepin
#import "/.calepin/calepin.typ" as calepin_runtime
#set document(title: [Install])
#metadata((tags: ("getting started", "installation"))) <website-metadata>

#title()

= Typst CLI
<typst-cli>

Calepin requires Typst 0.15.0 or newer. Install or update the Typst CLI from the #link("https://github.com/typst/typst#installation")[Typst installation instructions], and make sure it is available on your `PATH`.

= Calepin
<calepin-cli>

The simplest way to install Calepin is with the official installer script, which works on MacOS and Linux:

#calepin_runtime.chunk_from_raw_plain("sh", raw("curl --proto '=https' --tlsv1.2 -LsSf https://github.com/vincentarelbundock/calepin/releases/latest/download/calepin-installer.sh | sh\n", block: true, lang: "sh"))

On Windows via powershell:

#calepin_runtime.chunk_from_raw_plain("sh", raw("powershell -ExecutionPolicy Bypass -c \"irm https://github.com/vincentarelbundock/calepin/releases/latest/download/calepin-installer.ps1 | iex\"\n", block: true, lang: "sh"))

If you are a `cargo` for Rust user, you can install with:

#calepin_runtime.chunk_from_raw_plain("sh", raw("cargo install calepin\n", block: true, lang: "sh"))

== Updating Calepin

If you installed Calepin with the official installer, update it with:

#calepin_runtime.chunk_from_raw_plain("sh", raw("calepin update\n", block: true, lang: "sh"))

This updates only Calepin, using the `calepin-update` helper installed alongside
the main binary. Typst, Python, R, Jupyter, and Jupyter kernels are managed
separately.

If `calepin update` reports that `calepin-update` is missing, reinstall Calepin
with the official installer command above. If you installed Calepin with Cargo,
Homebrew, or another package manager, use that tool to upgrade Calepin instead.

= Jupyter support
<jupyter-kernels>

Calepin has built-in support for #strong[Python] and #strong[R], and can also
run many other languages through Jupyter kernels, but that requires installing
the language kernel and kernel client tooling.

To use a Jupyter kernel, install `jupyter_client` first:

#calepin_runtime.chunk_from_raw_plain("sh", raw("pip install jupyter_client\n", block: true, lang: "sh"))

Most kernels then install with a single `pip install`:

#calepin_runtime.chunk_from_raw_plain("sh", raw("pip install bash_kernel       # Bash\npip install octave_kernel     # GNU Octave\npip install gnuplot_kernel    # Gnuplot\n", block: true, lang: "sh"))

Some Python kernel packages also need to register a Jupyter kernelspec after
installation. For Bash, run this in the same Python environment that Calepin
uses:

#calepin_runtime.chunk_from_raw_plain("sh", raw("python -m bash_kernel.install --sys-prefix\n", block: true, lang: "sh"))

If you use `uv run`, the equivalent command is:

#calepin_runtime.chunk_from_raw_plain("sh", raw("uv run python -m bash_kernel.install --sys-prefix\n", block: true, lang: "sh"))

Some kernels are installed from their language's own package manager:

#calepin_runtime.chunk_from_raw_plain("sh", raw("# Julia\njulia -e 'using Pkg; Pkg.add(\"IJulia\")'\n", block: true, lang: "sh"))

Run `jupyter kernelspec list` to see what engines are registered and available.

= Nix
<nix>

If you use Nix flakes, the default package is the basic Calepin CLI wrapped with
Typst on `PATH`:

#calepin_runtime.chunk_from_raw_plain("sh", raw("nix run github:vincentarelbundock/calepin -- --help\nnix run github:vincentarelbundock/calepin#calepin -- compile paper.typ\n", block: true, lang: "sh"))

To build the package without running it:

#calepin_runtime.chunk_from_raw_plain("sh", raw("nix build github:vincentarelbundock/calepin\n", block: true, lang: "sh"))

The default development shell is also minimal:

#calepin_runtime.chunk_from_raw_plain("sh", raw("nix develop github:vincentarelbundock/calepin\n", block: true, lang: "sh"))

For contributor work on the documentation website, use the heavier website
shell. It includes Rust tooling, Typst, `uv`, R packages used by the examples,
and the diagram tools used by the website pages.

#calepin_runtime.chunk_from_raw_plain("sh", raw("nix develop github:vincentarelbundock/calepin#website\nmake website\n", block: true, lang: "sh"))
