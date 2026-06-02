---
title: Templates
---

Templates are not quite supported yet, but much of the infrastructure for partials and `minijinja` templates is there. At the CLI, things could look like:

```sh
calepin compile paper.typ --format html --template html-in-md
calepin compile paper.typ --format html --template pico
calepin compile paper.typ --format html --template basic
```

`--template` is optional and only applies to HTML output. Use `html-in-md` when you want a markdown-embeddable HTML fragment instead of a full HTML document. Use `pico` and `basic` to force either built-in theme on HTML output.
