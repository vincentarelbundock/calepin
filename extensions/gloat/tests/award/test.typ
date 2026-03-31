#import "/src/lib.typ" as gloat

// should work with data in datetime or string form
#gloat.award(
  name: "Award name",
  date: "2025",
  from: "Org name",
)

#gloat.award(
  name: "Award name",
  date: datetime(year: 2025, month: 1, day: 1),
  from: "Org name",
)
