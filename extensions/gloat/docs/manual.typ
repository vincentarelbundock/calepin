#import "@preview/tidy:0.4.3"

#let docs = tidy.parse-module({
  read("../src/core.typ")
  read("../src/bib.typ")
  read("../src/extra.typ")
  read("../src/util.typ")
})

#tidy.show-module(
  docs,
  style: tidy.styles.default,
  sort-functions: none,
)
