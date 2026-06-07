# Cross-references Milestone 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `calepin.chunk(label: "fig-x")` (and `label: ("fig-x", ...)`) attach a real, prefix-routed Typst cross-reference label to a chunk's figure, so `@fig-x` resolves — with strict validation.

**Architecture:** The Typst runtime already computes each chunk's label/options and ships them to Rust as `<calepin-chunk>` metadata. We extend `_emit-chunk` to accept a string-or-list `label`, derive an internal id (first entry, else auto `chunk-N`), and emit the raw label names as `crossref-labels` metadata. Rust classifies those names by prefix (`fig-`/`tbl-`/`lst-`), validates them (strict prefix, distinct kinds), and carries them through `results.json`. The runtime's render pass reads the classified labels back from `results.json` and attaches `fig-` labels to the figure (forcing a `figure` wrapper even without a caption). Only `fig-` routing ships in M1; `lst-`/`tbl-` classification exists but routing arrives in later milestones.

**Tech Stack:** Rust (serde_json, anyhow), the embedded Typst runtime (`runtime/*.typ`), real `typst` + `python3`/`pdftotext` for integration tests.

**Commits:** The repo owner does not run `git add`/`git commit` automatically. At each "Commit" step, present the pasteable command for the owner to run; do not execute it yourself.

---

## Milestone roadmap (context only — M2–M5 get their own plans)

- **M1 (this plan):** `calepin.chunk(label: ...)` arg channel; `fig-` routing; Rust foundation (crossref classification, model fields, `results.json` contract); strict prefix + distinct-kind + non-empty validation.
- **M2:** Ordinal plumbing so the runtime learns labels Rust derived without a direct arg; enables the `#| label:` header channel; arg-vs-header conflict = error.
- **M3:** `lst-` (code listing -> `figure(kind: raw)`) and `tbl-` (table -> `figure(kind: table)`) routing; multi-kind lists; per-kind captions (`lst-cap`, `tbl-cap`); tinytable nesting check.
- **M4:** Harvested fence label `` ```r...```<fig-x> `` via query-time read + source strip step (new `process.rs`), reusing M2 ordinal plumbing.
- **M5:** Panel sub-labels `fig-x-1`, `fig-x-2` attached to grid cells.

---

## File structure

- Create: `calepin/src/typst/crossref.rs` — pure classification/validation of label names into kinds. One responsibility, no I/O.
- Modify: `calepin/src/typst/mod.rs` — register the `crossref` module.
- Modify: `calepin/src/typst/model.rs` — add `crossref_labels` to `ChunkSpec` and `ChunkResultDocument`.
- Modify: `calepin/src/typst/query.rs` — read `crossref-labels` from chunk metadata, classify, attach to `ChunkSpec`.
- Modify: `calepin/src/typst/results.rs` — carry `crossref_labels` from `ChunkSpec` into `ChunkResultDocument`.
- Modify: `calepin/src/typst/execute.rs` — populate `crossref_labels` when building `ChunkResultDocument`.
- Modify: `calepin/src/typst/runtime/chunk.typ` — string-or-list `label`; derive id; emit `crossref-labels` metadata.
- Modify: `calepin/src/typst/runtime/render.typ` — route `fig-` labels from `results.json` onto the figure; force the `figure` wrapper when a `fig-` label is present without a caption.
- Modify: `calepin/src/typst/runtime/state.typ` — small helper to attach a list of labels.
- Test: `calepin/tests/typst_preprocess.rs` — integration test that `@fig-x` resolves end-to-end.

---

## Task 1: crossref classification module

