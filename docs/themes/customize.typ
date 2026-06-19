#set document(title: [Customize])

#title()

For CSS-only changes, keep a built-in theme and append project CSS with
`styles` in `calepin.toml`:

```toml
theme = "academic"
styles = [
  "styles/site.css",
  "styles/figures.css",
]
```

Configured CSS files load after the selected theme's CSS, in the order listed.
Paths are resolved relative to `calepin.toml`, and each file must have a `.css`
extension. The setting affects HTML output only.

= Theme tokens

Built-in HTML themes expose stable CSS custom properties in the `--calepin-*`
namespace. Override these tokens from project CSS instead of relying on theme
internals.

#table(
  columns: (1.5fr, 1.5fr, 3.7fr),
  stroke: none,
  inset: 0.55em,
  [*Group*], [*Token*], [*Purpose*],
  "Colors", code(`--calepin-color-background`), "Page background in normal UI surfaces.",
  "Colors", code(`--calepin-color-text`), "Primary body text color.",
  "Colors", code(`--calepin-color-muted`), "Muted/de-emphasized text.",
  "Colors", code(`--calepin-color-border`), "UI border color.",
  "Colors", code(`--calepin-color-accent`), "Primary accent color (links, buttons, highlights).",
  "Colors", code(`--calepin-color-accent-hover`), "Accent color for hover states.",
  "Colors", code(`--calepin-color-accent-soft`), "Soft tint derived from accent.",
  "Colors", code(`--calepin-color-accent-contrast`), "Text/icon contrast color on accent backgrounds.",
  "Colors", code(`--calepin-color-link`), "Default link color.",
  "Colors", code(`--calepin-color-link-hover`), "Link hover color.",
  "Colors", code(`--calepin-color-focus`), "Focus ring/text emphasis color.",
  "Colors", code(`--calepin-color-selection`), "Text-selection background color.",
  "Colors", code(`--calepin-color-info`), "Info state color.",
  "Colors", code(`--calepin-color-success`), "Success state color.",
  "Colors", code(`--calepin-color-warning`), "Warning state color.",
  "Colors", code(`--calepin-color-danger`), "Danger/error state color.",
  "Colors", code(`--calepin-color-important`), "Important state color.",
  "Colors", code(`--calepin-color-info-soft`), "Translucent info variant.",
  "Colors", code(`--calepin-color-success-soft`), "Translucent success variant.",
  "Colors", code(`--calepin-color-warning-soft`), "Translucent warning variant.",
  "Colors", code(`--calepin-color-danger-soft`), "Translucent danger variant.",
  "Colors", code(`--calepin-color-important-soft`), "Translucent important variant.",
  "Callouts", code(`--calepin-callout-note-color`), "Callout tone for note blocks.",
  "Callouts", code(`--calepin-callout-tip-color`), "Callout tone for tip blocks.",
  "Callouts", code(`--calepin-callout-warning-color`), "Callout tone for warning blocks.",
  "Callouts", code(`--calepin-callout-caution-color`), "Callout tone for caution blocks.",
  "Callouts", code(`--calepin-callout-important-color`), "Callout tone for important blocks.",
  "Surfaces", code(`--calepin-surface`), "Primary surface color for cards/panels.",
  "Surfaces", code(`--calepin-surface-muted`), "Muted surface tone.",
  "Surfaces", code(`--calepin-surface-raised`), "Raised surface tone.",
  "Surfaces", code(`--calepin-surface-inset`), "Inset surface tone.",
  "Surfaces", code(`--calepin-surface-code`), "Code block background.",
  "Surfaces", code(`--calepin-surface-output`), "Code/output container background.",
  "Typography", code(`--calepin-font-body`), "Primary text font stack.",
  "Typography", code(`--calepin-font-heading`), "Heading font stack.",
  "Typography", code(`--calepin-font-mono`), "Monospace font stack.",
  "Typography", code(`--calepin-font-size`), "Base font size multiplier.",
  "Typography", code(`--calepin-font-size-sm`), "Small text font size.",
  "Typography", code(`--calepin-font-size-aside`), "Aside/sidenote font size.",
  "Typography", code(`--calepin-line-height`), "Default line height for body text.",
  "Typography", code(`--calepin-line-height-tight`), "Tighter line height for headings.",
  "Typography", code(`--calepin-heading-weight`), "Default heading weight.",
  "Typography", code(`--calepin-code-font-size`), "Code and preformatted font size.",
  "Spacing", code(`--calepin-space-xs`), "Extra-small spacing token.",
  "Spacing", code(`--calepin-space-sm`), "Small spacing token.",
  "Spacing", code(`--calepin-space-md`), "Medium spacing token.",
  "Spacing", code(`--calepin-space-lg`), "Large spacing token.",
  "Spacing", code(`--calepin-space-xl`), "Extra-large spacing token.",
  "Spacing", code(`--calepin-space-2xl`), "2XL spacing token.",
  "Spacing", code(`--calepin-block-gap`), "Vertical block spacing.",
  "Spacing", code(`--calepin-inline-gap`), "Horizontal inline spacing.",
  "Spacing", code(`--calepin-section-gap`), "Vertical section spacing.",
  "Layout", code(`--calepin-content-width`), "Max width for main content regions.",
  "Layout", code(`--calepin-wide-width`), "Max width for wide content regions.",
  "Layout", code(`--calepin-page-width`), "Computed page width for two-column layout.",
  "Layout", code(`--calepin-margin-width`), "Sidenote/margin note width.",
  "Layout", code(`--calepin-margin-gap`), "Gap between body and margin region.",
  "Layout", code(`--calepin-page-padding-inline`), "Inline page padding.",
  "Layout", code(`--calepin-topbar-height`), "Topbar height token.",
  "Layout", code(`--calepin-sidebar-width`), "Desktop sidebar width.",
  "Layout", code(`--calepin-sidebar-mobile-width`), "Mobile drawer width.",
  "Layout", code(`--calepin-shell-gap`), "Gap between shell grid tracks.",
  "Layout", code(`--calepin-toc-indent`), "Indent used by generated TOC trees.",
  "Borders", code(`--calepin-border-width`), "Border width used across theme.",
  "Borders", code(`--calepin-border`), "Shorthand border style using `--calepin-color-border`.",
  "Borders", code(`--calepin-radius-sm`), "Small corner radius.",
  "Borders", code(`--calepin-radius-md`), "Medium corner radius.",
  "Borders", code(`--calepin-radius-lg`), "Large corner radius.",
  "Shadows", code(`--calepin-shadow-card`), "Default card shadow.",
  "Shadows", code(`--calepin-focus-ring`), "Focus ring style.",
  "Layers", code(`--calepin-z-topbar`), "Z-index for top bar.",
  "Layers", code(`--calepin-z-backdrop`), "Backdrop and overlays.",
  "Layers", code(`--calepin-z-drawer`), "Side drawer z-index.",
  "Layers", code(`--calepin-z-popover`), "Popover/dialog z-index.",
  "Layers", code(`--calepin-z-modal`), "Modal z-index.",
  "Syntax", code(`--calepin-syntax-foreground`), "Syntax text color for code blocks.",
  "Syntax", code(`--calepin-syntax-background`), "Syntax background color for code blocks.",
  "Syntax", code(`--calepin-syntax-border`), "Syntax border color derived from foreground/background.",
)

= Raw HTML plus CSS

Set `theme = "typst"` when you do not want a bundled base theme. Configured
styles still apply to HTML output:

```toml
theme = "typst"
styles = ["styles/raw.css"]
```

This mode is useful when you want Typst's raw HTML structure and a completely
project-owned stylesheet.
