#import "/.calepin/calepin.typ": *



#let _raw-chunk-langs = ("python", "r", "mermaid", "dot", "tikz", "d2")
#show raw.where(block: true, lang: "typ", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "typst", theme: auto): it => _without-raw-chunk-transforms(() => _html-themed-raw-block(it))
#show raw.where(block: true, lang: "python", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("python", it) }
#show raw.where(block: true, lang: "r", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("r", it) }
#show raw.where(block: true, lang: "mermaid", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("mermaid", it) }
#show raw.where(block: true, lang: "dot", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("dot", it) }
#show raw.where(block: true, lang: "tikz", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("tikz", it) }
#show raw.where(block: true, lang: "d2", theme: auto): it => if _disable-raw-chunk-transforms.get() { _html-themed-raw-block(it) } else { chunk_from_raw_plain("d2", it) }

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

// Notebook theme
#import "/.calepin/calepin.typ": _html-themed-raw-block, chunk_from_raw_plain
#import "/.calepin/calepin.typ" as calepin
#import "@preview/marginalia:0.3.1" as marginalia

#let _superscript-numbering(pattern, ..i) = super(numbering(pattern, ..i))

// Place margin elements with marginalia in paged output. Page geometry stays
// under author control; add `marginalia.setup` in the document or a local theme
// when you want to reserve margin space.
#show raw.where(block: true): set text(size: .8em)

#calepin.elements.set-margin-impl(
  note: (body, numbering: auto, side: auto) => {
    let note-numbering = if numbering == auto { "1" } else { numbering }
    let marker-numbering = if type(note-numbering) == str {
      (..i) => _superscript-numbering(note-numbering, ..i)
    } else if type(note-numbering) == function {
      (..i) => super(note-numbering(..i))
    } else {
      note-numbering
    }
    let args = (numbering: marker-numbering)
    if marker-numbering != none {
      args.insert("anchor-numbering", marker-numbering)
    }
    if side != auto { args.insert("side", side) }
    marginalia.note(body, ..args)
  },
  figure: (body, caption: none, side: auto) => {
    let args = (:)
    if side != auto { args.insert("side", side) }
    marginalia.notefigure(body, caption: caption, ..args)
  },
)

#show raw.where(block: true): it => {
  if it.theme != auto {
    it
  } else if it.lang != none and _raw-chunk-langs.contains(it.lang) and _fenced-chunks-runs(
    it.lang,
    _resolve-options(it.lang, _call-defaults).at("fenced-chunks"),
  ) {
    chunk_from_raw_plain(it.lang, it)
  } else {
    _html-themed-raw-block(it)
  }
}

#import "/.calepin/calepin.typ" as calepin_runtime
// Imports
#import "/.calepin/calepin.typ" as calepin
#import "@preview/marginalia:0.3.1" as marginalia

// Margin notes
#show: marginalia.setup.with(
  outer: (far: 6mm, width: 50mm, sep: 6mm),
  inner: (far: 2.2cm, width: 0mm, sep: 0mm),
  top: 2.0cm,
  bottom: 2.0cm,
  book: false,
)
#let sidenote = calepin.elements.sidenote.with(side: "right")
#let sidefigure = calepin.elements.sidefigure.with(side: "right")

// Style text and headings
#set text(size: 11pt)
#set par(
  leading: 0.45em,
  first-line-indent: 1.15em,
  spacing: 0.9em,
)
#set heading(numbering: none)
#show title: smallcaps
#show title: set text(weight: "regular")
#show heading.where(level: 1): set text(size: 1em, style: "italic", weight: "regular")

// Figures and tables
#set figure(gap: 0.55em)
#set table(stroke: none)
#set table(inset: 0.45em)

// Executable code chunks
#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
)

// Title
#set document(title: [Matrix factorization with Calepin])
#title()

Matrix factorization represents a rectangular data matrix as the product of
two thinner matrices. In recommender systems, the rows are users, the columns
are items, and the observed entries are ratings. A rank $k$ model writes

$ hat(R)_(i, j) = mu + u_i^T v_j $

where $u_i$ is a user vector, $v_j$ is an item vector, and
$mu$ is the global average rating. The dot product is large when the
user and item point in similar latent directions.#sidenote(numbering: auto)[The sign and
rotation of the latent dimensions are not identified. What matters for
prediction is the dot product, not a unique interpretation of each axis.]

= A small ratings matrix

We will fit a rank-2 model to a tiny ratings matrix. Missing values are written
as `None`.

#calepin_runtime.chunk_from_raw_plain("python", raw("import math\nimport random\n\nusers = [\"Ada\", \"Ben\", \"Cy\", \"Dee\", \"Eli\"]\nitems = [\"Linear algebra\", \"Optimization\", \"Sci-fi\", \"Cooking\"]\n\nratings = [\n    [5.0, 4.0, None, 1.0],\n    [4.0, None, None, 1.0],\n    [1.0, 1.0, 5.0, 4.0],\n    [None, 1.0, 4.0, 5.0],\n    [5.0, None, 1.0, None],\n]\n\nobserved = [\n    (i, j, value)\n    for i, row in enumerate(ratings)\n    for j, value in enumerate(row)\n    if value is not None\n]\n\ndef show_matrix(matrix):\n    print(\"          \" + \"  \".join(f\"{name[:4]:>4}\" for name in items))\n    for name, row in zip(users, matrix):\n        cells = [\"   .\" if value is None else f\"{value:4.1f}\" for value in row]\n        print(f\"{name:>4}      \" + \"  \".join(cells))\n\nshow_matrix(ratings)\n", block: true, lang: "python"))