**Files:**
- Create: `calepin/src/typst/crossref.rs`
- Modify: `calepin/src/typst/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `calepin/src/typst/crossref.rs` with only the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_recognized_prefixes() {
        assert_eq!(classify_label("fig-x").unwrap().kind, CrossrefKind::Fig);
        assert_eq!(classify_label("tbl-x").unwrap().kind, CrossrefKind::Tbl);
        assert_eq!(classify_label("lst-x").unwrap().kind, CrossrefKind::Lst);
    }

    #[test]
    fn keeps_full_name_including_prefix() {
        assert_eq!(classify_label("fig-plot").unwrap().name, "fig-plot");
    }

    #[test]
    fn rejects_unprefixed_label() {
        let err = classify_label("myplot").unwrap_err().to_string();
        assert!(err.contains("myplot"), "{err}");
        assert!(err.contains("fig-"), "{err}");
    }

    #[test]
    fn parses_single_string_into_one_label() {
        let labels = parse_label_names(&["fig-x".to_string()]).unwrap();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].kind, CrossrefKind::Fig);
    }

    #[test]
    fn parses_distinct_kinds_list() {
        let labels = parse_label_names(&["fig-x".to_string(), "lst-y".to_string()]).unwrap();
        assert_eq!(labels.len(), 2);
    }

    #[test]
    fn rejects_duplicate_kinds() {
        let err = parse_label_names(&["fig-a".to_string(), "fig-b".to_string()])
            .unwrap_err()
            .to_string();
        assert!(err.contains("fig"), "{err}");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail to compile**

Run: `cargo test --manifest-path calepin/Cargo.toml crossref`
Expected: FAIL — `CrossrefKind`, `classify_label`, `parse_label_names` not found.

- [ ] **Step 3: Implement the module**

Prepend to `calepin/src/typst/crossref.rs` (above the test module):

```rust
use anyhow::{anyhow, Result};

/// Recognized cross-reference kinds, selected by a label-name prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossrefKind {
    Fig,
    Tbl,
    Lst,
}

