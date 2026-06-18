# Typst runtime shim transition

Date: 2026-06-18
Status: draft for review

## Problem

Calepin currently has two Typst implementations with different behavior:

- A binary-generated runtime written into `.calepin/calepin.typ`.
- The public `@preview/calepin:0.0.1` Typst Universe package.

The generated runtime is the implementation Calepin wants to use during
`calepin compile` and `calepin watch`. It knows about query/render modes,
results files, figure rendering, cross-reference labels, website elements, and
other fast-moving behavior.

The public package is currently a fallback shim. It makes direct `typst compile`
pleasant by showing a warning and leaving code blocks visible, but its
`calepin.chunk` implementation discards chunk options such as `label`,
`fig-caption`, and `results`.

That split breaks multi-file documents. Calepin rewrites the root source file's
`@preview/calepin` import to `/.calepin/calepin.typ`, but it does not rewrite
imports inside included files. An included file that imports
`@preview/calepin:0.0.1` therefore resolves the public fallback package. The
inner raw code block may still be discovered and executed, but the surrounding
`calepin.chunk(...)` options are lost, so the figure label and caption do not
survive to render.

Extending that rewrite to included files is not just a matter of searching for
more files. Calepin knows the root input path before Typst runs, so it can stage
and rewrite that one file. The include graph, however, is part of the evaluated
Typst program: includes can be root-relative or file-relative, can be selected
through conditionals, functions, loops, or `sys.inputs`, and can themselves
affect later evaluation. A Rust-side source scanner would either miss valid
Typst-generated dependencies or include files that Typst would not evaluate for
the current render. Staging arbitrary included files also changes path
semantics unless Calepin mirrors the source tree and rewrites every affected
`#include`, `#import`, `read`, image path, and related asset reference. Editing
the user's source files in place would avoid staging path problems, but that is
too invasive for a compile/watch tool. This is why recursive source rewriting is
rejected as the permanent fix.

The immediate need is a development strategy that lets Calepin ship Typst-side
behavior with the binary, without requiring frequent Typst Universe releases,
while still providing a path back to normal `@preview/calepin` imports once the
Typst-side API stabilizes.

## Background: Typst Packages, Modules, and Runtime Code

This design follows Typst's package and module model. Calepin should not create
a special ambient namespace that normal Typst does not have.

Typst packages are imported with paths such as:

```typst
#import "@preview/calepin:0.0.2" as calepin
```

The `preview` namespace is Typst Universe's public package namespace. Published
versions are effectively immutable from a Calepin development perspective:
changing behavior normally means preparing and publishing a new package version.
That is slower and more ceremony-heavy than shipping a Calepin binary.

Typst also supports local package namespaces such as:

```typst
#import "@local/calepin:0.0.2" as calepin
```

Those local packages live under Typst's package data directory, but relying on
Calepin to install global user-local packages would create state outside the
project, stale-version problems, and reproducibility issues in CI. It is useful
for manual development, but it should not be the main Calepin runtime strategy.

Typst has a `--package-path` option, but it takes one package root, not a search
path list. Using it to shadow only Calepin would require constructing a composed
package root containing both Calepin's generated package and any user-local
packages. That means symlinks, copies, or other filesystem overlay behavior
across operating systems. This spec rejects that approach.

Typst modules also do not behave like global plugins. Importing a module that
contains top-level `#show` rules does not apply those rules to the importing
document. Show rules are scoped in Typst markup. Calepin can still keep the show
rule logic in runtime `.typ` files, but the generated wrapper must call a
runtime body wrapper so the show rules apply to the notebook body:

```typst
#import "/.calepin/calepin.typ" as runtime

#runtime.notebook-body[
  #include "/main.typ"
]
```

Inside `notebook-body`, runtime-scoped show rules can transform stable markers
emitted by `calepin.chunk`, `calepin.inline`, and `calepin.results`.

The same module model is why Calepin should not auto-insert imports as the
primary design. Typst files are modules, and an import defines names in that
module's scope. Adding `#import ... as calepin` to the root file does not define
`calepin` inside files reached by `#include`. Making that work would require
rewriting each included module or creating a non-Typst ambient namespace.
Neither is a principled Typst design.

