#import "/.calepin/calepin.typ" as calepin

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

#let chunk = calepin.chunk

#let python-figure(
  label: none,
  caption: none,
  alt: none,
  device-width: 6,
  device-height: 3.8,
  dpi: 160,
  width: 90%,
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
