
     ╭────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╮
     │ Plan: Batch Rendering + Site Builder Extraction + Plugin Improvements                                                                                          │
     │                                                                                                                                                                │
     │ Context                                                                                                                                                        │
     │                                                                                                                                                                │
     │ Calepin currently has ~600 lines of site-building code (Astro, npm, website config parsing) baked into the core binary. This code is only used when the input  │
     │ is a .yaml config file, yet it adds complexity, npm knowledge, and an entire WASM plugin protocol (build_site) to the core.                                    │
     │                                                                                                                                                                │
     │ The goal is to:                                                                                                                                                │
     │ 1. Remove all site-building from core — it becomes an external tool that calls calepin                                                                         │
     │ 2. Add parallel batch rendering so external tools can efficiently render multiple files                                                                        │
     │ 3. Add preprocess/postprocess script hooks to the custom format system                                                                                         │
     │                                                                                                                                                                │
     │ Phase 1: Remove site building from core                                                                                                                        │
     │                                                                                                                                                                │
     │ Pure deletion. Do first to simplify the codebase before adding new features.                                                                                   │
     │                                                                                                                                                                │
     │ Delete entirely                                                                                                                                                │
     │                                                                                                                                                                │
     │ - calepin/src/website.rs (307 lines)                                                                                                                           │
     │ - calepin/src/site_builder.rs (287 lines)                                                                                                                      │
     │ - plugins/astro/ (entire crate — ~870 lines)                                                                                                                   │
     │                                                                                                                                                                │
     │ Modify calepin/src/main.rs                                                                                                                                     │
     │                                                                                                                                                                │
     │ - Remove mod site_builder; and mod website; declarations                                                                                                       │
     │ - Remove the .yaml/.yml input detection block (lines 87-101) — this was the entry point for site building                                                      │
     │                                                                                                                                                                │
     │ Modify calepin/src/preview/mod.rs                                                                                                                              │
     │                                                                                                                                                                │
     │ - Remove pub fn run_website() (~78 lines)                                                                                                                      │
     │ - Remove fn site_npm_install() (~17 lines)                                                                                                                     │
     │ - Remove fn site_npm_build() (~14 lines)                                                                                                                       │
     │                                                                                                                                                                │
     │ Modify calepin/src/preview/server.rs                                                                                                                           │
     │                                                                                                                                                                │
     │ - Remove pub fn start_site() (~75 lines) — only called from run_website()                                                                                      │
     │                                                                                                                                                                │
     │ Modify calepin/src/preview/watcher.rs                                                                                                                          │
     │                                                                                                                                                                │
     │ - Remove pub fn watch_dir() (~35 lines) — only called from run_website()                                                                                       │
     │                                                                                                                                                                │
     │ Modify calepin/src/plugins.rs                                                                                                                                  │
     │                                                                                                                                                                │
     │ - Remove has_build_site field from PluginHandle                                                                                                                │
     │ - Remove call_build_site() method                                                                                                                              │
     │ - Remove has_build_site detection in load_one()                                                                                                                │
     │                                                                                                                                                                │
     │ Modify Makefile                                                                                                                                                │
     │                                                                                                                                                                │
     │ - Remove website target                                                                                                                                        │
     │                                                                                                                                                                │
     │ No dependency changes needed                                                                                                                                   │
     │                                                                                                                                                                │
     │ All preview deps (notify, tiny_http, open, ctrlc, indicatif) are still used for single-file preview.                                                           │
     │                                                                                                                                                                │
     │ ---                                                                                                                                                            │
     │ Phase 2: Batch rendering                                                                                                                                       │
     │                                                                                                                                                                │
     │ CLI interface (cli.rs)                                                                                                                                         │
     │                                                                                                                                                                │
     │ calepin --batch manifest.json          # render all, write files, metadata on stdout                                                                           │
     │ calepin --batch manifest.json --batch-stdout   # render all, bodies + metadata on stdout                                                                       │
     │ calepin --batch -                       # read manifest from stdin                                                                                             │
     │                                                                                                                                                                │
     │ Add two fields to the Cli struct:                                                                                                                              │
     │ #[arg(long, value_name = "MANIFEST")]                                                                                                                          │
     │ pub batch: Option<String>,                                                                                                                                     │
     │                                                                                                                                                                │
     │ #[arg(long)]                                                                                                                                                   │
     │ pub batch_stdout: bool,                                                                                                                                        │
     │                                                                                                                                                                │
     │ JSON input (manifest)                                                                                                                                          │
     │                                                                                                                                                                │
     │ [                                                                                                                                                              │
     │   {"input": "index.qmd", "output": "_site/index.html", "format": "html", "overrides": ["toc=false"]},                                                          │
     │   {"input": "basics.qmd", "output": "_site/basics.html"}                                                                                                       │
     │ ]                                                                                                                                                              │
     │                                                                                                                                                                │
     │ Only input is required. output defaults to input with format extension. format defaults to metadata or "html". overrides defaults to [].                       │
     │                                                                                                                                                                │
     │ JSON output (stdout)                                                                                                                                           │
     │                                                                                                                                                                │
     │ [                                                                                                                                                              │
     │   {"input": "index.qmd", "output": "_site/index.html", "status": "ok",                                                                                         │
     │    "title": "Home", "date": "2025-01-01"},                                                                                                                     │
     │   {"input": "basics.qmd", "output": "_site/basics.html", "status": "ok",                                                                                       │
     │    "title": "Getting Started", "body": "<p>...</p>"}                                                                                                           │
     │ ]                                                                                                                                                              │
     │                                                                                                                                                                │
     │ - body is only present when --batch-stdout is used (and files are NOT written)                                                                                 │
     │ - On error: "status": "error", "error": "message"                                                                                                              │
     │ - Metadata fields: title, date, subtitle, abstract — enough for a site builder to construct navigation/indexes without re-parsing YAML                         │
     │                                                                                                                                                                │
     │ Parallelism                                                                                                                                                    │
     │                                                                                                                                                                │
     │ Use std::thread::scope (stable since Rust 1.63, no new dependency). Each thread calls render_file() independently — each owns its own R/Python sessions,       │
     │ ElementRenderer, plugins. No shared mutable state.                                                                                                             │
     │                                                                                                                                                                │
     │ let results: Vec<BatchResult> = std::thread::scope(|s| {                                                                                                       │
     │     let handles: Vec<_> = jobs.iter().map(|job| {                                                                                                              │
     │         s.spawn(|| render_one_job(job, write_files))                                                                                                           │
     │     }).collect();                                                                                                                                              │
     │     handles.into_iter().map(|h| h.join().unwrap()).collect()                                                                                                   │
     │ });                                                                                                                                                            │
     │                                                                                                                                                                │
     │ This is safe because:                                                                                                                                          │
     │ - render_core() creates all mutable state locally (sessions, ElementRenderer with RefCells, plugins)                                                           │
     │ - comrak, syntect, hayagriva are all thread-safe                                                                                                               │
     │ - Each R/Python session is its own subprocess — fully independent per thread                                                                                   │
     │ - File I/O (writing outputs) doesn't conflict since each job has a distinct output path                                                                        │
     │                                                                                                                                                                │
     │ No session reuse across files — parallelism is the performance win, not session sharing. For a 20-page site with no code, all pages render simultaneously. For │
     │  pages with R/Python code, each thread pays session startup cost independently, but they run in parallel.                                                      │
     │                                                                                                                                                                │
     │ New module: calepin/src/batch.rs                                                                                                                               │
     │                                                                                                                                                                │
     │ use serde::{Deserialize, Serialize};                                                                                                                           │
     │                                                                                                                                                                │
     │ #[derive(Deserialize)]                                                                                                                                         │
     │ pub struct BatchJob {                                                                                                                                          │
     │     pub input: String,                                                                                                                                         │
     │     pub output: Option<String>,                                                                                                                                │
     │     pub format: Option<String>,                                                                                                                                │
     │     #[serde(default)]                                                                                                                                          │
     │     pub overrides: Vec<String>,                                                                                                                                │
     │ }                                                                                                                                                              │
     │                                                                                                                                                                │
     │ #[derive(Serialize)]                                                                                                                                           │
     │ pub struct BatchResult {                                                                                                                                       │
     │     pub input: String,                                                                                                                                         │
     │     pub output: String,                                                                                                                                        │
     │     pub status: String,   // "ok" or "error"                                                                                                                   │
     │     #[serde(skip_serializing_if = "Option::is_none")]                                                                                                          │
     │     pub title: Option<String>,                                                                                                                                 │
     │     #[serde(skip_serializing_if = "Option::is_none")]                                                                                                          │
     │     pub date: Option<String>,                                                                                                                                  │
     │     #[serde(skip_serializing_if = "Option::is_none")]                                                                                                          │
     │     pub subtitle: Option<String>,                                                                                                                              │
     │     #[serde(skip_serializing_if = "Option::is_none")]                                                                                                          │
     │     pub r#abstract: Option<String>,                                                                                                                            │
     │     #[serde(skip_serializing_if = "Option::is_none")]                                                                                                          │
     │     pub body: Option<String>,                                                                                                                                  │
     │     #[serde(skip_serializing_if = "Option::is_none")]                                                                                                          │
     │     pub error: Option<String>,                                                                                                                                 │
     │ }                                                                                                                                                              │
     │                                                                                                                                                                │
     │ pub fn run_batch(manifest_source: &str, write_files: bool, quiet: bool) -> Result<()>;                                                                         │
     │                                                                                                                                                                │
     │ run_batch():                                                                                                                                                   │
     │ 1. Read manifest JSON (from file, or stdin if "-")                                                                                                             │
     │ 2. Deserialize Vec<BatchJob>                                                                                                                                   │
     │ 3. Spawn parallel threads via std::thread::scope                                                                                                               │
     │ 4. Each thread: resolve output path, call render_file(), extract metadata from result, optionally write file                                                   │
     │ 5. Collect Vec<BatchResult>, serialize to stdout as JSON                                                                                                       │
     │                                                                                                                                                                │
     │ Integration in main.rs                                                                                                                                         │
     │                                                                                                                                                                │
     │ After CLI parsing and early returns (completions, highlight-styles), before single-file rendering:                                                             │
     │                                                                                                                                                                │
     │ if let Some(ref manifest) = cli.batch {                                                                                                                        │
     │     return batch::run_batch(manifest, !cli.batch_stdout, cli.quiet);                                                                                           │
     │ }                                                                                                                                                              │
     │                                                                                                                                                                │
     │ Add mod batch; to module declarations.                                                                                                                         │
     │                                                                                                                                                                │
     │ ---                                                                                                                                                            │
     │ Phase 3: Plugin interface improvements                                                                                                                         │
     │                                                                                                                                                                │
     │ 3a: Script-based postprocess for custom formats                                                                                                                │
     │                                                                                                                                                                │
     │ In _calepin/formats/slides.yaml:                                                                                                                               │
     │ base: html                                                                                                                                                     │
     │ extension: html                                                                                                                                                │
     │ postprocess: ./split-slides.py                                                                                                                                 │
     │                                                                                                                                                                │
     │ The script receives the complete rendered document (after page template) on stdin. It writes the transformed document to stdout. The format name is passed as  │
     │ argv[1].                                                                                                                                                       │
     │                                                                                                                                                                │
     │ Implementation in formats/mod.rs                                                                                                                               │
     │                                                                                                                                                                │
     │ - Add postprocess_script: Option<PathBuf> field to CustomRenderer                                                                                              │
     │ - In load_custom_format(): parse config["postprocess"], resolve relative to the YAML file's directory                                                          │
     │ - In CustomRenderer::apply_template(): after getting the base template output, if postprocess_script is set, pipe the output through the script                │
     │                                                                                                                                                                │
     │ Priority order:                                                                                                                                                │
     │ 1. WASM plugin postprocess (replaces template entirely — existing behavior)                                                                                    │
     │ 2. Base format template (produces complete document)                                                                                                           │
     │ 3. Script postprocess (transforms the complete document)                                                                                                       │
     │                                                                                                                                                                │
     │ 3b: Per-document preprocess hook                                                                                                                               │
     │                                                                                                                                                                │
     │ In _calepin/formats/obsidian.yaml:                                                                                                                             │
     │ base: html                                                                                                                                                     │
     │ preprocess: ./expand-wikilinks.py                                                                                                                              │
     │                                                                                                                                                                │
     │ The script receives JSON on stdin:                                                                                                                             │
     │ {                                                                                                                                                              │
     │   "body": "raw .qmd text (after include expansion)",                                                                                                           │
     │   "format": "obsidian"                                                                                                                                         │
     │ }                                                                                                                                                              │
     │                                                                                                                                                                │
     │ It writes the transformed body text on stdout (plain text, not JSON).                                                                                          │
     │                                                                                                                                                                │
     │ Implementation                                                                                                                                                 │
     │                                                                                                                                                                │
     │ - Add preprocess_script: Option<PathBuf> field to CustomRenderer                                                                                               │
     │ - Add fn preprocess(&self) -> Option<&Path> { None } method to OutputRenderer trait; override in CustomRenderer                                                │
     │ - In render_core() (main.rs), between expand_includes and parse_body: check renderer.preprocess(), if Some, pipe body through the script                       │
     │                                                                                                                                                                │
     │ Shared script runner                                                                                                                                           │
     │                                                                                                                                                                │
     │ Both preprocess and postprocess use the same pattern: spawn a subprocess, write to stdin, read stdout. Add a helper function (in formats/mod.rs or a small     │
     │ utility):                                                                                                                                                      │
     │                                                                                                                                                                │
     │ fn run_script(script: &Path, stdin_data: &str, args: &[&str]) -> Result<String> {                                                                              │
     │     let mut child = Command::new(script)                                                                                                                       │
     │         .args(args)                                                                                                                                            │
     │         .stdin(Stdio::piped())                                                                                                                                 │
     │         .stdout(Stdio::piped())                                                                                                                                │
     │         .stderr(Stdio::piped())                                                                                                                                │
     │         .spawn()?;                                                                                                                                             │
     │     child.stdin.take().unwrap().write_all(stdin_data.as_bytes())?;                                                                                             │
     │     let output = child.wait_with_output()?;                                                                                                                    │
     │     if !output.status.success() {                                                                                                                              │
     │         let stderr = String::from_utf8_lossy(&output.stderr);                                                                                                  │
     │         bail!("Script {} failed: {}", script.display(), stderr);                                                                                               │
     │     }                                                                                                                                                          │
     │     Ok(String::from_utf8(output.stdout)?)                                                                                                                      │
     │ }                                                                                                                                                              │
     │                                                                                                                                                                │
     │ ---                                                                                                                                                            │
     │ Phase 4: Tests                                                                                                                                                 │
     │                                                                                                                                                                │
     │ 1. Batch JSON round-trip — deserialize manifest, serialize results                                                                                             │
     │ 2. Batch rendering — two simple .qmd files (no R/Python), verify both outputs correct                                                                          │
     │ 3. Batch stdout mode — verify JSON output contains bodies, no files written                                                                                    │
     │ 4. Batch error handling — one valid + one nonexistent file, verify partial success                                                                             │
     │ 5. Script postprocess — custom format with a postprocess script, verify transformation applied                                                                 │
     │ 6. Script preprocess — custom format with a preprocess script, verify body transformed before parsing                                                          │
     │ 7. Site builder removal regression — verify .yaml input produces a clear error                                                                                 │
     │                                                                                                                                                                │
     │ ---                                                                                                                                                            │
     │ Phase 5: Update CLAUDE.md                                                                                                                                      │
     │                                                                                                                                                                │
     │ - Remove website.rs, site_builder.rs, plugins/astro/ from module layout                                                                                        │
     │ - Remove build_site from plugin capabilities                                                                                                                   │
     │ - Add batch.rs to module layout                                                                                                                                │
     │ - Document --batch CLI flag                                                                                                                                    │
     │ - Document preprocess/postprocess in custom format section                                                                                                     │
     │ - Remove make website from build commands                                                                                                                      │
     │                                                                                                                                                                │
     │ ---                                                                                                                                                            │
     │ Implementation order                                                                                                                                           │
     │                                                                                                                                                                │
     │ 1. Phase 1 (site builder removal) — pure deletion, reduces codebase                                                                                            │
     │ 2. Phase 2 (batch rendering) — new batch.rs module + CLI flags                                                                                                 │
     │ 3. Phase 3 (plugin improvements) — preprocess/postprocess in formats/mod.rs                                                                                    │
     │ 4. Phase 4 (tests) — alongside each phase                                                                                                                      │
     │ 5. Phase 5 (docs) — after APIs are stable                                                                                                                      │
     │                                                                                                                                                                │
     │ Phases 1 and 3 are independent. Phase 2 is the largest piece.                                                                                                  │
     │                                                                                                                                                                │
     │ Key files                                                                                                                                                      │
     │                                                                                                                                                                │
     │ ┌────────────────────────────────┬────────────────────────────────────────────────────────────────────────────────┐                                            │
     │ │              File              │                                    Changes                                     │                                            │
     │ ├────────────────────────────────┼────────────────────────────────────────────────────────────────────────────────┤                                            │
     │ │ calepin/src/main.rs            │ Remove site builder, add batch entry point, add preprocess hook in render_core │                                            │
     │ ├────────────────────────────────┼────────────────────────────────────────────────────────────────────────────────┤                                            │
     │ │ calepin/src/batch.rs           │ New — batch types + parallel runner                                            │                                            │
     │ ├────────────────────────────────┼────────────────────────────────────────────────────────────────────────────────┤                                            │
     │ │ calepin/src/cli.rs             │ Add --batch and --batch-stdout flags                                           │                                            │
     │ ├────────────────────────────────┼────────────────────────────────────────────────────────────────────────────────┤                                            │
     │ │ calepin/src/formats/mod.rs     │ Add preprocess/postprocess script support to CustomRenderer                    │                                            │
     │ ├────────────────────────────────┼────────────────────────────────────────────────────────────────────────────────┤                                            │
     │ │ calepin/src/plugins.rs         │ Remove build_site capability                                                   │                                            │
     │ ├────────────────────────────────┼────────────────────────────────────────────────────────────────────────────────┤                                            │
     │ │ calepin/src/preview/mod.rs     │ Remove website preview + npm functions                                         │                                            │
     │ ├────────────────────────────────┼────────────────────────────────────────────────────────────────────────────────┤                                            │
     │ │ calepin/src/preview/server.rs  │ Remove start_site()                                                            │                                            │
     │ ├────────────────────────────────┼────────────────────────────────────────────────────────────────────────────────┤                                            │
     │ │ calepin/src/preview/watcher.rs │ Remove watch_dir()                                                             │                                            │
     │ ├────────────────────────────────┼────────────────────────────────────────────────────────────────────────────────┤                                            │
     │ │ calepin/src/website.rs         │ Delete                                                                         │                                            │
     │ ├────────────────────────────────┼────────────────────────────────────────────────────────────────────────────────┤                                            │
     │ │ calepin/src/site_builder.rs    │ Delete                                                                         │                                            │
     │ ├────────────────────────────────┼────────────────────────────────────────────────────────────────────────────────┤                                            │
     │ │ plugins/astro/                 │ Delete (entire crate)                                                          │                                            │
     │ └────────────────────────────────┴────────────────────────────────────────────────────────────────────────────────┘                                            │
     │                                                                                                                                                                │
     │ Verification                                                                                                                                                   │
     │                                                                                                                                                                │
     │ 1. make check — compiles after site builder removal                                                                                                            │
     │ 2. make test — existing tests pass                                                                                                                             │
     │ 3. Manual: echo '[{"input":"website/basics.qmd"}]' | calepin --batch - — verify parallel rendering works                                                       │
     │ 4. Manual: create a custom format with preprocess/postprocess scripts, render a file                                                                           │
     │ 5. Manual: calepin some_file.yaml — verify clear error (not silent site build)                                                                                 │
     ╰────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯

