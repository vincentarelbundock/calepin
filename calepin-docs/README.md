# calepin-docs

Generate Typst API reference pages from Python source.

Signatures come from **static analysis** ([`ruff_python_parser`]) and docstrings
from [`pydocstring`]. Nothing is imported or executed, so the documented package
needs no install and no working environment — and annotations render exactly as
the author wrote them (`int | None`, not `typing.Optional[int]`).

## Usage

```bash
make api-reference PACKAGE=path/to/package OUT=docs-src/reference/api
```

Or directly:

```bash
cargo run -p calepin-docs -- path/to/package --out reference [--website] [--dry-run]
```

`PACKAGE` is a directory containing `__init__.py`, or a single `.py` file.
`--website` adds Calepin `<website-metadata>` so the pages join a site's
navigation and search index; without it each page is a standalone Typst
document.

## Output

One `.typ` per exported definition, named by dotted qualname
(`pkg.module.Thing.typ`), plus:

- `index.typ` — every definition, linked, with its summary
- `api.typ` — the styling template

Pages call template functions rather than emitting styled markup directly, so
the reference can be restyled without regenerating it. `api.typ` is written only
when absent: **regeneration never overwrites your styling**.

## What is documented

The public surface is `__all__` when the package declares one, in its declared
order — including a computed one such as `__all__ = list(_EXPORTS.keys())`.
Names are traced to the module that actually defines them, through re-export
chains (`from .core.impl import Thing`) and through PEP 562 lazy-export tables
mapping each name to `(module, attribute)`. Without `__all__`, every public
top-level definition is documented.

Anything in `__all__` that cannot be traced is **reported on stderr**, never
silently dropped.

Classes carry their public methods plus `__init__`; other dunders and
underscore-prefixed names are skipped. The implicit `self` / `cls` receiver is
dropped from method signatures.

## Docstrings that are not literal body strings

Runtime introspection sees `__doc__` after the interpreter has assembled it.
Static analysis has to recognise how it was assembled, and three idioms are
handled because they are common enough to matter:

- **Decorator docstrings** — `@doc("...")` and friends, where the decorator sets
  `func.__doc__`. Any decorator call whose first argument is a string literal
  counts; it is then not also listed as a decorator on the page.
- **Docstring aliases** — `wrapper.__doc__ = original.__doc__`, resolved to the
  original definition's text.
- **Shared fragments** — `{placeholder}` slots filled by `str.format` from
  module-level string constants. Unknown placeholders are left intact, so a
  docstring showing a dict literal survives.

## Markdown docstrings

Docstrings that are neither Google nor NumPy style come back from `pydocstring`
as unstructured prose. Many projects write Markdown there, so headings, bullet
lists (nested), fenced code, and paragraphs render as Typst constructs rather
than being escaped into literal punctuation. A leading `# name()` heading is
dropped, since the page already prints its own heading.

## Limits

Static analysis sees the source, not the runtime object:

- **Inherited members are not resolved.** A method defined on a base class does
  not appear on the subclass's page.
- **Dynamically generated attributes are invisible** — anything built by
  `setattr` or a metaclass, or a docstring assembled by a mechanism other than
  the three above.
- **`.pyi` stubs are ignored**; only the `.py` source is read.
- **Absolute imports leaving the package are not followed.** A name re-exported
  from a third-party package resolves as unresolved.

These are the cases where runtime introspection (`inspect`) wins. Everything
else — verbatim annotations, no import side effects, no environment to build —
is where static analysis wins.

## Known upstream gap

`pydocstring` 0.4 lifts `.. deprecated::` directives out of Google and NumPy
docstrings, but not out of `Style::Plain` ones, where the directive stays in the
extended summary. `emit::recover_plain_directive` works around this for
deprecations; remove it once upstream handles plain documents.

[`ruff_python_parser`]: https://crates.io/crates/ruff_python_parser
[`pydocstring`]: https://crates.io/crates/pydocstring