The model should use the observed entries to fill in the missing cells. With
only two latent dimensions, it has to compress the observed pattern rather than
memorize every rating separately.

= A synthetic rank-2 heatmap

Before fitting the ratings data, it is useful to look at an exactly rank-2
matrix. The following chunk imports NumPy, generates two skinny factors, and
plots their product with Matplotlib.

#calepin.chunk(
  "python",
  label: "fig-rank-two-heatmap",
  fig-caption: [Synthetic rank-2 matrix],
  fig-alt-text: "Heatmap of a generated rank-2 matrix",
  fig-device-format: "png",
  fig-device-width: 6,
  fig-device-height: 3.8,
  fig-device-dpi: 160,
  fig-width: 80%,
)[```python
import numpy as np
import matplotlib.pyplot as plt

rng = np.random.default_rng(7)
left = rng.normal(size=(18, 2))
right = rng.normal(size=(2, 14))
rank_two = left @ right

plt.figure(figsize=(6, 3.8))
image = plt.imshow(rank_two, aspect="auto", cmap="viridis")
plt.title("Synthetic rank-2 matrix")
plt.xlabel("column")
plt.ylabel("row")
plt.colorbar(image, fraction=0.046, pad=0.04, label="value")
plt.tight_layout()
```]

In HTML output, this figure is a useful asset-inlining check: a self-contained
render should embed the generated PNG directly in the page instead of linking
to a file under `.calepin/`.#sidenote(numbering: none)[#lorem(25)]

= Fitting a rank-2 model

The loss is squared error on observed ratings plus a small penalty that keeps
the latent vectors from growing too large:

$
  L = sum_((i, j) in Omega) (r_(i, j) - hat(R)_(i, j))^2
    + lambda (sum_i ||u_i||^2 + sum_j ||v_j||^2)
  $

Here is a simple stochastic gradient descent fit.

#calepin_runtime.chunk_from_raw_plain("python", raw("random.seed(12)\n\nrank = 2\nlearning_rate = 0.025\nregularization = 0.02\nepochs = 2400\n\nmu = sum(value for _, _, value in observed) / len(observed)\nuser_factors = [\n    [random.uniform(-0.35, 0.35) for _ in range(rank)]\n    for _ in users\n]\nitem_factors = [\n    [random.uniform(-0.35, 0.35) for _ in range(rank)]\n    for _ in items\n]\n\ndef dot(left, right):\n    return sum(a * b for a, b in zip(left, right))\n\ndef predict(i, j):\n    return mu + dot(user_factors[i], item_factors[j])\n\ndef rmse():\n    total = 0.0\n    for i, j, value in observed:\n        total += (value - predict(i, j)) ** 2\n    return math.sqrt(total / len(observed))\n\ncheckpoints = []\nfor epoch in range(epochs + 1):\n    if epoch in (0, 25, 100, 400, 1200, 2400):\n        checkpoints.append((epoch, rmse()))\n    random.shuffle(observed)\n    for i, j, value in observed:\n        error = value - predict(i, j)\n        old_user = user_factors[i][:]\n        old_item = item_factors[j][:]\n        for d in range(rank):\n            user_factors[i][d] += learning_rate * (\n                error * old_item[d] - regularization * old_user[d]\n            )\n            item_factors[j][d] += learning_rate * (\n                error * old_user[d] - regularization * old_item[d]\n            )\n\nfor epoch, value in checkpoints:\n    print(f\"epoch {epoch:4d}: observed RMSE = {value:.3f}\")\n", block: true, lang: "python"))

#sidefigure[
#calepin.chunk("python", echo: false, results: "typst")[```python
def cell(value):
    return f"[{value}]"

rows = [
    "table.hline()",
    cell("*Item*"), cell("factor 1"), cell("factor 2"),
    "table.hline()",
]
for name, vector in zip(items, item_factors):
    rows.extend([cell(name), cell(f"{vector[0]:.2f}"), cell(f"{vector[1]:.2f}")])
rows.append("table.hline()")

fragment = (
    "#figure("
    "table(columns: 3, align: (left, left, left), inset: 0.28em, "
    + ", ".join(rows)
    + "), caption: [Learned item factors])"
)
print(fragment)
```]
]

The margin table is computed from the trained model. The exact coordinate
system is arbitrary, but nearby items have similar factor vectors.

= Completing the matrix

The fitted model can now predict the missing ratings.

#calepin_runtime.chunk_from_raw_plain("python", raw("completed = []\nfor i, row in enumerate(ratings):\n    out = []\n    for j, value in enumerate(row):\n        out.append(value if value is not None else predict(i, j))\n    completed.append(out)\n\nshow_matrix(completed)\n", block: true, lang: "python"))

The same fitted object can also list only the originally missing entries.

#calepin_runtime.chunk_from_raw_plain("python", raw("for i, row in enumerate(ratings):\n    for j, value in enumerate(row):\n        if value is None:\n            print(f\"{users[i]:>3} -> {items[j]:<14} {predict(i, j):.2f}\")\n", block: true, lang: "python"))

= What to check

#calepin_runtime.chunk_from_raw_plain("r", raw("#| label: fig-blahblah\n#| fig-width: 100%\n#| results: hide\nplot(mpg ~ hp, data = mtcars)\n", block: true, lang: "r"))

Matrix factorization is useful because it shares information across rows and
columns.#sidefigure()[#calepin.results("fig-blahblah")] But this sharing is
also a modeling assumption. A practical workflow should check prediction error
on held-out observed entries, tune the rank $k$, and inspect whether the
completed matrix makes domain sense. Calepin is useful here because the prose,
the fitted model, and the diagnostics all live in the same executable document.
