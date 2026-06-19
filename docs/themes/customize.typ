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

= Style raw HTML with CSS

This setting is for teams that want complete control of the generated markup and CSS while still
keeping Calepin's notebook execution model. It is especially useful for design systems with
established HTML/CSS conventions.

Set `theme = "typst"` when you do not want a bundled base theme. Configured styles still apply to HTML output:

```toml
theme = "typst"
styles = ["styles/raw.css"]
```

= Theme tokens

Built-in HTML themes expose stable CSS custom properties in the `--calepin-*`
namespace. Override these tokens from project CSS instead of relying on theme
internals.

#table(
  columns: (1.5fr, 1.5fr, 3.7fr),
  stroke: none,
  inset: 0.55em,
  [*Group*], [*Token*], [*Purpose*],
  "Colors", `--calepin-color-background`, "Page background in normal UI surfaces.",
  "Colors", `--calepin-color-text`, "Primary body text color.",
  "Colors", `--calepin-color-muted`, "Muted/de-emphasized text.",
  "Colors", `--calepin-color-border`, "UI border color.",
  "Colors", `--calepin-color-accent`, "Primary accent color (links, buttons, highlights).",
  "Colors", `--calepin-color-accent-hover`, "Accent color for hover states.",
  "Colors", `--calepin-color-accent-soft`, "Soft tint derived from accent.",
  "Colors", `--calepin-color-accent-contrast`, "Text/icon contrast color on accent backgrounds.",
  "Colors", `--calepin-color-link`, "Default link color.",
  "Colors", `--calepin-color-link-hover`, "Link hover color.",
  "Colors", `--calepin-color-focus`, "Focus ring/text emphasis color.",
  "Colors", `--calepin-color-selection`, "Text-selection background color.",
  "Colors", `--calepin-color-info`, "Info state color.",
  "Colors", `--calepin-color-success`, "Success state color.",
  "Colors", `--calepin-color-warning`, "Warning state color.",
  "Colors", `--calepin-color-danger`, "Danger/error state color.",
  "Colors", `--calepin-color-important`, "Important state color.",
  "Colors", `--calepin-color-info-soft`, "Translucent info variant.",
  "Colors", `--calepin-color-success-soft`, "Translucent success variant.",
  "Colors", `--calepin-color-warning-soft`, "Translucent warning variant.",
  "Colors", `--calepin-color-danger-soft`, "Translucent danger variant.",
  "Colors", `--calepin-color-important-soft`, "Translucent important variant.",
  "Callouts", `--calepin-callout-note-color`, "Callout tone for note blocks.",
  "Callouts", `--calepin-callout-tip-color`, "Callout tone for tip blocks.",
  "Callouts", `--calepin-callout-warning-color`, "Callout tone for warning blocks.",
  "Callouts", `--calepin-callout-caution-color`, "Callout tone for caution blocks.",
  "Callouts", `--calepin-callout-important-color`, "Callout tone for important blocks.",
  "Surfaces", `--calepin-surface`, "Primary surface color for cards/panels.",
  "Surfaces", `--calepin-surface-muted`, "Muted surface tone.",
  "Surfaces", `--calepin-surface-raised`, "Raised surface tone.",
  "Surfaces", `--calepin-surface-inset`, "Inset surface tone.",
  "Surfaces", `--calepin-surface-code`, "Code block background.",
  "Surfaces", `--calepin-surface-output`, "Code/output container background.",
  "Typography", `--calepin-font-body`, "Primary text font stack.",
  "Typography", `--calepin-font-heading`, "Heading font stack.",
  "Typography", `--calepin-font-mono`, "Monospace font stack.",
  "Typography", `--calepin-font-size`, "Base font size multiplier.",
  "Typography", `--calepin-font-size-sm`, "Small text font size.",
  "Typography", `--calepin-font-size-aside`, "Aside/sidenote font size.",
  "Typography", `--calepin-line-height`, "Default line height for body text.",
  "Typography", `--calepin-line-height-tight`, "Tighter line height for headings.",
  "Typography", `--calepin-heading-weight`, "Default heading weight.",
  "Typography", `--calepin-code-font-size`, "Code and preformatted font size.",
  "Spacing", `--calepin-space-xs`, "Extra-small spacing token.",
  "Spacing", `--calepin-space-sm`, "Small spacing token.",
  "Spacing", `--calepin-space-md`, "Medium spacing token.",
  "Spacing", `--calepin-space-lg`, "Large spacing token.",
  "Spacing", `--calepin-space-xl`, "Extra-large spacing token.",
  "Spacing", `--calepin-space-2xl`, "2XL spacing token.",
  "Spacing", `--calepin-block-gap`, "Vertical block spacing.",
  "Spacing", `--calepin-inline-gap`, "Horizontal inline spacing.",
  "Spacing", `--calepin-section-gap`, "Vertical section spacing.",
  "Layout", `--calepin-content-width`, "Max width for main content regions.",
  "Layout", `--calepin-wide-width`, "Max width for wide content regions.",
  "Layout", `--calepin-page-width`, "Computed page width for two-column layout.",
  "Layout", `--calepin-margin-width`, "Sidenote/margin note width.",
  "Layout", `--calepin-margin-gap`, "Gap between body and margin region.",
  "Layout", `--calepin-page-padding-inline`, "Inline page padding.",
  "Layout", `--calepin-topbar-height`, "Topbar height token.",
  "Layout", `--calepin-sidebar-width`, "Desktop sidebar width.",
  "Layout", `--calepin-sidebar-mobile-width`, "Mobile drawer width.",
  "Layout", `--calepin-shell-gap`, "Gap between shell grid tracks.",
  "Layout", `--calepin-toc-indent`, "Indent used by generated TOC trees.",
  "Borders", `--calepin-border-width`, "Border width used across theme.",
  "Borders", `--calepin-border`, "Shorthand border style using `--calepin-color-border`.",
  "Borders", `--calepin-radius-sm`, "Small corner radius.",
  "Borders", `--calepin-radius-md`, "Medium corner radius.",
  "Borders", `--calepin-radius-lg`, "Large corner radius.",
  "Shadows", `--calepin-shadow-card`, "Default card shadow.",
  "Shadows", `--calepin-focus-ring`, "Focus ring style.",
  "Layers", `--calepin-z-topbar`, "Z-index for top bar.",
  "Layers", `--calepin-z-backdrop`, "Backdrop and overlays.",
  "Layers", `--calepin-z-drawer`, "Side drawer z-index.",
  "Layers", `--calepin-z-popover`, "Popover/dialog z-index.",
  "Layers", `--calepin-z-modal`, "Modal z-index.",
  "Syntax", `--calepin-syntax-foreground`, "Syntax text color for code blocks.",
  "Syntax", `--calepin-syntax-background`, "Syntax background color for code blocks.",
  "Syntax", `--calepin-syntax-border`, "Syntax border color derived from foreground/background.",
)