/// The fixed, ordered prefix table. Extend here to add a kind.
const PREFIXES: [(&str, CrossrefKind); 3] = [
    ("fig-", CrossrefKind::Fig),
    ("tbl-", CrossrefKind::Tbl),
    ("lst-", CrossrefKind::Lst),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossrefLabel {
    pub kind: CrossrefKind,
    /// Full label name including the prefix, e.g. `fig-plot`.
    pub name: String,
}

/// Classify one label name by its prefix. Strict: an unrecognized prefix is an error.
pub fn classify_label(name: &str) -> Result<CrossrefLabel> {
    for (prefix, kind) in PREFIXES {
        if name.starts_with(prefix) && name.len() > prefix.len() {
            return Ok(CrossrefLabel { kind, name: name.to_string() });
        }
    }
    Err(anyhow!(
        "label `{}` has no recognized cross-reference prefix (expected one of: fig-, tbl-, lst-)",
        name
    ))
}

/// Classify a list of names; reject empty lists and repeated kinds.
pub fn parse_label_names(names: &[String]) -> Result<Vec<CrossrefLabel>> {
    if names.is_empty() {
        return Err(anyhow!("label list is empty"));
    }
    let mut labels = Vec::with_capacity(names.len());
    let mut seen_kinds: Vec<CrossrefKind> = Vec::new();
    for name in names {
        let label = classify_label(name)?;
        if seen_kinds.contains(&label.kind) {
            return Err(anyhow!(
                "label list has more than one `{}` entry; use one label per kind",
                &name[..name.find('-').map(|i| i + 1).unwrap_or(name.len())]
            ));
        }
        seen_kinds.push(label.kind);
        labels.push(label);
    }
    Ok(labels)
}
```

- [ ] **Step 4: Register the module**

In `calepin/src/typst/mod.rs`, add alongside the other `mod` declarations:

```rust
mod crossref;
```

(If sibling modules are `pub mod`, match the surrounding style instead.)

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --manifest-path calepin/Cargo.toml crossref`
Expected: PASS (6 tests).

- [ ] **Step 6: Commit**

Present for the owner to run:
```bash
git add calepin/src/typst/crossref.rs calepin/src/typst/mod.rs
git commit -m "feat(crossref): classify label names by prefix"
```

---

## Task 2: carry crossref labels in the data model

**Files:**
- Modify: `calepin/src/typst/model.rs:190-198` (`ChunkSpec`)
- Modify: `calepin/src/typst/model.rs:308-316` (`ChunkResultDocument`)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `calepin/src/typst/model.rs`:

```rust
#[test]
fn chunk_result_document_serializes_crossref_labels() {
    let doc = ChunkResultDocument {
        label: "fig-x".to_string(),
        engine: EngineName::R,
        status: ChunkStatus::Ok,
        display_options: serde_json::from_str(
            r#"{"echo":true,"output":true,"results":"render","warning":true,
                "message":true,"placeholder":true,"fig-width":null,"fig-height":null,
                "fig-align":null,"fig-responsive":null,"fig-link":null,"fig-caption":null,
                "fig-cap-location":null,"fig-alt-text":null,"fig-subcaptions":null,
                "fig-layout-columns":null,"fig-layout-rows":null,"kind":null}"#,
        )
        .unwrap(),
        items: vec![],
        crossref_labels: vec![CrossrefLabelDoc {
            kind: "fig".to_string(),
            name: "fig-x".to_string(),
        }],
    };
    let json = serde_json::to_string(&doc).unwrap();
    assert!(json.contains(r#""crossref-labels""#), "{json}");
    assert!(json.contains(r#""fig-x""#), "{json}");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path calepin/Cargo.toml model::tests::chunk_result_document_serializes_crossref_labels`
Expected: FAIL — `CrossrefLabelDoc` and field `crossref_labels` not found.

- [ ] **Step 3: Add the serialized label type and fields**

In `calepin/src/typst/model.rs`, add near the other small serde structs:

```rust
/// Serialized form of a routed cross-reference label, written into results.json
/// and read back by the Typst runtime. `kind` is one of "fig" | "tbl" | "lst".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossrefLabelDoc {
    pub kind: String,
    pub name: String,
}
```

Add a field to `ChunkSpec` (after `ordinal`):

```rust
    #[serde(default)]
    pub crossref_labels: Vec<CrossrefLabelDoc>,
```

Add a field to `ChunkResultDocument` (after `items`):

```rust
    #[serde(rename = "crossref-labels", default, skip_serializing_if = "Vec::is_empty")]
    pub crossref_labels: Vec<CrossrefLabelDoc>,
```

- [ ] **Step 4: Add a conversion helper on `crossref::CrossrefLabel`**

In `calepin/src/typst/crossref.rs`, add to the impl area:

```rust
impl CrossrefKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CrossrefKind::Fig => "fig",
            CrossrefKind::Tbl => "tbl",
            CrossrefKind::Lst => "lst",
        }
    }
}

impl CrossrefLabel {
    pub fn to_doc(&self) -> crate::typst::model::CrossrefLabelDoc {
        crate::typst::model::CrossrefLabelDoc {
            kind: self.kind.as_str().to_string(),
            name: self.name.clone(),
        }
    }
}
```

Make the module visible to siblings: ensure `mod.rs` exposes what query/results need (use `pub(crate) mod crossref;` or `pub use` as the surrounding style dictates).

- [ ] **Step 5: Fix existing `ChunkSpec`/`ChunkResultDocument` constructors**

Search and update every struct-literal so it compiles:
Run: `cargo build --manifest-path calepin/Cargo.toml 2>&1 | head -40`
For each "missing field `crossref_labels`" site (in `query.rs`, `execute.rs`, `results.rs`, and their test modules), add `crossref_labels: vec![]` or `crossref_labels: Vec::new()`.

- [ ] **Step 6: Run to verify pass**

Run: `cargo test --manifest-path calepin/Cargo.toml model::tests::chunk_result_document_serializes_crossref_labels`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add calepin/src/typst/model.rs calepin/src/typst/crossref.rs
git commit -m "feat(crossref): carry routed labels through the data model"
```

---

## Task 3: emit `crossref-labels` from the runtime (query metadata)

**Files:**
- Modify: `calepin/src/typst/runtime/chunk.typ:36-49` (`_emit-chunk`)
- Modify: `calepin/src/typst/runtime/state.typ` (id-derivation helper)

- [ ] **Step 1: Add a runtime unit test (typst query)**

Add to the `tests` module in `calepin/src/typst/runtime.rs`, mirroring the existing query tests (e.g. the one around line 118). Document body:

```rust
#[test]
fn query_emits_crossref_labels_for_list_label() {
    // skip when typst is unavailable, matching sibling tests
    let Some(typst) = typst_binary() else { return };
    let doc = r#"#import "calepin.typ": *
#chunk("r", label: ("fig-x", "lst-y"))[```r
1+1
```]
"#;
    let stdout = run_query(&typst, doc, "<calepin-chunk>");
    assert!(stdout.contains(r#""crossref-labels""#), "{stdout}");
    assert!(stdout.contains(r#""fig-x""#), "{stdout}");
    assert!(stdout.contains(r#""lst-y""#), "{stdout}");
    // internal id is the primary (first) label
    assert!(stdout.contains(r#""label": "fig-x""#), "{stdout}");
}
```

Reuse whatever helpers the existing runtime tests use to write the runtime and run `typst query`; if they are inline, copy that setup exactly (write `calepin.typ` = `RUNTIME_SOURCE`, run `typst query <file> "<calepin-chunk>"`).

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path calepin/Cargo.toml runtime::tests::query_emits_crossref_labels_for_list_label -- --nocapture`
Expected: FAIL — no `crossref-labels` in output (and a list `label` currently panics or stringifies wrong).

- [ ] **Step 3: Add the id/labels derivation helper**

In `calepin/src/typst/runtime/state.typ`, add:

```typst
// Accept `label` as none | str | array of str. Returns the internal id (used
// for results lookup + artifact filenames) and the raw label-name list.
#let _derive-label(label-opt, generated-prefix, counter-value) = {
  if label-opt == none {
    (id: generated-prefix + "-" + str(counter-value), names: (), generated: true)
  } else if type(label-opt) == str {
    (id: label-opt, names: (label-opt,), generated: false)
  } else if type(label-opt) == array {
    if label-opt.len() == 0 { panic("calepin.chunk: label list must not be empty") }
    for entry in label-opt {
      if type(entry) != str { panic("calepin.chunk: label entries must be strings") }
    }
    (id: label-opt.first(), names: label-opt, generated: false)
  } else {
    panic("calepin.chunk: label must be a string or an array of strings")
  }
}
```

- [ ] **Step 4: Use it in `_emit-chunk`**

In `calepin/src/typst/runtime/chunk.typ`, replace the label block at the top of `_emit-chunk` (lines ~38-47) with:

```typst
  let label-opt = options.at("label")
  let auto-label-state = options.at("auto-label-state")
  let auto-label-prefix = options.at("auto-label-prefix")
  let derived = _derive-label(label-opt, auto-label-prefix, str(auto-label-state.get()))
  let label = derived.id
  let crossref-names = derived.names
  let generated-label = derived.generated
  let label-step = if generated-label {
    auto-label-state.update(n => n + 1)
  } else {
    _sync-auto-label-counter(auto-label-state, label)
  }
```

In the query branch (currently `[#label-step #metadata(_chunk-spec(body, engine, label, options)) <calepin-chunk>]`), thread the names through. Change `_chunk-spec` in the same file to accept and emit them:

```typst
#let _chunk-spec(body, engine, label, crossref-names, options) = {
  let out = (
    body: body,
    engine: engine,
    label: label,
    "crossref-labels": crossref-names,
  )
  for key in _base-options.keys() {
    if key != "fenced-chunks" {
      out.insert(key, options.at(key))
    }
  }
  out
}
```

and update its single call site to `_chunk-spec(body, engine, label, crossref-names, options)`.

- [ ] **Step 5: Run to verify pass**

Run: `cargo test --manifest-path calepin/Cargo.toml runtime::tests::query_emits_crossref_labels_for_list_label`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add calepin/src/typst/runtime/state.typ calepin/src/typst/runtime/chunk.typ
git commit -m "feat(crossref): runtime emits crossref-labels for string-or-list label"
```

---

## Task 4: classify metadata labels in Rust query

**Files:**
- Modify: `calepin/src/typst/query.rs:91-138` (`parse_chunk_metadata`)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `calepin/src/typst/query.rs`, following the existing JSON-driven tests (e.g. around line 1089):

```rust
#[test]
fn classifies_crossref_labels_from_metadata() {
    let json = r#"[
      {"func":"metadata","value":{
        "body":{"func":"raw","text":"1","block":true,"lang":"r"},
        "engine":"r","label":"fig-x","crossref-labels":["fig-x"]
      },"label":"<calepin-chunk>"}
    ]"#;
    let result = parse_chunks(json, None).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].crossref_labels.len(), 1);
    assert_eq!(result[0].crossref_labels[0].kind, "fig");
    assert_eq!(result[0].crossref_labels[0].name, "fig-x");
}

