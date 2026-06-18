# Semantic Website Menus Design

## Goal

Replace the website top-navigation model based on `[navbar]` and
`position = "left" | "center" | "right"` with semantic named menus. The site
configuration should describe what each navigation group means, while bundled
and custom themes decide where those groups are rendered.

Backward compatibility is not required for this change.

## Background

The current website config has `[navbar]` with `[[navbar.item]]` entries. Each
item has a layout-oriented `position` field. The Rust model preserves
`left`, `center`, and `right`, but the bundled themes do not give those values a
uniform meaning:

- The `calepin` theme renders `left`, `center`, and `right` as separate groups.
- The `academic` theme flattens all navbar entries into one compact menu, so the
  values only affect ordering.

The current docs also describe sidebar item icons with an `icon` field, but the
sidebar config struct only accepts `target` and `glob`. The real icon mechanism
available to navigation labels is the `{icon:...}` token expanded into
`label_html`.

## New Config Model

Use `[menus]` with named arrays:

```toml
[menus]

[[menus.main]]
target = "about.typ"
label = "About"

[[menus.main]]
target = "blog.typ"
label = "Blog"

[[menus.social]]
target = "https://github.com/user/repo"
label = "{icon:github} GitHub"

[[menus.footer]]
target = "privacy.typ"
label = "Privacy"
```

Each named menu is a semantic group. Calepin will define these conventional
names:

- `main`: primary site navigation.
- `social`: social, repository, profile, and external icon-style links.
- `footer`: footer navigation.

Other menu names are allowed and exposed to custom themes. Bundled themes may
ignore unknown menu names.

Menu items support:

- `target`: a single internal `.typ` page, external URL, or already-rendered URL.
- `glob`: a list of internal `.typ` pages.
- `label`: optional rich label. For internal pages, omitted labels come from page
  metadata, then document title, then filename stem.
- `weight`: optional integer used for ordering.

Ordering rules:

1. Items are sorted by `weight` when any item in the same menu has a weight.
2. Items with no weight keep their config order after weighted items.
3. Glob-expanded items keep deterministic path order within their insertion
   point unless each expanded page later gains a page-level menu weight feature.

The first implementation does not add dropdowns, nested menu entries, or
front-matter menu membership. Those can be added later without changing the
basic named-menu model.

## Sidebar Boundary

Keep `[sidebar]` separate from `[menus]`.

The sidebar is structured documentation navigation, not a generic menu
placement. It has section titles, generated page lists, active-section behavior,
folding, and a stricter internal-page-only contract. Themes can still decide how
to render it, but the config object should remain semantically dedicated to the
content tree.

The sidebar item config should remain:

```toml
[sidebar]
fold = false

[[sidebar.section]]
title = "Guide"

  [[sidebar.section.item]]
  target = "install.typ"

  [[sidebar.section.item]]
  glob = "guide/*.typ"
```

Sidebar labels continue to come from page metadata. Sidebar icon docs should be
removed unless an actual sidebar icon field is implemented separately.

## Icon Tokens

Keep the existing `{icon:...}` rich-label syntax and extend it to local SVG
paths. Do not introduce a separate `icon` field for menus in this design.

Examples:

```toml
label = "{icon:github} GitHub"
label = "GitHub {icon:external-link}"
label = "{icon:simple-icons:googlescholar}"
label = "{icon:assets/icons/project.svg} Project"
label = "{icon:./assets/icons/project.svg} Project"
```

Resolution rules:

- Specs ending in `.svg` or containing a path separator (`/`) are local SVG
  paths.
- Local SVG paths resolve relative to the website source directory.
- Local SVG paths must stay inside the source directory after normalization.
- Non-path specs resolve through Iconify:
  - `github` means `lucide:github`.
  - `simple-icons:github` means Iconify collection `simple-icons`, icon
    `github`.
- Downloaded Iconify SVGs remain cached under `.calepin/icons`.

Sanitization rules:

- Both local and downloaded SVGs must pass the sanitizer before being inlined.
- Rename or generalize the sanitizer comments so they no longer claim it only
  handles trusted Iconify SVGs.
- Continue rejecting SVGs with scripts, `foreignObject`, event-handler
  attributes, and `javascript:` URLs.

Accessibility rules:

