# tufte/

A Tufte-style starter example migrated from the staged `tufte/` layout to
`examples/tufte` and converted to the built-in `academic` theme.

The build now uses the CSS customization workflow from `docs/themes/customize.typ`:

```toml
theme = "academic"
styles = ["visual-identity.css"]
```

Because `academic` already ships sidenote and margin-figure layout classes, the example now uses
the built-in element helpers:

- `calepin.elements.sidenote`
- `calepin.elements.sidefigure`

## Files

- `config.toml`:
  - sets `theme = "academic"` and `styles = ["visual-identity.css"]
  - keeps a local `python` executable path override
- `tufte_starter.typ`:
  - keeps setup/math helpers and chunk settings inline
  - imports `/.calepin/calepin.typ` and uses built-in `elements.sidenote`/`elements.sidefigure`
- `visual-identity.css`:
  - token-based override of `--calepin-*` variables and minimal presentational tweaks
- `tufte_starter.html` is retained as a rendered reference artifact
