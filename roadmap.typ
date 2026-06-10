= Roadmap Notes

== Typst `eval` for document introspection

Typst `eval --in` could eventually replace Calepin's direct use of `typst query` for document metadata extraction. Today, Calepin shells out to `typst query` for three introspection passes:

- setup and website metadata, using `<calepin-config>` and `<website-metadata>`;
- executable chunk discovery, using raw block, fence-label, and `<calepin-chunk>` selectors;
- page-sync anchors, using `<calepin-page>`.

The mechanical replacement is straightforward:

```sh
typst query input.typ '<calepin-chunk>' ...
typst eval --in input.typ 'query(<calepin-chunk>)' --format=json ...
```

The main reason to consider `eval` is that it can shape the JSON in Typst before Rust parses it. Instead of receiving generic query records and then filtering or splitting them in Rust, Calepin could ask for a document-specific object:

```typ
(
  setup: query(<calepin-config>).map(it => it.value),
  page_meta: query(<website-metadata>).first().value,
  chunks: query(raw.where(block: true).or(<calepin-fence-label>).or(<calepin-chunk>)),
)
```

This could collapse the current setup/page-metadata and chunk-discovery query calls into one structured eval call. It would also make page sync cleaner, because `eval` can run transformations over introspection results:

```typ
query(<calepin-page>).map(it => (
  label: it.value.label,
  page: it.location().page(),
))
```

That may allow Calepin to stop embedding page numbers directly into `<calepin-page>` metadata with `here().page()`, or at least reduce the amount of Rust-side parsing around page anchors.

The migration should be gated by Typst version support. Calepin currently supports Typst 0.14.x, while `typst eval` appears to land in Typst 0.15. A conservative path is:

1. Add a `typst_eval` helper next to the existing `typst_query` helper.
2. Use `eval` only when the configured Typst executable supports it.
3. Migrate page sync first, because its output can be made more directly useful.
4. Migrate setup/page metadata and chunk discovery to a single structured eval call.
5. Keep `typst query` as fallback until Calepin raises its minimum Typst version or Typst formally deprecates `query`.