- Template entries continue to expose:
  - `label`: plain accessible label with icon tokens stripped.
  - `label_html`: escaped label text plus inline icon markup.
  - `href`
  - `active`
- If a label contains only icons, derive the accessible label from the target or
  icon spec as a fallback. The docs should recommend including text in the label
  when possible, even if a theme visually hides it.

## Theme Context

Replace the three navbar-specific context fields:

- `site.navbar_left`
- `site.navbar_center`
- `site.navbar_right`

with a menu map:

- `site.menus.main`
- `site.menus.social`
- `site.menus.footer`
- `site.menus.<custom-name>`

If the template engine cannot conveniently address dynamic map keys with dot
syntax, expose both:

- `site.menus`: map from menu name to entries.
- `site.menu_list`: ordered list of `{ name, items }` for iteration.

Navigation entry shape remains `href`, `label`, `label_html`, and `active`.

Bundled theme behavior:

- `calepin` theme renders `site.menus.main` near the brand and renders
  `site.menus.social` near built-in controls such as theme, language, search,
  and output mode.
- `academic` theme renders `site.menus.main` and `site.menus.social` in its
  compact header menu. It may visually distinguish social/icon-only links, but
  it is not required to mimic the `calepin` layout.
- `footer` rendering is optional in the first implementation. If implemented,
  bundled themes render `site.menus.footer` in the page footer.

## Documentation Updates

Update `docs/websites/config.typ`:

- Rename the "Top bar" section to "Menus" or "Site menus".
- Replace `[navbar]` examples with `[menus]` examples.
- Explain that menu names are semantic and themes decide placement.
- Document `menus.main`, `menus.social`, and `menus.footer`.
- Document `target`, `glob`, `label`, and `weight`.
- Remove `position = "left" | "center" | "right"` from docs.
- Remove the sidebar `icon = "lucide:download"` section unless sidebar icon
  support is implemented in code.
- Add local icon examples for `{icon:assets/icons/project.svg}`.
- Explain that local icon paths are source-relative and must stay inside the
  website source directory.

Update `docs/themes.typ`:

- Replace `site.navbar_left`, `site.navbar_center`, and `site.navbar_right` with
  `site.menus` and, if implemented, `site.menu_list`.
- Update the minimal theme example to render `site.menus.main` and
  `site.menus.social`.
- Keep documenting `href`, `label`, `label_html`, and `active`.
- Mention that custom themes can choose where named menus appear.

Update website scaffold configs:

- Replace `[navbar]` with `[menus]`.
- Move page links to `[[menus.main]]`.
- Move GitHub, Scholar, and similar icon links to `[[menus.social]]`.
- Keep current label token usage, for example `label = "{icon:github}"`.

## Implementation Notes

The current `NavItemPlan`, `NavItemModel`, and label-token expansion can be
reused. The main structural change is replacing `NavbarPlan` and `NavbarModel`
with a map keyed by menu name.

Recommended internal types:

```rust
struct MenusConfig {
    menus: BTreeMap<String, Vec<MenuItemConfig>>,
}

struct MenuPlan {
    items: Vec<NavItemPlan>,
}

struct MenuModel {
    items: Vec<NavItemModel>,
}
```

Use `BTreeMap` or another deterministic structure so generated output is stable.
Validate menu names with a simple identifier grammar such as lowercase letters,
digits, hyphens, and underscores. Reject empty names.

## Testing

Add behavior-focused tests for:

- Parsing `[menus]` with `main`, `social`, custom menu names, `target`, `glob`,
  `label`, and `weight`.
- Rejecting invalid menu names.
- Resolving internal, external, and glob menu entries.
- Sorting by `weight` while preserving config order for unweighted entries.
- Exposing relative menu links in theme context.
- Rendering bundled themes with `menus.main` and `menus.social`.
- Resolving `{icon:assets/icons/local.svg}` from the source directory.
- Rejecting local icons outside the source directory.
- Rejecting unsafe local SVG content.
- Updating stale sidebar icon docs or implementing sidebar icons if that choice
  changes before implementation.

## Non-Goals

- Backward compatibility with `[navbar]`.
- Dropdowns or nested menus.
- Page front-matter menu membership.
- A separate `icon` field for menu entries.
- Exact visual parity between bundled themes.
