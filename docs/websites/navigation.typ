#set document(title: [Website navigation])
#metadata((title: "Navigation")) <website-metadata>

#title()

= Side bar

The sidebar is the main navigation for documentation-style sites. Configure it with `[sidebar]`; a section can list pages one by one:

```toml
[sidebar]

[[sidebar.section]]
title = "Guide"

  [[sidebar.section.item]]
  target = "install.typ"
```

or include several pages with a glob:

```toml
[[sidebar.section.item]]
glob = "guide/*.typ"
```

Use `target` for one source page and `glob` for a list of source pages. Sidebar entries always point to Typst source files, not rendered `.html` files. _Calepin_ resolves those pages and writes the right `.html` links in the generated site.

The sidebar label comes from the page source, not from `calepin.toml`. Put the label in the page's website metadata:

```typ
#set document(title: [Install])
#metadata((title: "Install")) <website-metadata>

#title()
```

If a page has no `website-metadata.title`, _Calepin_ falls back to the document title, then the filename stem. This keeps multilingual sidebars in one place: each translated page carries its own translated title.

If you do not configure a sidebar, _Calepin_ builds one from `.typ` files in the source directory. Hidden files are skipped.

Titled sections are foldable: each page loads with the section that contains it open and the others folded, and readers can open more sections by hand. To keep every section expanded instead, disable folding:

```toml
[sidebar]
fold = false
```

= Site menus

Use `[menus]` for named navigation groups. Menu names describe what the links
mean; themes decide where to render them. The bundled themes understand
`main`, `social`, and `footer`. Custom themes can use any additional menu name.

```toml
[[menus.main]]
target = "index.typ"
label = "Home"
weight = 10

[[menus.social]]
target = "https://github.com/user/repo"
label = "{icon:github} GitHub"
```

Menu items use `target` or `glob`. Use a `.typ` `target` or `glob` for internal
source pages; use any other `target` for external links or a literal
already-rendered URL. Omit `label` for internal pages to use the page metadata
title, document title, or filename stem.

Use `weight` to control ordering within one menu. Lower weights appear first.
Items without weights keep their config order after weighted items.

Labels can include Iconify icons with `{icon:...}`. If the prefix is omitted,
_Calepin_ uses `lucide`, so `{icon:github}` means `{icon:lucide:github}`.
Icon prefixes are Iconify collection names. Search available icons in the
#link("https://icon-sets.iconify.design/")[Iconify icon sets] browser.

Local SVG icons are also supported with source-relative paths:

```toml
[[menus.social]]
target = "https://example.com/project"
label = "{icon:assets/icons/project.svg} Project"
```

Local icon paths must stay inside the website source directory. _Calepin_
sanitizes local and downloaded SVGs before inlining them.
