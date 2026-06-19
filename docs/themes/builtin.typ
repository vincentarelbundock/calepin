#set document(title: [Built-in themes])
#metadata((title: "Built-in")) <website-metadata>

#title()

_Calepin_ ships with two built-in themes. Built-in themes are compiled into the
binary, so they are always available by name and can be selected without adding
theme files to your project.

= `calepin`

`calepin` is the default documentation theme. It is designed for project
documentation, manuals, notebook collections, and sites where navigation and
reference lookup matter.

It includes sidebar navigation, a top bar, previous and next page links, an
on-page table of contents, dark mode, copy buttons on code blocks, and rendered,
source, and PDF view switching.

#image("/assets/screenshot_website.png", width: 100%, alt: "Screenshot of the Calepin documentation theme")

= `academic`

`academic` is a reading-first essay and blog theme. It is designed for articles,
research notes, project blogs, and smaller websites that prioritize long-form
reading over dense navigation.

It includes a centered narrow text column, margin-note support, top navigation,
dark mode, copy buttons on code blocks, and the shared Calepin search and
language controls.

#image("/assets/screenshot_academic.png", width: 100%, alt: "Screenshot of the Calepin academic theme")

= Selecting a built-in theme

Set `theme` in `calepin.toml`:

```toml
theme = "calepin"
theme = "academic"
```

Or select one for a single compile:

```sh
calepin compile paper.typ --theme academic
```

Website scaffolds are also theme-aware:

```sh
calepin new website my-site --theme academic
```