## Goals

- Make multi-file Calepin documents behave consistently.
- Let fast-moving Typst runtime behavior ship with the Calepin binary.
- Avoid package-path overlays, global local-package installation, or recursive
  source-tree rewrites as the core design.
- Provide a temporary bridge between current development imports and a future
  official protocol shim package.
- Move Typst behavior out of `staging.rs` and into runtime `.typ` files where
  possible.
- Produce clear diagnostics when Calepin cannot safely infer the intended
  runtime mode.

## Non-goals

- Fully preserve `@preview/calepin:0.0.1` multi-file behavior. Version `0.0.1`
  is a fallback warning shim and does not emit enough metadata to recover chunk
  options from included files.
- Auto-insert `#import ... as calepin` as the primary way to make the function
  API available. Root-only auto-insertion does not help included modules, and
  recursive insertion is source-tree rewriting.
- Implement package-path composition or filesystem overlays.
- Edit user source files in place.
- Support arbitrary old public package versions indefinitely.

## Chosen Design

Calepin will support two temporary Typst runtime modes that both preserve
Typst's explicit-import model:

1. **Protocol shim mode** for documents that import a supported public shim
   package, initially `@preview/calepin:0.0.2`.
2. **Local runtime mode** for development documents that import the
   binary-generated runtime directly as `/.calepin/calepin.typ`.

Calepin chooses the mode by evaluating the document and looking for a protocol
marker, not by scanning source text for an import string.

### Protocol Shim Mode

Publish a new public package version, `@preview/calepin:0.0.2`, as a small
stable protocol shim.

This package should not own complex rendering. Instead, it should expose the
public author API:

```typst
#calepin.setup(...)
#calepin.chunk(...)
#calepin.inline(...)
#calepin.results(...)
```

and emit stable labeled metadata/markers:

- `<calepin-protocol>`: declares the shim protocol version.
- `<calepin-config>`: setup metadata.
- `<calepin-chunk>`: executable chunk metadata for preprocessing.
- `<calepin-render-chunk>`: render-position marker for a chunk.
- `<calepin-render-results>`: render-position marker for relocated output.
- Existing labels such as `<calepin-fence-label>` may remain if still useful.

The generated binary runtime wraps the document body and transforms these
markers into rendered output. The public shim only needs to preserve enough
information for the binary to parse chunks and for the runtime to know where to
render results.

The protocol marker should be plain data, for example:

```typst
#metadata((
  name: "calepin",
  protocol: 1,
  package-version: "0.0.2",
)) <calepin-protocol>
```

Calepin should accept supported protocol versions, not just one package version.
That leaves room for `@preview/calepin:0.0.3` to use the same protocol without
changing the binary mode selection rule.

### Local Runtime Mode

During heavy development, authors may import the binary-generated runtime
directly:

```typst
#import "/.calepin/calepin.typ" as calepin
```

This should be the recommended development import until the public shim is
released and stable. It must be used consistently in every file that calls
Calepin, including included files.

Local runtime mode keeps using the generated `.calepin` runtime as the public
Typst API. Since every file imports that same runtime, included chunks preserve
their labels, captions, and options.

The existing root-file rewrite from `@preview/calepin` to
`/.calepin/calepin.typ` may remain temporarily as a single-file convenience, but
it should not be presented as a correct multi-file strategy. For multi-file
documents, Calepin should either detect supported protocol shim mode or require
explicit local runtime imports.

## Mode Selection

Calepin metadata collection should run in two phases.

### Phase 1: Protocol Probe

Generate the `.calepin` runtime as usual so a wrapper can import it, but do not
rewrite `@preview/calepin` imports for the probe.

Run a Typst eval query that includes at least:

```typst
query(<calepin-protocol>)
```

If the query returns a supported protocol marker, Calepin selects protocol shim
mode. It should then collect setup/chunk metadata from the shim markers and
render with the generated runtime wrapper.

If the query returns no supported protocol marker, Calepin selects local runtime
mode.

If the query fails because a public Calepin package cannot be resolved, Calepin
should report a clear import/runtime diagnostic rather than falling through to a
confusing Typst package download error.

### Phase 2A: Protocol Shim Metadata

