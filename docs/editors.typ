#set document(title: [Editor integration])
#metadata((tags: ("getting started", "editors", "VS Code", "Tinymist"))) <website-metadata>

#title()

Calepin documents are ordinary Typst source files, so they work with any editor that supports Typst. The editor can provide language tooling and preview, while Calepin remains responsible for executing computational chunks.

= VS Code, Cursor, and Positron

Install _Calepin for Typst_ from the VS Code Marketplace. Cursor, Positron, and other VSX-compatible editors can install the same extension from Open VSX. It provides these command-palette actions:

- `Calepin Typst New`
- `Calepin Typst Compile`
- `Calepin Typst Watch`
- `Calepin Typst Stop`

The extension uses its bundled Calepin binary when available, then falls back to `calepin` on `PATH`. Set `calepin.binaryPath` to select another executable. The watch command runs Calepin continuously, with integrated preview for PDF and HTML; use it when executable chunks should rerun as the notebook changes.

= Tinymist preview

Tinymist can preview the authored `.typ` file after one successful Calepin compile, with no notebook-specific `typstExtraArgs`:

```sh
calepin compile paper.typ
```

Use the canonical facade and document adapter:

````typ
#import "/.calepin/calepin.typ" as calepin
#show: calepin.document
````

Tinymist refreshes prose, layout, and other Typst changes, but it does not execute computational chunks. After changing executable code, run Calepin again or use `Calepin Typst Watch`. Until then, the preview shows the last stored computational snapshot.

The generic facade follows the most recently compiled or watched single notebook when Calepin is not supplying its internal Typst inputs. For independent simultaneous previews, import the generated notebook-specific facade instead:

````typ
#import "/.calepin/paper/calepin.typ" as calepin
#show: calepin.document
````

Existing aliased imports and Calepin-managed workflows remain compatible. Do not use `#import "/.calepin/calepin.typ": *`: the exported `document` adapter shadows Typst's built-in `document` element, including `#set document(...)`.

= Other editors

The same compile-once contract works with other Typst language servers, preview tools, and plain `typst compile`, provided they use the same project root as Calepin. Run `calepin compile` to refresh stored results, or run `calepin watch` for automatic execution and rendering. No editor-specific Calepin settings or lock file are required.