#[test]
fn rejects_unprefixed_metadata_label() {
    let json = r#"[
      {"func":"metadata","value":{
        "body":{"func":"raw","text":"1","block":true,"lang":"r"},
        "engine":"r","label":"myplot","crossref-labels":["myplot"]
      },"label":"<calepin-chunk>"}
    ]"#;
    let err = parse_chunks(json, None).unwrap_err().to_string();
    assert!(err.contains("myplot"), "{err}");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path calepin/Cargo.toml query::tests::classifies_crossref_labels_from_metadata query::tests::rejects_unprefixed_metadata_label`
Expected: FAIL — `crossref_labels` always empty / no error raised.

- [ ] **Step 3: Read and classify the names in `parse_chunk_metadata`**

In `calepin/src/typst/query.rs`, add this import at the top:

```rust
use crate::typst::crossref::parse_label_names;
```

In `parse_chunk_metadata`, just before constructing `ChunkSpec`, add:

```rust
    let crossref_names: Vec<String> = value
        .get("crossref-labels")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let crossref_labels = if crossref_names.is_empty() {
        Vec::new()
    } else {
        parse_label_names(&crossref_names)
            .map_err(|e| anyhow!("chunk `{}`: {}", label, e))?
            .iter()
            .map(|l| l.to_doc())
            .collect()
    };
