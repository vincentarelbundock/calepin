= Supported Languages
<supported-languages>

Calepin has built-in engines for #strong[Python] and #strong[R]. Diagram
engines (#strong[Mermaid], #strong[Graphviz DOT], #strong[TikZ],
#strong[D2]) are also built in.

Any language with a
#link("https://github.com/jupyter/jupyter/wiki/Jupyter-kernels")[Jupyter kernel]
works as an engine: use the kernel name as the chunk language. Popular
examples include #strong[Bash] (`bash`), #strong[Julia] (`julia`),
#strong[Octave] (`octave`), #strong[Gnuplot] (`gnuplot`), and
#strong[Ruby] (`ruby`). Install Jupyter kernels per
#link("../getting-started/install.html#jupyter-support")[the Jupyter install section].

Run `jupyter kernelspec list` to see what is registered. Whatever name
appears in that list can be used as an engine name directly in a chunk:

````typ
```bash
echo "hello from bash"
```
````
