// Python engine session via a persistent python3 subprocess.
//
// ## Design
//
// A single python3 process runs for the lifetime of the document render. On init,
// the bootstrap script is piped directly to stdin (`python3 -S -s -u -`). It sets up
// a read-eval loop over stdin/stdout using a sentinel-delimited protocol (see
// subprocess.rs). All chunks execute in a shared `_globals` dict, so variables
// persist across chunks — notebook semantics by design.
//
// Two execution modes:
// - **Block** (`capture`): exec() the code, capturing stdout, warnings, errors,
//   and matplotlib figures. The bootstrap redirects stdout to a StringIO buffer,
//   records warnings via the warnings module, and checks for open matplotlib
//   figures after each chunk.
// - **Inline** (`evaluate_inline`): eval() a single expression and return its
//   string representation. Used for `{python} expr` in body text.
//
// Matplotlib is set to the non-interactive Agg backend at startup. After each
// block execution, any open figures are saved to the requested path and closed.
// Only the current figure (`gcf()`) is saved — multiple figures per chunk are
// not yet supported.
//
// ## Functions
//
// - PythonSession::init()            — Spawn python3 with the bootstrap read-eval loop.
// - PythonSession::evaluate_inline() — Evaluate a single Python expression and return the result.
// - PythonSession::capture()         — Execute a Python code chunk with output/warning/error/plot
//                                      capture using the sentinel protocol.

use anyhow::{Context, Result};

use super::make_sentinel;
use super::subprocess::SubprocessSession;

/// Bootstrap Python script sent once at startup.
/// Sets up a read-eval loop that reads sentinel-delimited code blocks from stdin,
/// executes them with output/warning/error/plot capture, and writes
/// sentinel-delimited results to stdout.
const PYTHON_BOOTSTRAP: &str = r#"
import sys, io, os

# Force non-interactive matplotlib backend before any user code can import it.
# The env var is checked by matplotlib on first import -- no import needed here,
# so there's zero startup cost when no chunks use matplotlib.
os.environ["MPLBACKEND"] = "Agg"

# Shared globals dict gives notebook-style variable persistence across chunks.
# Note: objects accumulate here for the lifetime of the subprocess (by design).
# Requires Python 3.9+ for str.removesuffix().
_globals = {"__builtins__": __import__("builtins")}

while True:
    header = sys.stdin.readline()
    if not header:
        break
    sentinel = header.strip().removesuffix("_BEGIN")
    end_marker = sentinel + "_END"

    lines = []
    while True:
        line = sys.stdin.readline()
        if not line or line.strip() == end_marker:
            break
        lines.append(line)

    # First line is metadata, rest is code
    meta_line = lines[0].strip() if lines else ""
    code = "".join(lines[1:])

    if meta_line.startswith("INLINE:"):
        expr = meta_line[len("INLINE:"):]
        try:
            result = eval(compile(expr, "<inline>", "eval"), _globals)
            print(str(result), flush=True)
        except Exception as e:
            print(f"{sentinel}_ERROR:{e}", flush=True)
        print(f"{sentinel}_DONE", flush=True)
        continue

    # Parse metadata: "META:fig_path=/tmp/fig.png;width=7;height=5;dpi=150"
    meta = dict(item.split("=", 1) for item in meta_line[5:].split(";") if "=" in item) if meta_line.startswith("META:") else {}

    fig_path = meta.get("fig_path", "")
    width = float(meta.get("width", "7"))
    height = float(meta.get("height", "5"))
    dpi = float(meta.get("dpi", "150"))

    sep = sentinel + "_SEP"
    parts = []

    buf = io.StringIO()
    err = None
    warns_list = []

    # Capture stdout via temporary swap (avoids importing contextlib at startup).
    # warnings/traceback imported lazily on first use.
    old_stdout = sys.stdout
    sys.stdout = buf
    try:
        import warnings
        with warnings.catch_warnings(record=True) as w:
            warnings.simplefilter("always")
            if "matplotlib.pyplot" in sys.modules:
                sys.modules["matplotlib.pyplot"].show = lambda *a, **k: None
            exec(compile(code, "<chunk>", "exec"), _globals)
            warns_list = [str(x.message) for x in w]
    except Exception:
        import traceback
        err = traceback.format_exc()
    finally:
        sys.stdout = old_stdout

    # Check for matplotlib figures. The import is a no-op if matplotlib is
    # already loaded (cached in sys.modules), and a cheap ImportError if not
    # installed. Only runs when fig_path is set (i.e., not a table chunk).
    # bbox_inches="tight" recomputes layout, so set_size_inches after user
    # code is fine even if user called tight_layout().
    #
    # Only the current figure (gcf) is saved. If a chunk creates multiple
    # figures, earlier ones are discarded. This matches R's behavior where
    # only the last plot device output is captured per chunk.
    has_plot = False
    if fig_path:
        try:
            import matplotlib.pyplot as plt
            if plt.get_fignums():
                fig = plt.gcf()
                fig.set_size_inches(width, height)
                try:
                    fig.savefig(fig_path, dpi=dpi, bbox_inches="tight")
                except Exception as save_err:
                    parts.append(f"{sentinel}_WARNING:Failed to save figure: {save_err}")
                plt.close("all")
                if os.path.exists(fig_path) and os.path.getsize(fig_path) > 0:
                    has_plot = True
        except ImportError:
            pass

    output = buf.getvalue().rstrip("\n")

    if err:
        parts.append(f"{sentinel}_ERROR:{err}")
    elif output:
        parts.append(f"{sentinel}_OUTPUT:{output}")

    for ww in warns_list:
        parts.append(f"{sentinel}_WARNING:{ww}")

    if has_plot:
        parts.append(f"{sentinel}_PLOT:{fig_path}")

    result = ("\n" + sep + "\n").join(parts)
    print(result, flush=True)
    print(f"{sentinel}_DONE", flush=True)
"#;

/// RAII guard for the Python subprocess.
pub struct PythonSession {
    proc: SubprocessSession,
}

impl PythonSession {
    /// Spawn a python3 subprocess running the bootstrap script.
    /// Uses -s (no user site), -u (unbuffered stdio),
    /// and passes the bootstrap as a -c argument (no temp file needed).
    pub fn init() -> Result<Self> {
        let proc = SubprocessSession::spawn_with_env(
            "python3",
            // No -S here: site-packages must be importable for `uv run calepin` to work.
            &["-s", "-u", "-c", PYTHON_BOOTSTRAP],
            &[("PYTHONDONTWRITEBYTECODE", "1"), ("PYTHONNOUSERSITE", "1")],
        )
        .context("Failed to start Python")?;
        Ok(PythonSession { proc })
    }

    /// Evaluate an inline Python expression and return the result as a string.
    pub fn evaluate_inline(&mut self, expr: &str) -> Result<String> {
        let sentinel = make_sentinel();
        let payload = format!("INLINE:{}", expr);
        let raw = self.proc.execute(&sentinel, &payload)?;
        let (_, result) = raw.split_once('\n').unwrap_or(("", ""));
        Ok(result.to_string())
    }

    /// Capture Python code output using the sentinel protocol.
    pub fn capture(
        &mut self,
        code: &str,
        fig_path: &str,
        width: f64,
        height: f64,
        dpi: f64,
    ) -> Result<String> {
        let sentinel = make_sentinel();
        let meta = format!(
            "META:fig_path={};dev=;width={};height={};dpi={}",
            fig_path, width, height, dpi
        );
        let payload = format!("{}\n{}", meta, code);
        self.proc.execute(&sentinel, &payload)
    }
}
