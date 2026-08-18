#set document(title: [Accessibility])
#metadata((
  tags: ("accessibility", "notebooks", "websites"),
  summary: "Making Calepin output accessible: alt text for generated figures, accessible PDFs with Typst's PDF/UA support, and accessible HTML navigation.",
)) <website-metadata>

#title()

= Alt text for figures

Every figure a chunk generates can carry alternative text for screen readers. Set it per chunk with the `fig-alt-text` display option:

````typ
```r
#| fig-alt-text: Fuel efficiency versus horsepower by transmission type
plot(mpg ~ hp, data = mtcars)
```
````

A document-wide default can be set in `calepin.setup(fig-alt-text: ...)`, but a shared default is rarely good alt text: prefer a specific description on each chunk.

In HTML output, the alt text becomes the `alt` attribute of the generated `<img>` element. In paged output, it is passed to Typst's `image(alt: ...)`, which embeds it in the tagged PDF structure.

Alt text follows the figure when its output is relocated with `#calepin.results(...)`; see #link("notebooks/code_execution.typ")[Code execution].

A `tbl-` chunk wraps its printed output in a table figure. That figure holds no image, so Typst cannot describe it on its own; `fig-alt-text` supplies its description too:

````typ
```r
#| label: tbl-summary
#| tbl-caption: First rows of `mtcars`
#| fig-alt-text: Table of the first rows of the mtcars data set
knitr::kable(head(mtcars))
```
````

= Alt text for hand-written elements

Chunk output is only part of a document. Typst elements you write yourself carry their own alt text:

- `#image("chart.png", alt: "...")` for a static image. `calepin health` reports images with missing or empty alt text as warnings.
- `#math.equation(alt: "...", block: false, $pi r^2$)` for math. Under `ua-1`, every equation needs one, inline equations included.
- `#figure(..., alt: "...")` when a figure's body is a drawing or generated content rather than an image.
- `#calepin.elements.sidefigure(alt: "...")[...]` and `#calepin.elements.lightbox-video(..., alt: "...")` for the margin and media elements.

= Accessible PDFs (PDF/UA)

Typst can produce tagged, accessible PDFs. Arguments after `--` are forwarded to the `typst` binary, so:

```sh
calepin compile paper.typ -- --pdf-standard ua-1
```

Under `ua-1`, Typst requires a document title and non-empty alt text on every image and equation, and reports a diagnostic for each violation. In practice:

- Set a title with `#set document(title: [...])`.
- Start the document with a level-1 heading.
- Give every figure-producing chunk a `fig-alt-text` option.
- Wrap every equation, inline ones too, in `#math.equation(alt: ...)`.
- Add `alt:` to hand-written images and to figures whose body is not an image.

A third-party Typst package that emits images or figures without alt text will fail a `ua-1` build from inside the package, and no option on your side can fix it.

= Accessible websites

Calepin's HTML output uses semantic elements for navigation, headings, and figures. A few things remain yours to provide:

- Icon-only links need labels. In `calepin.toml`, menu items whose label is an icon accept an `aria-label`:

  ```toml
  [[menus.social]]
  target = "https://github.com/example/repo"
  label = "{icon:github}"
  aria-label = "GitHub"
  ```

- The site logo's alternative text comes from the `logo-alt` key in `calepin.toml`.
- Write descriptive link text and meaningful figure captions; captions complement alt text, they do not replace it.
