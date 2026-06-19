# tufte/

Material extracted from PR #39 (`add-tufte-template-starters`), kept separate so
it can be reviewed and ported into the current theme architecture without pulling
in the stale template subsystem the PR was built against.

Three pieces:

## 1. visual-identity.css

The Tufte look, reduced to only what differs from the built-in `academic` theme:

- Cream paper (`#fffff8`), ink text (`#12110f`), Palatino serif, brick-red links.
- Lighter heading weight.
- CSS-counter numbered sidenotes.
- MathML fallback styling for inline and display math.
- Tufte-toned code and output blocks.

The margin-note float layout, the responsive inline fallback, and the
endnote-to-sidenote behavior are already provided by `academic` and are not
repeated here.

## 2. margin-figure.css

The `.margin-figure` construct, which places a figure, table, or code output in
the right gutter. `academic` has text margin notes but no margin-figure, so this
is the one layout feature the PR adds on top of `academic`.

## 3. starter/

The literate-programming example as shipped in the PR: a rank-2 matrix
factorization walkthrough mixing prose, Typst math, Python chunks, an embedded
NumPy/Matplotlib heatmap, sidenotes, and margin figures.

- `tufte_starter.typ` is the document.
- `_tufte_literate.typ` holds the setup, sidenote/margin helpers, and the MathML
  fallback helpers, imported as `/_tufte_literate.typ`.
- `config.toml` points at a sibling `themes/` directory and a `.venv` Python
  interpreter; adjust the relative paths for wherever you run it.
- `tufte_starter.html` is the rendered artifact, for quick inspection.
