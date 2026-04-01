---
name: calepin
description: >
  Create and render documents, websites, and books with the Calepin CLI.
  Covers .qmd authoring (TOML front matter, code chunks, citations,
  cross-references, figures, tables, math), project scaffolding,
  configuration, templates, and extensions. Use when writing or
  editing .qmd files, configuring _calepin/ projects, customizing
  templates, or running calepin commands.
---

# Calepin

*Calepin* renders `.qmd` (Quarto-compatible) documents to HTML, LaTeX, Typst, and Markdown. It executes R, Python, and shell code chunks, processes citations, resolves cross-references, and applies Jinja2 templates.

## When to use what

| Task | Where to look |
|------|---------------|
| Minimal document, front matter, render command | [reference.md](references/reference.md) > "Minimal Document", "CLI Commands" |
| Code chunks, options, inline code | [reference.md](references/reference.md) > "Code Chunks", "Inline Code" |
| Figures, tables, math, theorems | [reference.md](references/reference.md) > "Figures", "Tables", "Math" |
| Citations and bibliography | [reference.md](references/reference.md) > "Citations and Bibliography" |
| Cross-references (@fig-, @tbl-, @eq-, @thm-) | [reference.md](references/reference.md) > "Cross-references" |
| Configuration and project structure | [reference.md](references/reference.md) > "Configuration", "Project Structure" |
| Templates and customization | [reference.md](references/reference.md) > "Templates" |
| Jinja variables, conditionals, includes | [reference.md](references/reference.md) > "Jinja Body Processing" |
| Websites and navigation | [reference.md](references/reference.md) > "Websites" |
| Extensions and color schemes | [reference.md](references/reference.md) > "Extensions", "Color Schemes" |

## Quick start

```toml
---
title = "My Document"
bibliography = "references.bib"
---
```

````
```{r}
summary(mtcars)
```
````

```bash
calepin render document.qmd           # HTML
calepin render document.qmd -t latex  # LaTeX
calepin preview document.qmd          # live reload
```

## Key conventions

- Front matter is TOML (not YAML) between `---` delimiters.
- Chunk options use `#| key = value` (TOML) or `#| key: value` (Quarto-compatible). Names use dashes: `fig-width`, not `fig.width`.
- Chunk labels go in the header: `{r, my-label}`, never in `#|` comments.
- Cross-reference prefixes: `@fig-`, `@tbl-`, `@eq-`, `@thm-`, `@sec-`.
- Templates are Jinja2 files in `{stem}_calepin/templates/{writer}/`. Override only what you need; unoverridden files fall through to built-in defaults.
- Template variables: `cfg.*` for user-authored values, `clp.*` for engine-computed values. Variable names use underscores.
- Color schemes control syntax highlighting. No separate `[highlight]` config; use `colors = ["nord", "ayu"]` in config.toml.
- When referring to the software by name, write *Calepin* (italic, capital C).
- Never use em or en dashes in documentation or prose.
