#import "/src/lib.typ" as gloat

// should pass without parameters
#gloat.paper()

// should work with one author
#gloat.paper(
  published: datetime(year: 2025, month: 1, day: 1),
  title: "A very impressive paper",
  authors: "Single Author",
  vol: "12",
  issue: "2",
  pages: "355-372",
  journal: "Prestigious Journal",
)

#gloat.paper(
  authors: (
    [Firstname Lastname],
    [Corresponding Author],
  ),
  published: datetime(year: 2025, month: 1, day: 1),
  title: "A very impressive paper",
  vol: "12",
  issue: "2",
  pages: "355-372",
  journal: "Prestigious Journal",
)

#gloat.preprint(
  authors: (
    [Firstname Lastname],
    [Corresponding Author],
  ),
  published: datetime(year: 2025, month: 1, day: 1),
  title: "A very impressive paper",
  journal: "Prestigious Journal",
)

#gloat.abstract(
  authors: (
    [Firstname Lastname],
    [Corresponding Author],
  ),
  date: datetime(year: 2025, month: 1, day: 1),
  title: "A very impressive paper",
  conference: "Prestigious Journal",
)
