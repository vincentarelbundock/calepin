#import ".calepin/calepin.typ"
#calepin.setup(
  echo: true,
  eval: true,
)

#set document(
  title: [#emph[Calepin]: Computational notebooks in Typst],
)
#title()

= Nonsense Part I

#lorem(50)

```python
print(40 + 2)
```

#pagebreak()

= Nonsense Part II

#lorem(100)

```python
from plotnine import ggplot, aes, geom_point, labs
from plotnine.data import mtcars

(
    ggplot(mtcars, aes("mpg", "hp"))
    + geom_point(color = "blue", size = 10)
    + labs(x="Miles per gallon", y="Horsepower")
).show()
```