In protocol shim mode, Calepin should collect:

```typst
(
  protocol: query(<calepin-protocol>),
  setup: query(selector(<calepin-config>).or(<website-metadata>)),
  chunks: query(raw.where(block: true).or(<calepin-fence-label>).or(<calepin-chunk>)),
)
```

The exact chunk query can evolve, but the important point is that the public
shim owns the stable marker schema and the generated runtime owns rendering.

The generated wrapper should import the binary runtime and wrap the real
document body:

```typst
#import "/.calepin/calepin.typ" as runtime

#runtime.notebook-body[
  #include "/main.typ"
]
```

`notebook-body` should contain the raw-block show rules and marker show rules
that currently live partly in `staging.rs`.

In protocol shim mode, the wrapper should include the original root-relative
input path, not a copied `.calepin/.../source.typ`, unless Calepin has a
specific reason to stage a source copy. The shim package already supplies the
author-facing Calepin API, so no import rewrite is needed. Including the
original file preserves Typst's normal relative include/import/read semantics
for multi-file projects.

### Phase 2B: Local Runtime Metadata

In local runtime mode, Calepin expects files that call Calepin to import:

```typst
#import "/.calepin/calepin.typ" as calepin
```

The root-file import rewrite can remain temporarily:

```typst
#import "@preview/calepin:0.0.1" as calepin
```

may be staged as:

```typst
#import "/.calepin/calepin.typ" as calepin
```

but only for the root source file Calepin stages. Included files are not
rewritten. If an included file imports an unsupported public Calepin package,
that file will not use the local runtime and should be treated as unsupported.

Calepin should issue a diagnostic when local runtime mode sees evidence of the
old fallback path, such as an auto-labeled raw block where a nearby public
Calepin import likely discarded chunk options. The diagnostic should point users
to one of the two supported choices:

```typst
// development
#import "/.calepin/calepin.typ" as calepin

// public shim, once available
#import "@preview/calepin:0.0.2" as calepin
```

## Runtime Wrapper Refactor

`staging.rs` should stop generating the raw-block show-rule implementation
directly. It should assemble wrapper structure and data only.

The generated wrapper should become conceptually:

```typst
#import "/.calepin/calepin.typ" as runtime

#runtime.notebook-body(
  jupyter-kernels: ("julia-1.12", "bash"),
)[
  #include "/main.typ"
]
```

The runtime should provide:

```typst
#let notebook-body(jupyter-kernels: (), body) = [
  // show rules for raw code blocks
  // show rules for protocol render markers
  // body
]
```

Because Typst show rules are scoped, placing those rules inside `notebook-body`
and wrapping the body is the correct way to keep behavior in runtime `.typ`
files while still applying it to the document content.

In local runtime mode, Calepin may still use a staged root source when it needs
to rewrite the root import to `/.calepin/calepin.typ`. That staging limitation
is one reason local runtime mode should be framed as a development bridge, not
the long-term public-package design.

Avoid generating one show rule per language in Rust if possible. A generic
runtime rule can inspect `it.lang` and call the existing raw-block routing logic.
If Typst's show-rule syntax forces some static rule generation, keep that
generation as small as possible and delegate all behavior to runtime functions.

## Data Flow

### Protocol Shim Mode

1. Author imports `@preview/calepin:0.0.2` in any file.
2. The public shim emits protocol/setup/chunk/render markers.
3. Calepin probes for `<calepin-protocol>`.
4. Calepin parses `<calepin-config>` and `<calepin-chunk>` metadata.
5. Calepin executes chunks and writes `results.json`.
6. Calepin renders through the generated wrapper.
7. The generated runtime's `notebook-body` show rules replace render markers
   with output from `results.json`.

### Local Runtime Mode

1. Author imports `/.calepin/calepin.typ` in every file that calls Calepin.
2. The generated runtime itself emits setup/chunk metadata during query mode.
3. Calepin parses metadata, executes chunks, and writes `results.json`.
4. Calepin renders through the generated wrapper.
5. The generated runtime renders chunks directly or through marker show rules,
   depending on the local runtime implementation.

## Error Handling and Diagnostics

### Unsupported public package

