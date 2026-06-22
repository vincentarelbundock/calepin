#import "/.calepin/calepin.typ" as calepin
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

Titled sections are foldable: each page loads with the section that contains it open and the others folded. Opening a different section folds the previous one. To keep every section expanded instead, disable folding:

```toml
[sidebar]
fold = false
```

= Table of contents

Pages can show an "On this page" table of contents built from their own headings (levels 1-3 by default). The `calepin` theme shows one by default; other themes, including `academic`, are opt-in.

Set a site-wide default with `[toc]`:

```toml
[toc]
enabled = true
depth = 2
```

`depth` is the maximum heading level included, from 1 to 6.

Override either field for a single page with `<website-metadata>`:

```typ
#metadata((toc: (enabled: false))) <website-metadata>
```

```typ
#metadata((toc: (depth: 2))) <website-metadata>
```

Page metadata and `calepin.toml` merge field by field: a page can override just `depth` and still inherit `enabled` from `calepin.toml`, or the reverse. Whatever is left unset falls back to the theme's own default.

= Site menus

Use `[menus]` for named navigation groups. Menu names describe what the links
mean; themes decide where to render them. The bundled themes understand
`main` and `social`. Custom themes can use any additional menu name.

```toml
[[menus.main]]
target = "index.typ"
label = "Home"
weight = 10

[[menus.social]]
target = "https://github.com/user/repo"
label = "{icon:github}"
aria-label = "GitHub"
```

Menu items use `target` or `glob`. Use a `.typ` `target` or `glob` for internal
source pages; use any other `target` for external links or a literal
already-rendered URL. Omit `label` for internal pages to use the page metadata
title, document title, or filename stem.

= Footer

Configure the site footer with `[[footer.item]]`. Footer items can be links or
plain text rows for copyright and legal notices:

```toml
[[footer.item]]
label = "© 2026 Example"

[[footer.item]]
target = "https://example.com/privacy"
label = "Privacy"
```

A footer row with only `label` is rendered as text (no hyperlink).

Use `weight` to control ordering within one menu or footer. Lower weights appear first.
Items without weights keep their config order after weighted items.

Labels can include Iconify icons with `{icon:...}`. If the prefix is omitted,
_Calepin_ uses `lucide`, so `{icon:github}` means `{icon:lucide:github}`.
Icon prefixes are Iconify collection names. Search available icons in the
#link("https://icon-sets.iconify.design/")[Iconify icon sets] browser.

For icon-only visible labels, set `aria-label` so screen readers get a human-readable name:

```toml
aria-label = "GitHub"
```

(If you omit `aria-label`, Calepin uses fallback text if available; for an icon-only label with no fallback text this will be less readable.)

Local SVG icons are also supported with source-relative paths:

```toml
[[menus.social]]
target = "https://example.com/project"
label = "{icon:assets/icons/project.svg} Project"
```

Local icon paths must stay inside the website source directory. _Calepin_
sanitizes local and downloaded SVGs before inlining them.
