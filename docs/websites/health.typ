#set document(title: [Health checks])
#metadata((title: "Health")) <website-metadata>

#title()

`calepin health` runs quick diagnostic checks on the current project (notebook or website) and reports anything that could break rendering or execution.

It checks the following by default:

- executable availability for configured engines and diagram tools
- Python/Jupyter kernel metadata consistency
- local literal links inside `.typ` files
- local literal images inside `.typ` files
- missing alt text on literal Typst images
- duplicate explicit page routes from `slug` and `url` page metadata

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

= Image and slug checking

`health` scans literal `#image("...")` calls in Typst source files. Missing local image files are reported as **errors**. Images without non-empty `alt:` text are reported as **warnings**.

`health` also scans `<website-metadata>` for literal `slug` and `url` values and reports duplicate or unsafe output routes as **errors**. The check compares the rendered HTML route, so pages in different directories can use the same slug as long as they do not write the same output path.

Raw Typst code spans and blocks are ignored by source checks, so documentation examples do not need to point at real files.

`--strict` turns warnings into failures (in addition to errors).

```text
calepin health --strict --check-external-links
```
