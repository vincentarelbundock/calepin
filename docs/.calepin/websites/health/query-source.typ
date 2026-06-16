#import "/.calepin/query-html.typ" as html

#set document(title: [Health checks])
#metadata((title: "Health")) <website-metadata>

#title()

`calepin health` runs quick diagnostic checks on the current project (notebook or website) and reports anything that could break rendering or execution.

It checks the following by default:

- executable availability for configured engines and diagram tools
- Python/Jupyter kernel metadata consistency
- local literal links inside `.typ` files

```text
calepin health [OPTIONS]
```

Use `--json` for machine-readable output.

= Link checking

`health` scans `#link("...")` literals in Typst source files and verifies targets.

- A bad local target (for example `#link("missing.html")`) is reported as an **error**.
- External `http://` and `https://` links are ignored by default.
- Pass `--check-external-links` to validate HTTP(S) links too.

By default, `health` walks directories recursively from the current directory,
ignoring `.calepin`, `.git`, `target`, `node_modules`, and `.venv`.
You can limit recursion depth with `--depth`:

```text
calepin health --depth 2
```

With recursive links that may refer to generated outputs, run `calepin health` after a
build so referenced pages exist.

`--strict` turns warnings into failures (in addition to errors).

```text
calepin health --strict --check-external-links
```
