#import "/.calepin/calepin.typ" as calepin
#import "/.calepin/calepin.typ" as calepin_runtime
#set document(title: [Install])

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
