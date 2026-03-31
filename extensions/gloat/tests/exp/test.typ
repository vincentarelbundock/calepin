#import "/src/lib.typ" as gloat

// should pass with strings or datetime for start/end dates
#gloat.exp(
  role: "Jobname",
  org: "Institution",
  start: datetime(year: 2022, month: 6, day: 7),
  end: "Present",
  location: [#lorem(3)],
  summary: [#lorem(6)],
  details: [
    #lorem(30)
  ],
)
