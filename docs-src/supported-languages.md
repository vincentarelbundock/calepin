# Supported Languages

Calepin has built-in engines for **Python** and **R**. Diagram engines (**Mermaid**, **Graphviz DOT**, **TikZ**, **D2**) are also built in.

Any language with a [Jupyter kernel](https://github.com/jupyter/jupyter/wiki/Jupyter-kernels) works as an engine: use the kernel name as the chunk language. Popular examples include **Bash** (`bash`), **Julia** (`julia`), **Octave** (`octave`), **Gnuplot** (`gnuplot`), and **Ruby** (`ruby`).

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

Run `jupyter kernelspec list` to see what is registered. Whatever name appears in that list can be used as an engine name directly in a chunk:

````typ
```bash
echo "hello from bash"
```
````
