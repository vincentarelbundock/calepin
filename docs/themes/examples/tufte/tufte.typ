#import "/.calepin/calepin.typ" as calepin
#import "@preview/marginalia:0.3.1" as marginalia

#show: marginalia.setup.with(
  outer: (far: 6mm, width: 50mm, sep: 6mm),
  inner: (far: 2.2cm, width: 0mm, sep: 0mm),
  top: 2.0cm,
  bottom: 2.0cm,
  book: false,
)

#set page(
  paper: "us-letter",
  numbering: "1",
  number-align: right,
)

#let sidenote = calepin.elements.sidenote.with(side: "right")
#let sidefigure = calepin.elements.sidefigure.with(side: "right")

#set text(size: 10pt)
#set par(
  leading: 0.45em,
  first-line-indent: 1.15em,
  spacing: 0.9em,
)

#set heading(numbering: none)
#show heading: set align(left)
#show heading.where(level: 1): smallcaps
#show heading.where(level: 1): set text(size: 1.1em, weight: "regular")
#show heading.where(level: 2): set text(size: 10.5pt, style: "italic", weight: "regular")

#set figure(gap: 0.55em)
#set table(stroke: none)
#set table(inset: 0.45em)

#let chunk = calepin.chunk
#let newthought = smallcaps

#let setup(
  title: none,
  echo: true,
  eval: true,
  results: "verbatim",
) = {
  if title != none {
    set document(title: title)
  }

  calepin.setup(
    echo: echo,
    eval: eval,
    results: results,
  )
}

#setup(title: [Matrix factorization with Calepin])

#let python-figure(
  label: none,
  caption: none,
  alt: none,
  device-width: 6,
  device-height: 3.8,
  dpi: 160,
  width: 80%,
  body,
) = chunk(
  "python",
  label: label,
  fig-caption: caption,
  fig-alt-text: alt,
  fig-device-format: "png",
  fig-device-width: device-width,
  fig-device-height: device-height,
  fig-device-dpi: dpi,
  fig-width: width,
)[#body]

#let rank-k = $k$
#let user-vector = $u_i$
#let item-vector = $v_j$
#let global-mean = $mu$
#let rating-model = [
  $ hat(R)_(i, j) = mu + u_i^T v_j $
]
#let factorization-loss = [
  $
    L = sum_((i, j) in Omega) (r_(i, j) - hat(R)_(i, j))^2
      + lambda (sum_i ||u_i||^2 + sum_j ||v_j||^2)
  $
]


= Matrix factorization with Calepin

Matrix factorization represents a rectangular data matrix as the product of
two thinner matrices. In recommender systems, the rows are users, the columns
are items, and the observed entries are ratings. A rank #rank-k model writes

#rating-model

where #user-vector is a user vector, #item-vector is an item vector, and
#global-mean is the global average rating. The dot product is large when the
user and item point in similar latent directions.#sidenote(numbering: auto)[The sign and
rotation of the latent dimensions are not identified. What matters for
prediction is the dot product, not a unique interpretation of each axis.]

== A small ratings matrix

We will fit a rank-2 model to a tiny ratings matrix. Missing values are written
as `None`.

```python
import math
import random

users = ["Ada", "Ben", "Cy", "Dee", "Eli"]
items = ["Linear algebra", "Optimization", "Sci-fi", "Cooking"]

ratings = [
    [5.0, 4.0, None, 1.0],
    [4.0, None, None, 1.0],
    [1.0, 1.0, 5.0, 4.0],
    [None, 1.0, 4.0, 5.0],
    [5.0, None, 1.0, None],
]

observed = [
    (i, j, value)
    for i, row in enumerate(ratings)
    for j, value in enumerate(row)
    if value is not None
]

def show_matrix(matrix):
    print("          " + "  ".join(f"{name[:4]:>4}" for name in items))
    for name, row in zip(users, matrix):
        cells = ["   ." if value is None else f"{value:4.1f}" for value in row]
        print(f"{name:>4}      " + "  ".join(cells))

show_matrix(ratings)
```

