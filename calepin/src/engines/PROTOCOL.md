# Engine protocol

Every engine (`r`, `python`, and every Jupyter-backed kernel such as `julia`
or `sh`/`bash`) implements the same contract: given a chunk's source and
figure options, produce the same shape of result regardless of language.
This document specifies that contract. `engines/mod.rs` (`EngineResult`,
`process_results`), `engines/subprocess.rs` (the transport), and each
engine's bootstrap script (`r.rs`, `python.rs`, `jupyter.rs`) implement it;
`calepin/tests/engine_conformance.rs` asserts it holds across engines.

## Sentinel framing

Each chunk execution is one request/response pair over the subprocess's
stdin/stdout, framed by a sentinel unique to that request (`make_sentinel()`:
process id + an atomic counter, so collisions with echoed user output are not
a practical concern).

Request (Rust -> subprocess):

```
{sentinel}_BEGIN
{meta}
{code line 1}
{code line 2}
...
{sentinel}_END
```

The first line after `_BEGIN` is metadata (a `META:` JSON object: engine,
figure path, format, dimensions, and so on); everything after it is the
chunk's source, sent verbatim.

Response (subprocess -> Rust): a sequence of tagged lines, each a "part":

```
{sentinel}_SOURCE:...
{sentinel}_SEP
{sentinel}_OUTPUT:...
{sentinel}_SEP
{sentinel}_PLOT:...
...
{sentinel}_DONE
```

Parts are separated by a line that is exactly `{sentinel}_SEP`; the reader
(`process_results` in `mod.rs`) splits on that exact line, not any substring
match, so a `_SEP`-looking string inside a part's own text does not truncate
it. The response ends with a line that is exactly `{sentinel}_DONE`
(`subprocess.rs` requires an exact match too, so a decoy such as
`..._DONE_DECOY` does not stop the read early).

Each part is one of:

- `_SOURCE:` (one or more source lines, joined with `\n`, echoed for display)
- `_OUTPUT:` (captured stdout, or a printed value)
- `_WARNING:` (a warning or direct-stderr write)
- `_MESSAGE:` (an R `message()` or other diagnostic distinct from a warning)
- `_ERROR:` (an uncaught error; the chunk's status becomes `Error` unless it
  tolerates errors)
- `_UNAVAILABLE:` (the requested engine/kernel could not be started at all,
  e.g. an uninstalled Jupyter kernel; the chunk is marked `Unavailable`, not
  `Error`)
- `_PLOT:` (a path to a figure file the engine already wrote to disk)

## Ordering within a chunk

A chunk is evaluated one top-level statement at a time (R: one parsed
expression; Python: one AST node; a Jupyter kernel: the whole cell, since a
kernel has no notion of "one statement"). Per statement, in order:

1. **Source** is flushed just before the first part that needs it. Statements
   that produce no output are batched into one `_SOURCE:` part (so blank or
   comment-only lines do not fragment the echo); the buffer flushes as soon
   as anything (output, a plot, or an error) needs to reference it.
2. **Output** (stdout, or a printed/auto-printed value) for that statement.
3. **Warnings/messages** produced while evaluating it.
4. **A plot**, if the statement caused one, is flushed *after* the source
   that drew it, never before. `plot(x)` followed by `cat("hi")` must
   produce `SOURCE, PLOT, SOURCE, OUTPUT`, not `PLOT, SOURCE, OUTPUT`: the
   figure is a consequence of code the reader has not been shown yet.

## Errors: the failing statement's source is never dropped

When a statement raises, the engine still flushes whatever source
accumulated up to and including the failing statement (and, in R and Python,
any trailing statements that never ran) as a `_SOURCE:` part, followed by
`_ERROR:`. An implementation must not gate "flush remaining source" on
success only: that drops the one line that explains the error.

## Timeout: one rule for every engine

There is exactly one timeout, owned by `SubprocessSession`
(`subprocess.rs`): unbounded by default, or whatever duration the `--timeout`
CLI flag set. No engine may invent its own fallback (a Jupyter-specific hard
30s ceiling is a bug, not a feature). On timeout the subprocess is asked to
exit gracefully first (SIGTERM on unix, so a bridge process can shut down
anything it manages, e.g. Jupyter kernels), then killed outright if it does
not exit within a short grace period. Either way the session is marked dead
and must be respawned for the next chunk, never reused (otherwise a session
that silently keeps "working" after being killed produces a confusing
"exited unexpectedly" on the *next* chunk instead of surfacing the timeout).

## fd-level capture

Ordinary output goes through the language's own stdout object (R's
`capture.output()`, Python's `sys.stdout` swap) and is captured reliably.
Anything that bypasses that layer and writes straight to file descriptor 1,
such as `subprocess.run([...])`, `os.system()`, R's `system()`, or a C
extension's `printf`, does not. Python additionally redirects fd 1 itself
(`os.dup2`) for the duration of each statement, merging that capture into
the same `_OUTPUT:` part. Where fd-level redirection is not implemented (R's
`system()`, whose child inherits the real fd 1 with no portable way for R
alone to retarget it), the transport still degrades safely: `process_results`
treats text that arrives ahead of the first tagged part as engine output
instead of silently discarding the whole part.

## Figures: `fig-` vs `tbl-` chunks

A chunk's figure path is derived from its label. A `fig-*` label gets a real
path and any plot is saved there (see the `-N` rule below for multiple plots
per chunk). A `tbl-*` chunk gets an *empty* figure path, but the engine must
still not let a stray `plot()`/`plt.plot()` call there leak into the graphics
state of the next chunk that does have a figure path:

- Python always runs `plt.close("all")` after every chunk, regardless of
  whether `fig_path` was set.
- R always opens a real graphics device, even with no figure path (targeting
  a throwaway file under `tempdir()`), so a stray `plot()` call is captured
  and discarded instead of falling through to R's default device, which
  otherwise opens `Rplots.pdf` in the project directory and never closes it.

## The `-N` figure suffix rule

A chunk may produce more than one figure. The first keeps the base path Rust
computed (`<label>-1.<ext>`); later ones get `-N` substituted for the
trailing `-1`. Defined once per language runtime, not re-derived per call
site: `PY_PLOT_INDEX_HELPER` in `mod.rs` is spliced into both the Python
engine bootstrap and the Jupyter bridge bootstrap (both are Python, so one
definition serves both); R keeps its own R-language implementation
(`.calepin_plot_path` in `r.rs`) since it cannot share Python source.

## META encoding

Metadata values that can contain the `META:` line's delimiters (in
particular a figure path, which may contain `;` or `=`) must round-trip
exactly. Python and the Jupyter bridge send `META:` as JSON, handling this
for free. R's bootstrap predates JSON support in base R, so `r.rs`
percent-encodes only the ambiguous characters (`%`, `;`, `=`, `\n`, `\r`);
the R side decodes with `utils::URLdecode()`, which understands `%XX`.
