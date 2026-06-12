#import "@preview/calepin:0.0.1" as calepin
#import "/assets/academic.typ" as site

#set document(title: [About])

#calepin.setup(
  echo: true,
  eval: true,
  results: "render",
  fenced-chunks: true,
)

#site.profile(
  "Ada Scholar",
  title: "Associate Professor of Computational Social Science",
  affiliation: "Department of Example Studies, Example University",
  photo: "assets/profile.svg",
  links: (
    (label: "Email", url: "mailto:ada@example.edu"),
    (label: "ORCID", url: "https://orcid.org/0000-0000-0000-0000"),
    (label: "Google Scholar", url: "https://scholar.google.com/"),
    (label: "GitHub", url: "https://github.com/"),
    (label: "CV", url: "cv.html"),
  ),
)[
  I study computational methods, public policy, and reproducible research.
  This site is built from Typst sources, so the same structured data can drive
  articles, publication lists, course pages, and a CV.
]

#site.panels(
  [
    === Interests

    - Causal inference
    - Political economy
    - Reproducible computation
  ],
  [
    === Education

    - PhD, Example University
    - MA, Example University
    - BA, Example College
  ],
)

#site.selected-publications(count: 3)

#site.listing(
  "Teaching",
  kind: "teaching",
  count: 2,
  more: (label: "Courses", url: "teaching/index.html"),
)

#site.recent-posts(count: 3)
