HTML templates can wrap Typst's HTML export in a built-in or user-defined
layout. At the CLI:

```sh
calepin compile paper.typ --format html --template pico
calepin compile paper.typ --format html --template basic
calepin compile paper.typ --format html --template tufte
```

`--template` is optional and only applies to HTML output. The built-in themes
are `pico`, `basic`, and `tufte`. The `tufte` theme uses a Palatino-style
serif face and reserves a right margin for `.sidenote`, `.marginnote`, and
`.margin-figure` elements. A directory under the configured `themes_dir` can
also provide a custom `layout.html` plus optional `styles/`, `scripts/`, and
`partials/`.