```

Then add `crossref_labels,` to the `ChunkSpec { ... }` literal.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --manifest-path calepin/Cargo.toml query::tests::classifies_crossref_labels_from_metadata query::tests::rejects_unprefixed_metadata_label`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add calepin/src/typst/query.rs
git commit -m "feat(crossref): classify and validate metadata labels at query time"
```

---

## Task 5: propagate labels into results.json

**Files:**
- Modify: `calepin/src/typst/execute.rs:44-95` (where `ChunkResultDocument` is built)
- Check: `calepin/src/typst/results.rs:10-20` (map assembly keyed by `chunk.label`)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `calepin/src/typst/results.rs`:

```rust
#[test]
fn results_document_preserves_crossref_labels() {
    use crate::typst::model::CrossrefLabelDoc;
    let chunk = ChunkResultDocument {
        label: "fig-x".to_string(),
        engine: EngineName::R,
        status: ChunkStatus::Ok,
        display_options: default_display_options_for_test(),
        items: vec![],
        crossref_labels: vec![CrossrefLabelDoc { kind: "fig".into(), name: "fig-x".into() }],
    };
    let doc = build_results_document("paper.typ", "0.0.0", vec![chunk]);
    assert_eq!(doc.chunks["fig-x"].crossref_labels[0].name, "fig-x");
}
```

If `results.rs` lacks a display-options test helper, build `DisplayOptions` via `serde_json::from_str` as in Task 2 Step 1, or reuse the existing helper the sibling test (`builds_results_document_keyed_by_label`) uses.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path calepin/Cargo.toml results::tests::results_document_preserves_crossref_labels`
Expected: FAIL to compile (missing field) until execute.rs sets it.

- [ ] **Step 3: Populate `crossref_labels` when building the document**

In `calepin/src/typst/execute.rs`, every place a `ChunkResultDocument { ... }` is constructed from a `ChunkSpec` (the success and error/skip paths near lines 44-95), add:

```rust
            crossref_labels: chunk.crossref_labels.clone(),
```

`results.rs` keys the map by `chunk.label` (the internal id) and needs no change beyond compiling.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --manifest-path calepin/Cargo.toml results::tests::results_document_preserves_crossref_labels`
Expected: PASS.

- [ ] **Step 5: Run the full suite**

Run: `cargo test --manifest-path calepin/Cargo.toml`
Expected: PASS (fix any remaining struct-literal sites flagged by the compiler with `crossref_labels: vec![]`).

- [ ] **Step 6: Commit**

```bash
git add calepin/src/typst/execute.rs calepin/src/typst/results.rs
git commit -m "feat(crossref): write routed labels into results.json"
```

---

## Task 6: attach `fig-` labels onto the figure (runtime render)

**Files:**
- Modify: `calepin/src/typst/runtime/state.typ` (multi-label attach helper)
- Modify: `calepin/src/typst/runtime/render.typ:438-518` and `:564-599` (`_render-display-item`, `_render-image-grid`, `_render-results`)

- [ ] **Step 1: Add the multi-attach helper**

In `calepin/src/typst/runtime/state.typ`, alongside `_attach-label`:

```typst
// Attach every figure-kind label name from a results chunk to `content`.
#let _attach-fig-labels(content, chunk) = {
  let labels = chunk.at("crossref-labels", default: ())
  let names = labels.filter(l => l.at("kind") == "fig").map(l => l.at("name"))
  if names.len() == 0 {
    content
  } else {
    let acc = content
    for name in names { acc = [#acc #label(name)] }
    acc
  }
}

// True when this chunk carries at least one `fig-` cross-reference label.
#let _has-fig-label(chunk) = {
  chunk.at("crossref-labels", default: ()).any(l => l.at("kind") == "fig")
}
```

- [ ] **Step 2: Thread the chunk dict into figure rendering**

In `render.typ`, `_render-results` already loads `chunk`. Pass it down: change `_render-image-grid(items, label, opts)` and `_render-display-item(item, label, opts)` call sites to also receive `chunk`, and inside them replace the `fig-caption != none` gate around `_attach-label(...)` so a figure is also produced/labelled when `_has-fig-label(chunk)` is true.

Concretely, in `_render-display-item` image branch, where it currently does:

```typst
    let rendered = if fig-caption != none {
      _attach-label(figure(img, caption: _figure-caption(fig-caption, fig-cap-location)), label)
    } else {
      img
    }