If a document imports `@preview/calepin:0.0.1` and no protocol marker is found,
Calepin should not silently promise multi-file support. It should warn or error
with language like:

```text
This document uses an unsupported Calepin Typst package.
For multi-file documents, use @preview/calepin:0.0.2 or import
/.calepin/calepin.typ from every file that calls Calepin.
```

The final severity can be phased:

- Warning for a short transition period.
- Error once `@preview/calepin:0.0.2` is available and docs/scaffolds have moved.

### Mixed runtime imports

If supported protocol markers and local runtime markers both appear, Calepin
should error. Mixing `@preview/calepin:0.0.2` and `/.calepin/calepin.typ` in one
document would make marker ownership ambiguous.

### Missing protocol marker

If no marker is found and no local runtime metadata is found, Calepin should
report that no Calepin runtime was detected. This is clearer than executing zero
chunks silently.

### Package download failures

If Typst fails while resolving `@preview/calepin`, Calepin should preserve the
underlying Typst error but add context explaining that public package imports
come from Typst Universe and that development builds can use the local runtime
import instead.

## Migration Strategy

### Immediate development phase

- Update development docs and scaffolds to use:

```typst
#import "/.calepin/calepin.typ" as calepin
```

- Document that every included file that calls Calepin must import the same
  local runtime.
- Keep root-file import rewriting only as a convenience for simple documents.

### Shim release phase

- Publish `@preview/calepin:0.0.2` as the protocol shim.
- Update docs and scaffolds to use:

```typst
#import "@preview/calepin:0.0.2" as calepin
```

- Keep binary-generated rendering behavior in `.calepin`.
- Calepin probes for the protocol marker and selects shim mode.

### Stabilization phase

- Remove or narrow root-source import rewriting.
- Make unsupported public package versions an error.
- Keep the shim protocol stable across binary releases as long as possible.

## Testing

Add behavior tests for:

- A single-file document importing `/.calepin/calepin.typ` still executes and
  renders chunks.
- A multi-file document where both files import `/.calepin/calepin.typ`
  preserves included chunk labels and captions.
- A multi-file document where an included file imports old
  `@preview/calepin:0.0.1` does not silently lose labels; it emits the planned
  diagnostic.
- A synthetic protocol-shim fixture emits `<calepin-protocol>` and is selected
  over local runtime mode.
- A protocol-shim fixture in an included file is detected by the probe.
- Mixed protocol/local runtime markers error.
- Direct Typst-style fallback behavior remains available in the public shim
  when Calepin inputs are absent.

Tests should avoid network access. The protocol-shim package can be represented
by a small local fixture imported by path in runtime tests, or by a direct Typst
file that defines the same marker-emitting API. The goal is to test Calepin's
mode selection and marker handling, not Typst Universe download behavior.

## Open Questions

- Should old `@preview/calepin:0.0.1` be a warning or hard error in local runtime
  mode once this lands?
- Should Calepin keep root-source import rewriting at all after the protocol
  shim exists?
- What is the exact marker schema for render-position markers? The schema
  should be minimal and stable, likely label plus marker kind, with rendering
  options continuing to come from parsed chunk metadata and `results.json`.
- Should `@preview/calepin:0.0.2` support only marker emission, or also include
  the direct-compile warning fallback from `0.0.1`? The recommendation is to
  keep the fallback warning so direct `typst compile` remains understandable.

## Recommendation

Use explicit imports, not automatic namespace injection. During the current
heavy-development period, use local runtime imports deliberately:

```typst
#import "/.calepin/calepin.typ" as calepin
```

Every module that calls `calepin.*` should import Calepin explicitly. This is
consistent with Typst's module model even though the import target is
temporary. Do not add package-path overlays, global local package installation,
or recursive import insertion.

In parallel, prepare `@preview/calepin:0.0.2` as a small stable protocol shim.
Once released, Calepin should detect the shim by querying `<calepin-protocol>`.
If the marker is present and supported, use protocol shim mode. Otherwise use
local runtime mode and produce clear diagnostics for unsupported public package
imports.

This keeps fast-moving behavior in the binary-generated runtime, fixes
multi-file documents without recursive source rewriting, and limits Typst
Universe releases to public API/protocol changes rather than every runtime
implementation change.
