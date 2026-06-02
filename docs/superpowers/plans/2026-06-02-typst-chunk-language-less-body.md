# Typst Chunk Language-Less Body Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Change `calepin.chunk` so the engine is passed as the first positional argument and the body is a language-less raw block, e.g. `#ch("python")[``` ... ```]`.

**Architecture:** Keep the Typst runtime responsible for rendering and metadata emission, and keep `query.rs` responsible for extracting and validating chunk metadata from Typst query output. The parser will read the engine from the chunk’s positional argument, and the body will be accepted only when it contains exactly one raw node without a language tag. Echoed source and output text continue to render as standard Typst raw blocks, with the echoed input tagged using the explicit engine.

**Tech Stack:** Rust, Typst, `typst query`, existing calepin integration tests.

---

### Task 1: Update Typst metadata extraction

**Files:**
- Modify: `calepin/src/typst/query.rs:1-160`
- Test: `calepin/src/typst/query.rs:311-390`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn parse_valid_chunk_uses_positional_engine_and_language_less_raw_body() {
    let query_json = r#"[
      {
        "value": {
          "label": "plot",
          "engine": "python",
          "body": {
            "func": "raw",
            "text": "\nprint(42)\n"
          }
        }
      }
    ]"#;

    let chunks = parse_chunks(query_json, None).unwrap();
    assert_eq!(chunks[0].engine.as_str(), "python");
    assert_eq!(chunks[0].code, "print(42)\n");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path calepin/Cargo.toml parse_valid_chunk_uses_positional_engine_and_language_less_raw_body -v`
Expected: FAIL because `extract_code` still rejects or misreads the new body shape.

- [ ] **Step 3: Write minimal implementation**

```rust
fn extract_code(body: &Value, label: &str) -> Result<String> {
    let raw = extract_raw_node(body, label)?;
    if raw.get("lang").is_some_and(|lang| !lang.is_null()) {
        return Err(anyhow!("chunk `{}` raw element must not declare a language", label));
    }
    raw.get("text")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow!("chunk `{}` raw element is missing text", label))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path calepin/Cargo.toml parse_valid_chunk_uses_positional_engine_and_language_less_raw_body -v`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add calepin/src/typst/query.rs
git commit -m "fix: accept language-less typst chunk bodies"
```

### Task 2: Update Typst runtime API and rendering

**Files:**
- Modify: `calepin/src/typst/runtime.typ:1-260`
- Test: `calepin/src/typst/runtime.rs:1-180`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn runtime_source_exposes_positional_engine_chunk_api() {
    assert!(RUNTIME_SOURCE.contains("#let chunk(engine"));
    assert!(RUNTIME_SOURCE.contains("chunk(\"python\", label: \"answer\""));
    assert!(RUNTIME_SOURCE.contains("raw(code, block: true, lang: engine)"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path calepin/Cargo.toml runtime_source_exposes_positional_engine_chunk_api -v`
Expected: FAIL because `chunk` still takes `engine:` as a named argument.

- [ ] **Step 3: Write minimal implementation**

```typst
#let chunk(
  engine,
  body,
  label: none,
  cache: auto,
  echo: auto,
  eval: auto,
  include_: auto,
  results: auto,
  warning: auto,
  message: auto,
  error: auto,
  format: auto,
  item: auto,
  placeholder: auto,
  dev: auto,
  dpi: auto,
  fig-width: auto,
  fig-height: auto,
  out-width: auto,
  out-height: auto,
  fig-cap: none,
  fig-alt: none,
  tbl-cap: none,
  kind: auto,
) = {
  let code = _raw-text(body)
  let code = if code.starts-with("\n") { code.slice(1) } else { code }
  ...
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path calepin/Cargo.toml runtime_source_exposes_positional_engine_chunk_api -v`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add calepin/src/typst/runtime.typ calepin/src/typst/runtime.rs
git commit -m "feat: make typst chunk engine positional"
```

### Task 3: Update example document and integration coverage

**Files:**
- Modify: `examples/basic.typ:1-120`
- Modify: `calepin/tests/typst_preprocess.rs:1-140`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn preprocess_accepts_language_less_raw_chunk_body() {
    let input = r#"#import ".calepin/calepin.typ"

#calepin.setup(cache: false)

#calepin.chunk("python", label: "answer")[`
print("FALLBACK_12345")
`]
"#;

    // Existing preprocess integration should render the fallback source and preserve code block styling.
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path calepin/Cargo.toml preprocess_accepts_language_less_raw_chunk_body -v`
Expected: FAIL until the example and test fixture use the new positional-engine form.

- [ ] **Step 3: Write minimal implementation**

```typst
#calepin.chunk("python", label: "answer")[`
print("FALLBACK_12345")
`]
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path calepin/Cargo.toml --test typst_preprocess -v`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add examples/basic.typ calepin/tests/typst_preprocess.rs
git commit -m "docs: update calepin chunk examples"
```

