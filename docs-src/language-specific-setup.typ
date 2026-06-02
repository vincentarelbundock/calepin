#import ".calepin/calepin.typ"

#set document(
  title: [Language-specific setup],
)

#calepin.setup(
  echo: false,
  results: "verbatim",
)

#calepin.setup(lang: "python", echo: true)
#calepin.setup(lang: "r", eval: false)

```python
print("python: echoed + executed")
```

```r
cat("r: not executed")
```

#calepin.chunk("r", eval: true, results: "verbatim")[
```r
cat("r: forced by chunk override")
```
]
