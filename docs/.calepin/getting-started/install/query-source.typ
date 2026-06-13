#import "/.calepin/query-html.typ" as html

#set document(title: [Install])

#title()

= Typst CLI
<typst-cli>

Calepin requires Typst 0.14.2 or newer. Install or update the Typst CLI from the #link("https://github.com/typst/typst#installation")[Typst installation instructions], and make sure it is available on your `PATH`.

= Calepin
<calepin-cli>



MacOS and Linux:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/vincentarelbundock/calepin/releases/latest/download/calepin-installer.sh | sh
```

Windows via powershell:

```sh
powershell -ExecutionPolicy Bypass -c "irm https://github.com/vincentarelbundock/calepin/releases/latest/download/calepin-installer.ps1 | iex"
```

= Jupyter support
<jupyter-kernels>

Calepin has built-in support for #strong[Python] and #strong[R], and can also
run many other languages through Jupyter kernels, but that requires installing
the language kernel and kernel client tooling.

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
```

Run `jupyter kernelspec list` to see what engines are registered and available.
