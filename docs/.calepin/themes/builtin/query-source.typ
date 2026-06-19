#set document(title: [Built-in])
#import "/.calepin/calepin.typ" as calepin

#title()

_Calepin_ ships with three built-in themes. Built-in themes are compiled into the
binary, so they are always available by name and can be selected without adding
theme files to your project.

= `calepin`

`calepin` is the default documentation theme. It is designed for project
documentation, manuals, notebook collections, and sites where navigation and
reference lookup matter.

It includes sidebar navigation, a top bar, previous and next page links, an
on-page table of contents, dark mode, copy buttons on code blocks, and rendered,
source, and PDF view switching.

#calepin.elements.gallery(
  (
    ("/themes/screenshots/calepin_website_dark.png", "Calepin theme dark website", [blah]),
    ("/themes/screenshots/calepin_website_light.png", "Calepin theme light website", [blah]),
  ),
  columns: 2,
  max-width: 42em,
)

= `academic`

`academic` is a reading-first essay and blog theme. It is designed for articles,
research notes, project blogs, and smaller websites that prioritize long-form
reading over dense navigation.

It includes a centered narrow text column, margin-note support, top navigation,
dark mode, copy buttons on code blocks, and the shared Calepin search and
language controls.

#calepin.elements.gallery(
  (
    ("/themes/screenshots/academic_website_dark.png", "academic theme dark website", [blah]),
    ("/themes/screenshots/academic_website_light.png", "academic theme light website", [blah]),
  ),
  columns: 2,
  max-width: 42em,
)

= `typst`

`typst` disables the website and notebook themed wrappers and uses raw Typst
output. Use this when you want unstyled HTML or the output unchanged from the
Typst source.
