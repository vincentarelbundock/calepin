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

= Accessible PDFs (PDF/UA)

Typst can produce tagged, accessible PDFs. Arguments after `--` are forwarded to the `typst` binary, so:

```sh
calepin compile paper.typ -- --pdf-standard ua-1
```

Under `ua-1`, Typst requires a document title and non-empty alt text on every image, and reports a diagnostic for each violation. In practice:

- Set a title with `#set document(title: [...])`.
- Give every figure-producing chunk a `fig-alt-text` option.

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
