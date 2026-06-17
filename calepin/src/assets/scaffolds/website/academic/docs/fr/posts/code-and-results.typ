#import "@preview/calepin:0.0.1" as calepin

#set document(title: [Code et résultats])
#metadata((
  title: "Code et résultats",
  translation_key: "code-and-results",
  kind: "post",
  date: "2026-06-09",
  tags: ("code", "carnet"),
  summary: "Un billet avec du code exécutable pour vérifier le style.",
  slug: "code-et-resultats",
)) <website-metadata>

#calepin.setup(
  echo: true,
  eval: true,
  results: "render",
  fenced-chunks: true,
)

#title()

#lorem(50)

```r
plot(mpg ~ hp, data = mtcars)
```

#lorem(70)
