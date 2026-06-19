#set document(title: [Themes])
#import "/.calepin/calepin.typ" as calepin

#metadata((title: "Overview")) <website-metadata>

#title()

Themes control how _Calepin_ renders two types of content: websites and notebooks (HTML or PDF). A website theme can provide MiniJinja HTML templates, partials, CSS, JavaScript, and notebook themes add typst-side template support for HTML and PDF outputs. The default theme is called `calepin`. 

#calepin.elements.gallery(
  (
    ("/themes/screenshots/calepin_website_dark.png", "Calepin website theme in dark mode", [Calepin website theme in dark mode]),
    ("/themes/screenshots/calepin_website_light.png", "Calepin website theme in light mode", [Calepin website theme in light mode]),
    ("/themes/screenshots/academic_website_dark.png", "Academic website theme in dark mode", [Academic website theme in dark mode]),
    ("/themes/screenshots/academic_website_light.png", "Academic website theme in light mode", [Academic website theme in light mode]),
    ("/themes/screenshots/tufte_notebook_dark.png", "Tufte notebook theme in dark mode", [Tufte notebook theme in dark mode]),
    ("/themes/screenshots/tufte_notebook_light.png", "Tufte notebook theme in light mode", [Tufte notebook theme in light mode]),
    ("/themes/screenshots/tufte_notebook_pdf_01.png", "Tufte notebook PDF page 1", [Tufte notebook PDF page 1]),
    ("/themes/screenshots/tufte_notebook_pdf_02.png", "Tufte notebook PDF page 2", [Tufte notebook PDF page 2]),
  ),
  columns: 2,
  max-width: 42em,
)

= Choosing a theme

Select a different built-in or local theme with `theme` in a website's `calepin.toml`:

```toml
theme = "calepin"           # the default documentation theme
theme = "academic"          # a built-in essay/blog theme
theme = "themes/my-theme"   # a local theme directory
theme = "typst"             # raw Typst output, no Calepin theme
```

If the `calepin.toml` file is not located in the same directory as the document or website being compiled, you can specify the path to the config file with `--config`:

```sh
calepin compile notebook.typ --config=/path/to/calepin.toml
```

You can also set the theme in-document with:

```typ
#calepin.setup(theme: "academic")
```

When several theme settings are present during a compile, the document setting wins, then `calepin.toml`, then the default theme.

= Structure

A theme can provide templates for website pages, single-document HTML, and Typst-level notebook rendering:

```text
themes/my-theme/
  theme.toml         # theme metadata
  layouts/
    webpage.html     # layout for website pages
    notebook.html    # layout for a single notebook rendered to HTML
    landing.html     # optional page-specific website layout
  partials/          # MiniJinja fragments
  styles/            # CSS files
  scripts/           # JavaScript files
  notebook.typ.jinja # Typst notebook template for PDF (and SVG)
```

The files in `layouts/` and `partials/` use the #link("https://docs.rs/minijinja/latest/minijinja/")[MiniJinja template language].
