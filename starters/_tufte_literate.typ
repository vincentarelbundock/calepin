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

#let html-target = sys.inputs.at("calepin-target", default: "paged") == "html"

#let sidenote(body) = {
  if html-target {
    std.html.elem("span", attrs: (class: "sidenote"))[#body]
  } else {
    footnote(body)
  }
}

#let marginfig(body) = {
  if html-target {
    std.html.elem("aside", attrs: (class: "margin-figure"))[#body]
  } else {
    body
  }
}

#let margin-python(body) = marginfig[
  #chunk("python", echo: false, results: "typst")[#body]
]

#let inline-math(html, math) = {
  if html-target {
    std.html.elem("span", attrs: (class: "calepin-math-inline"))[#html()]
  } else {
    math
  }
}

#let display-math(html, math) = {
  if html-target {
    std.html.elem("div", attrs: (class: "calepin-math-display"))[#html()]
  } else {
    math
  }
}

#let math-inline(body) = std.html.elem("math")[#body]
#let math-display(body) = std.html.elem("math", attrs: (display: "block"))[#body]
#let mi(body) = std.html.elem("mi")[#body]
#let mn(body) = std.html.elem("mn")[#body]
#let mo(body) = std.html.elem("mo")[#body]
#let mrow(body) = std.html.elem("mrow")[#body]
#let mover(base, over) = std.html.elem("mover")[#base #over]
#let msub(base, sub) = std.html.elem("msub")[#base #sub]
#let msup(base, sup) = std.html.elem("msup")[#base #sup]

#let html-k() = math-inline[#mi[k]]
#let html-u-i() = math-inline[#msub([#mi[u]], [#mi[i]])]
#let html-v-j() = math-inline[#msub([#mi[v]], [#mi[j]])]
#let html-mu() = math-inline[#mi[#sym.mu]]
#let html-r-hat-ij() = msub(
  [#mover([#mi[R]], [#mo[^]])],
  [#mrow[#mi[i]#mo[,]#mi[j]]],
)
#let html-rating-model() = math-display[
  #html-r-hat-ij()
  #mo[=]
  #mi[#sym.mu]
  #mo[+]
  #msup([#msub([#mi[u]], [#mi[i]])], [#mi[T]])
  #msub([#mi[v]], [#mi[j]])
]
#let html-loss() = math-display[
  #mi[L]
  #mo[=]
  #msub(
    [#mo[#sym.sum]],
    [#mrow[#mo[(]#mi[i]#mo[,]#mi[j]#mo[)]#mo[#sym.in]#mi[#sym.Omega]]],
  )
  #msup(
    [
      #mrow[
        #mo[(]
        #msub([#mi[r]], [#mrow[#mi[i]#mo[,]#mi[j]]])
        #mo[-]
        #html-r-hat-ij()
        #mo[)]
      ]
    ],
    [#mn[2]],
  )
  #mo[+]
  #mi[#sym.lambda]
  #mrow[
    #mo[(]
    #msub([#mo[#sym.sum]], [#mi[i]])
    #msup([#mrow[#mo[||]#msub([#mi[u]], [#mi[i]])#mo[||]]], [#mn[2]])
    #mo[+]
    #msub([#mo[#sym.sum]], [#mi[j]])
    #msup([#mrow[#mo[||]#msub([#mi[v]], [#mi[j]])#mo[||]]], [#mn[2]])
    #mo[)]
  ]
]

#let rank-k = inline-math(html-k, [$ k $])
#let user-vector = inline-math(html-u-i, [$ u_i $])
#let item-vector = inline-math(html-v-j, [$ v_j $])
#let global-mean = inline-math(html-mu, [$ mu $])
#let rating-model = display-math(html-rating-model, [
  $
    hat(R)_(i,j) = mu + u_i^T v_j
  $
])
#let factorization-loss = display-math(html-loss, [
  $
    L = sum_((i,j) in Omega) (r_(i,j) - hat(R)_(i,j))^2
      + lambda (sum_i ||u_i||^2 + sum_j ||v_j||^2)
  $
])
