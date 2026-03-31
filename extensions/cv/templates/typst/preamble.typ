{% raw %}
// gloat by Elliott Weix (Unlicense)
// https://github.com/eweix/gloat

// ===========================================================================
// core.typ
// ===========================================================================

#let cv(
  author: "",
  address: "",
  contacts: (),
  updated: datetime.today(),
  body,
) = {
  set document(author: author, title: author, date: updated)
  set text(size: 11pt, lang: "en")
  set page(
    margin: (top: 1.25cm, bottom: 1.25cm, left: 1.5cm, right: 1.5cm),
    footer: [
      #align(center)[
        #author -- CV -- #context { counter(page).display("1 of 1", both: true) }
      ]
    ],
  )
  show heading: it => text(size: 12pt, it.body)
  show heading.where(level: 1): it => pad(bottom: 12pt, smallcaps(it))
  show heading.where(level: 2): it => pad(bottom: 0pt, it)

  align(center)[
    #block(text(size: 14pt, weight: 700, [#smallcaps(author)]))
  ]
  pad(
    top: 2pt,
    align(center)[
      #smallcaps[#contacts.join("  |  ")]
    ],
  )
  if address != "" {
    align(center)[#smallcaps[#address]]
  }
  set par(justify: true)
  body
}

#let edu(
  institution: "",
  date: "",
  degrees: (),
  location: "",
  gpa: "",
  details: "",
) = {
  [#grid(
      columns: (auto, 1fr),
      align(left)[
        #{
          for degree in degrees [
            #strong[#degree] \
          ]
        }
        #institution
        \ #{
          if gpa != "" [
            GPA: #gpa
          ]
        }
      ],
      align(right)[
        #{ if location != "" { location } }
        #{
          if type(date) == datetime [
            \ #date.display("[month repr:long] [year]")
          ] else [
            \ #date
          ]
        }
      ],
    )
    #{ if details != "" [#details] }
  ]
}

#let exp(
  role: "",
  org: "",
  start: "",
  end: "",
  location: "",
  summary: "",
  details: [],
) = {
  [#grid(
      columns: (auto, 1fr),
      align(left)[
        #strong[#role]
        \ #org
        #{
          if summary != "" [
            \ #summary
          ]
        }
      ],
      align(right)[
        #{
          if location != "" [
            #location
          ]
        }
        #text[
          \ #{
            if type(start) == datetime {
              start.display("[month repr:long] [year]")
            } else { start }
          } #{
            if end != "" [
              #{
                if type(end) == datetime {
                  end.display("- [month repr:long] [year]")
                } else [\- #end]
              }
            ]
          }]
      ],
    ) #details]
}

#let ser(
  role: "",
  org: "",
  start: "",
  end: "",
  summary: none,
) = {
  grid(
    columns: (auto, 1fr),
    align(left)[
      #org, #strong[#role]
      #{
        if summary != none [
          \ #summary
        ]
      }
    ],
    align(right)[
      #text[
        #{
          if type(start) == datetime {
            start.display("[month repr:long] [year]")
          } else { start }
        } #{
          if end != "" [
            #{
              if type(end) == datetime {
                end.display("- [month repr:long] [year]")
              } else [\- #end]
            }
          ]
        }]
    ],
  )
}

#let award(
  name: "",
  date: "",
  from: "",
  amt: "",
  details: "",
) = {
  grid(
    columns: (3em, auto, 3em),
    align(left)[
      #{ if type(date) == datetime [#date.display("[year]")] else [#date] }
    ],
    align(left)[
      #strong[#name,] #text[#from. #details]
    ],
  )
}

#let skills(areas) = {
  for area in areas {
    strong[#area.at(0): ]
    area.at(1).join(" | ")
    linebreak()
  }
}

// ===========================================================================
// bib.typ
// ===========================================================================

#let cv-abstract(
  authors: (),
  title: "",
  conference: "",
  number: "",
  pages: "",
  date: "",
  kind: "",
  location: "",
  DOI: none,
) = {
  let credit = (
    { if pages != "" [#pages,] else [] },
    { if kind != "" [ Abstract and #kind] else [ Abstract] },
    { if number != "" [ #number] },
  )
    .enumerate()
    .map(((i, cred)) => { if cred != none { [#cred] } else { none } })
    .join()

  enum.item[
    #{ if type(authors) == array { authors.enumerate().map(((i, author)) => text(author)).join(", ") } else { authors } }.
    #title.
    #emph[#conference],
    #location\;
    #credit.
    #{
      if DOI != none [DOI: #link("https://doi.org" + DOI)[#DOI]]
    }
  ]
}

#let paper(
  authors: (),
  title: "",
  journal: none,
  published: "",
  vol: none,
  issue: none,
  pages: none,
  DOI: none,
  show-link: false,
) = {
  let date = {
    if type(published) == datetime {
      strong[#published.display("[year]")]
    } else if type(published) == content or type(published) == str {
      strong[#published]
    }
  }
  let credit = (
    { if journal != none { [#emph(journal) #date] } else { [#date] } },
    { if vol != none [, #vol#{ if issue != none [ (#issue)] }] },
    { if pages != none [, #pages] },
  )
    .enumerate()
    .map(((i, cred)) => { if cred != none { [#cred] } else { none } })
    .join()

  enum.item[
    #{ if type(authors) == array { authors.enumerate().map(((i, author)) => text(author)).join(", ") } else { authors } }.
    #title.
    #credit.
    #{
      if DOI != none [DOI: #link("https://doi.org" + DOI)[#DOI]]
    }
  ]
}

#let preprint(
  authors: (),
  title: "",
  journal: "",
  published: "",
  status: none,
  DOI: none,
) = {
  let date = {
    if type(published) == datetime {
      published.display("[month repr:long] [day], [year]")
    } else {
      published
    }
  }
  enum.item[
    #{ if type(authors) == array { authors.enumerate().map(((i, author)) => text(author)).join(", ") } else { authors } }.
    #title.
    #emph[#status].
    Preprint available on #emph[#journal], #date.
    #{
      if DOI != none [DOI: #link("https://doi.org/" + DOI)[#DOI]]
    }
  ]
}

#let pres(
  authors: (),
  title: "",
  conference: "",
  number: "",
  pages: "",
  date: "",
  kind: "",
  location: "",
  DOI: none,
) = {
  let credit = (
    { if pages != "" [#pages, ] },
    { if kind != "" [#kind] },
    { if number != "" [ #number] },
  )
    .enumerate()
    .map(((i, cred)) => { if cred != none [#cred] })
    .join()

  enum.item[
    #{ if type(authors) == array { authors.enumerate().map(((i, author)) => text(author)).join(", ") } else { authors } }.
    #title.
    #emph[#conference],
    #location\;
    #credit.
    #{
      if DOI != none [DOI: #link("https://doi.org" + DOI)[#DOI]]
    }
  ]
}

// ===========================================================================
// extra.typ
// ===========================================================================

#let hide(should-hide, content) = {
  if not should-hide { content }
}

#let proj(
  title: "",
  advisors: (),
  institution: "",
  start: "",
  end: "",
  time: "",
  access: [],
  significance: [],
  skills: [],
) = {
  pagebreak()
  heading(title)
  grid(columns: (1fr, auto))
  strong[Access.]
  [#access]
  strong[Significance.]
  [#significance]
  strong[Skills.]
  [#skills]
}
{% endraw %}
