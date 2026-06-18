#import "/.calepin/calepin.typ" as calepin

#set document(title: [Site exemple])
#metadata((title: "Accueil", translation_key: "home")) <website-metadata>

#calepin.setup(
  echo: true,
  eval: true,
  results: "verbatim",
  fenced-chunks: true,
)

#let target = sys.inputs.at("calepin-target", default: "paged")

#show: body => {
  if target == "html" {
    body
  } else {
    set page(columns: 2)
    body
  }
}

#title()

Ce scaffold est un petit site qui exerce les thèmes fournis avec Calepin.
Changez `theme` dans `calepin.toml` entre `calepin` et `academic` pour comparer
le même contenu dans différents agencements.

= Contenu de lecture

#if target == "html" {
  html.elem("img", "", attrs: (
    class: "calepin-float-right calepin-scaffold-portrait",
    src: "../assets/portrait.jpg",
    alt: "Portrait photographique",
    width: "1280",
    height: "1920",
    loading: "lazy",
    decoding: "async",
  ))
} else {
  place(
    top + right,
    float: true,
    clearance: 1em,
    image("/assets/portrait.jpg", width: 32%),
  )
}

#lorem(45)

#lorem(75)

Une note de bas de page française.#footnote[Les notes de bas de page permettent
de vérifier l'espacement dans le thème par défaut et les mises en page de lecture.]
La phrase suivante poursuit le paragraphe dans le flux normal.

#lorem(35)

= Code et résultat

```python
valeurs = [1, 1, 2, 3, 5]
print(sum(valeurs))
```

= Petit tableau

#table(
  columns: 3,
  [Thème], [Idéal pour], [À vérifier],
  [calepin], [Documentation], [Barre latérale, navigation, code],
  [academic], [Essais], [Notes de bas de page, largeur de lecture],
)

= Texte complémentaire

#lorem(80)
