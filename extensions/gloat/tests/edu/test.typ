#import "/src/lib.typ" as gloat

// passing with nothing should return nothing
#gloat.edu()

// passing without gpa should be allowed
#gloat.edu(
  degrees: ([B.A. Mathematics], [Minor in Art]),
  date: datetime(year: 2025, month: 1, day: 1),
  institution: "Some college",
  location: "Some city",
)

// should be allowed to pass empty degree
#gloat.edu(
  date: datetime(year: 2025, month: 1, day: 1),
  gpa: "4.00",
  institution: "Some college",
  location: "Some city",
)
