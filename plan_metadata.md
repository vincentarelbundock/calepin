# Metadata Module: Rename & Restructure Plan

## Current layout

- `metadata/types.rs` (462 lines) -- data structures + methods (merge, overrides, date, inline eval)
- `metadata/parse.rs` (691 lines) -- front matter parsing, author/affiliation/scholarly parsing
- `metadata/mod.rs` (8 lines) -- re-exports
- `render/metadata.rs` (408 lines) -- format-specific author/appendix/citation rendering

---

## Recommended changes

### 1. Rename `render/metadata.rs` to `render/scholarly.rs`

It only builds scholarly front matter output: author blocks with affiliations/ORCID, appendix sections (license, citation, copyright, funding). Called only from `render/template.rs` via `build_appendix`, `build_authors`, `strip_markdown_formatting`.

Files: `render/metadata.rs`, `render/mod.rs`, `render/template.rs`

### 2. Extract date helpers to `metadata/date.rs`

`types.rs` contains ~90 lines of pure date utility code (`epoch_days_to_date`, `is_leap`, `format_date_str`, `format_date`, `format_ymd`) that don't belong with type definitions. Move `resolve_date` there too since it's purely date logic.

Files: `metadata/types.rs`, new `metadata/date.rs`, `metadata/mod.rs`

### 3. Move `strip_markdown_formatting` to `util.rs`

It strips `![alt](url)` and `[text](url)` syntax for `<title>` generation. This is a text utility, not metadata-specific. Belongs alongside `escape_html`.

Files: `render/metadata.rs`, `util.rs`, `render/template.rs`

### 4. Rename `author` field to `author_names`

Two fields exist: `author: Option<Vec<String>>` (flat names) and `authors: Vec<Author>` (rich structs). The naming is confusing and risks them getting out of sync. Renaming `author` to `author_names` makes the distinction explicit. Alternative: derive it from `authors` via a method instead of storing separately.

Files: `metadata/types.rs`, `metadata/parse.rs`, `render/metadata.rs`, `render/template.rs`, `jinja/variables.rs`, `engines/mod.rs`

### 5. Extract `Fmt` to shared location

`Fmt` is a zero-sized struct with static format-dispatch methods (`link`, `superscript`, `emphasis`, `url`) trapped inside the scholarly rendering file. Format-specific link/emphasis generation is done ad-hoc elsewhere too. Moving it to `render/fmt.rs` or similar would reduce duplication.

Files: `render/metadata.rs`, new shared location

### 6. Remove legacy parsing sections

`identity`, `shortcodes`, `formats` in `parse.rs` (lines 162-198) are legacy grouped sections that flatten into top-level fields. Remove entirely -- ~40 lines of unnecessary code.

Files: `metadata/parse.rs`

---

## No change needed

- **`parse_metadata()` decomposition** -- already delegates to `parse_authors()`, `parse_copyright()`, `parse_license()`, `parse_citation()`, `parse_funding()`. The remaining match arms are one-liners (simple scalars, `deserialize_section()` calls). No further splitting needed.
- **`abstract_text` field name** -- necessary workaround for `abstract` being a Rust keyword.
- **`Metadata` storing both document and project concerns** -- the merge-based approach is practical. Splitting into sub-structs would add complexity without benefit.
- **`parse.rs` mixing front matter splitting with deserialization** -- tightly coupled, called together.
- **`build_template_vars_with_headings` in `render/template.rs`** -- template machinery that consumes metadata, not metadata logic.

---

## Verification

- `make test` -- all existing tests pass
- `make docs` -- website renders correctly
- `cargo test test_split_frontmatter test_simple_author_string test_rich_author_with_affiliations`