```

replace with:

```typst
    let rendered = if fig-caption != none {
      _attach-fig-labels(figure(img, caption: _figure-caption(fig-caption, fig-cap-location)), chunk)
    } else if _has-fig-label(chunk) {
      _attach-fig-labels(figure(img), chunk)
    } else {
      img
    }
```

Apply the same pattern in the HTML-captioned branch and in `_render-image-grid` (force a `figure(content)` wrapper when `_has-fig-label(chunk)` even if `fig-caption == none`). Keep `label` (the id) for results lookup; use `chunk` for label attachment.

- [ ] **Step 3: Enforce missing-target (strict)**

At the end of `_render-results`, after the item loop, add:

```typst
  if _has-fig-label(chunk) and not _chunk-produced-image {
    panic("chunk `" + label + "` carries a fig- label but produced no image")
  }
```

Track `_chunk-produced-image` by setting it true whenever an image item is rendered in the loop (initialise `let _chunk-produced-image = false` before the loop and set it in the image-group branches).

- [ ] **Step 4: Add the integration test**

In `calepin/tests/typst_preprocess.rs`, following the existing skip-when-missing pattern (return early if `typst`/`python3`/`pdftotext` absent), add a test that writes a document like:

```typst
#import "@preview/calepin:0.0.1" as calepin
#calepin.setup(echo: false)
See @fig-demo.
#calepin.chunk("r", label: "fig-demo", fig-caption: [Demo])[```r
plot(1:10)
```]
```

run the built `calepin` binary to preprocess + `typst compile` to PDF, and assert with `pdftotext` that the output contains `Figure 1` (the resolved reference), not an error. Add a second case with **no** `fig-caption` asserting compilation still succeeds and `@fig-demo` resolves. Assert observable behaviour only — do not pin generated `.typ` or exact byte output.

- [ ] **Step 5: Run to verify pass**

Run: `cargo test --manifest-path calepin/Cargo.toml`
Then: `cargo test --manifest-path calepin/Cargo.toml --test typst_preprocess`
Expected: PASS (or SKIP if `typst` absent — confirm it is present with `typst --version`).

- [ ] **Step 6: Commit**

```bash
git add calepin/src/typst/runtime/state.typ calepin/src/typst/runtime/render.typ calepin/tests/typst_preprocess.rs
git commit -m "feat(crossref): attach fig- labels to figures and reference them"
```

---

## Task 7: lints, docs note, milestone close

**Files:**
- Modify: `docs/tables_and_figures.typ` (optional: demonstrate `@fig-diagnostics`)

- [ ] **Step 1: Clippy + full test**

Run: `cargo clippy --manifest-path calepin/Cargo.toml`
Run: `cargo test --manifest-path calepin/Cargo.toml`
Expected: no warnings introduced by new code; all tests pass.

- [ ] **Step 2: Demonstrate the reference in docs**

In `docs/tables_and_figures.typ`, add a sentence near the `fig-diagnostics` chunk that references it (`As shown in @fig-diagnostics, ...`) so the docs exercise the new capability. Behavior-level only.

- [ ] **Step 3: Commit**

```bash
git add docs/tables_and_figures.typ
git commit -m "docs: reference a figure by its chunk label"
```

---

## Self-review notes

- **Spec coverage (M1 scope):** prefix routing (`fig-`) — Tasks 1,6; `label:` string-or-list — Tasks 3,4; strict unprefixed error — Tasks 1,4; distinct-kind error — Task 1; internal id vs labels split (id stays `ChunkSpec.label`, labels added) — Tasks 2,3; `results.json` contract — Tasks 2,5; missing-target error — Task 6. Channels other than the `label:` arg, `lst-`/`tbl-`/panels, and fence-harvest are explicitly deferred to M2–M5.
- **No placeholders:** every code step shows the code; struct-literal fixes are discovered via the compiler (Task 2 Step 5, Task 5 Step 5) rather than guessed.
- **Type consistency:** `CrossrefKind`/`CrossrefLabel`/`classify_label`/`parse_label_names` (Task 1) reused in Tasks 2,4; serialized as `CrossrefLabelDoc { kind, name }` with JSON key `crossref-labels` in Rust (Task 2) and read under the same key in the runtime (`crossref-labels`, Task 6); internal id is `ChunkSpec.label` throughout.