The model should use the observed entries to fill in the missing cells. With
only two latent dimensions, it has to compress the observed pattern rather than
memorize every rating separately.

== A synthetic rank-2 heatmap

Before fitting the ratings data, it is useful to look at an exactly rank-2
matrix. The following chunk imports NumPy, generates two skinny factors, and
plots their product with Matplotlib.

#python-figure(
  label: "fig-rank-two-heatmap",
  caption: [Synthetic rank-2 matrix],
  alt: "Heatmap of a generated rank-2 matrix",
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

== Fitting a rank-2 model

The loss is squared error on observed ratings plus a small penalty that keeps
the latent vectors from growing too large:

#factorization-loss

Here is a simple stochastic gradient descent fit.

```python
random.seed(12)

rank = 2
learning_rate = 0.025
regularization = 0.02
epochs = 2400

mu = sum(value for _, _, value in observed) / len(observed)
user_factors = [
    [random.uniform(-0.35, 0.35) for _ in range(rank)]
    for _ in users
]
item_factors = [
    [random.uniform(-0.35, 0.35) for _ in range(rank)]
    for _ in items
]

def dot(left, right):
    return sum(a * b for a, b in zip(left, right))

def predict(i, j):
    return mu + dot(user_factors[i], item_factors[j])

def rmse():
    total = 0.0
    for i, j, value in observed:
        total += (value - predict(i, j)) ** 2
    return math.sqrt(total / len(observed))

checkpoints = []
for epoch in range(epochs + 1):
    if epoch in (0, 25, 100, 400, 1200, 2400):
        checkpoints.append((epoch, rmse()))
    random.shuffle(observed)
    for i, j, value in observed:
        error = value - predict(i, j)
        old_user = user_factors[i][:]
        old_item = item_factors[j][:]
        for d in range(rank):
            user_factors[i][d] += learning_rate * (
                error * old_item[d] - regularization * old_user[d]
            )
            item_factors[j][d] += learning_rate * (
                error * old_user[d] - regularization * old_item[d]
            )

for epoch, value in checkpoints:
    print(f"epoch {epoch:4d}: observed RMSE = {value:.3f}")
```

#sidefigure[
#chunk("python", echo: false, results: "typst")[```python
def cell(value):
    return f"[{value}]"

rows = [cell("*Item*"), cell("factor 1"), cell("factor 2")]
for name, vector in zip(items, item_factors):
    rows.extend([cell(name), cell(f"{vector[0]:.2f}"), cell(f"{vector[1]:.2f}")])

fragment = (
    "#figure("
    "table(columns: 3, inset: 0.28em, "
    + ", ".join(rows)
    + "), caption: [Learned item factors])"
)
print(fragment)
```]
]

The margin table is computed from the trained model. The exact coordinate
system is arbitrary, but nearby items have similar factor vectors.

== Completing the matrix

The fitted model can now predict the missing ratings.

```python
completed = []
for i, row in enumerate(ratings):
    out = []
    for j, value in enumerate(row):
        out.append(value if value is not None else predict(i, j))
    completed.append(out)

show_matrix(completed)
```

The same fitted object can also list only the originally missing entries.

```python
for i, row in enumerate(ratings):
    for j, value in enumerate(row):
        if value is None:
            print(f"{users[i]:>3} -> {items[j]:<14} {predict(i, j):.2f}")
```

== What to check

```r
#| label: fig-blahblah
#| fig-width: 100%
#| results: hide
plot(mpg ~ hp, data = mtcars)
```

Matrix factorization is useful because it shares information across rows and
columns.#sidefigure()[#calepin.results("fig-blahblah")] But this sharing is
also a modeling assumption. A practical workflow should check prediction error
on held-out observed entries, tune the rank #rank-k, and inspect whether the
completed matrix makes domain sense. Calepin is useful here because the prose,
the fitted model, and the diagnostics all live in the same executable document.
