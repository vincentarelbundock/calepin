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
use std::path::Path;

use super::subprocess::SubprocessSession;
use super::{build_payload, make_sentinel};
use crate::utils::process;
use crate::utils::tools;

/// Bootstrap Python script sent once at startup.
/// Sets up a read-eval loop that reads sentinel-delimited code blocks from stdin,
/// executes them with output/warning/error/plot capture, and writes
/// sentinel-delimited results to stdout.
const PYTHON_BOOTSTRAP: &str = r#"
import sys, io, os, json

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

    # Parse metadata: JSON preferred, semicolon format as fallback for legacy callers.
    meta = {}
    if meta_line.startswith("META:"):
        raw_meta = meta_line[5:]
        if raw_meta:
            try:
                meta = json.loads(raw_meta)
            except Exception:
                meta = dict(item.split("=", 1) for item in raw_meta.split(";") if "=" in item)

    fig_path = meta.get("fig_path", "")
    width = float(meta.get("width", "7"))
    height = float(meta.get("height", "5"))
    dpi = float(meta.get("dpi", "150"))

    sep = sentinel + "_SEP"
    parts = []
    if fig_path and os.path.exists(fig_path):
        try:
            os.remove(fig_path)
        except OSError as remove_err:
            parts.append(f"{sentinel}_WARNING:Failed to remove previous figure: {remove_err}")
    err = None
    warn_records = []
    old_stdout = sys.stdout
    old_stderr = sys.stderr

    try:
        import warnings, ast as _ast
        last_expr_result = None

        def _calepin_is_matplotlib_figure(value):
            if not fig_path:
                return False
            try:
                from matplotlib.figure import Figure
            except Exception:
                return False
            return isinstance(value, Figure)

        def _calepin_is_matplotlib_display_value(value, seen=None):
            if not fig_path or value is None:
                return False
            if seen is None:
                seen = set()
            value_id = id(value)
            if value_id in seen:
                return False
            seen.add(value_id)
            module = type(value).__module__
            if module == "matplotlib" or module.startswith("matplotlib."):
                return True
            if isinstance(value, (list, tuple, set, frozenset)):
                return any(_calepin_is_matplotlib_display_value(item, seen) for item in value)
            if isinstance(value, dict):
                return any(_calepin_is_matplotlib_display_value(item, seen) for item in value.values())
            return False

        def _calepin_should_print_expr_result(value):
            return value is not None and not _calepin_is_matplotlib_display_value(value)

        with warnings.catch_warnings(record=True) as warn_records:
            warnings.simplefilter("always")
            if "matplotlib.pyplot" in sys.modules:
                sys.modules["matplotlib.pyplot"].show = lambda *a, **k: None

            tree = _ast.parse(code, "<chunk>")
            code_lines = code.split("\n")
            prev_end = 0
            src_buf = []

            for node in tree.body:
                # Accumulate source lines (include gap: comments, blanks)
                end_line = node.end_lineno
                src_buf.extend(code_lines[prev_end:end_line])
                prev_end = end_line

                # Capture stdout per-statement
                out_buf = io.StringIO()
                err_buf = io.StringIO()
                sys.stdout = out_buf
                sys.stderr = err_buf
                try:
                    if isinstance(node, _ast.Expr):
                        expr_code = compile(_ast.Expression(body=node.value), "<chunk>", "eval")
                        result = eval(expr_code, _globals)
                        last_expr_result = result
                        if _calepin_should_print_expr_result(result):
                            print(repr(result))
                    else:
                        mod = _ast.Module(body=[node], type_ignores=[])
                        _ast.fix_missing_locations(mod)
                        stmt_code = compile(mod, "<chunk>", "exec")
                        exec(stmt_code, _globals)
                except Exception:
                    import traceback
                    err = traceback.format_exc()
                finally:
                    sys.stdout = old_stdout
                    sys.stderr = old_stderr

                output = out_buf.getvalue().rstrip("\n")
                if output:
                    # Flush accumulated source before output
                    parts.append(f"{sentinel}_SOURCE:" + "\n".join(src_buf))
                    src_buf = []
                    parts.append(f"{sentinel}_OUTPUT:{output}")

                diagnostics = err_buf.getvalue().rstrip("\n")
                if diagnostics:
                    parts.append(f"{sentinel}_WARNING:{diagnostics}")

                if err:
                    break

            # Flush remaining source (trailing statements + comments)
            remaining = src_buf + code_lines[prev_end:] if prev_end < len(code_lines) else src_buf
            if not err and remaining and "\n".join(remaining).strip():
                parts.append(f"{sentinel}_SOURCE:" + "\n".join(remaining))
    except Exception:
        sys.stdout = old_stdout
        sys.stderr = old_stderr
        import traceback
        err = traceback.format_exc()

    warns_list = [str(x.message) for x in warn_records]

    # Check for matplotlib figures. The import is a no-op if matplotlib is
    # already loaded (cached in sys.modules), and a cheap ImportError if not
    # installed. Only runs when fig_path is set (i.e., not a table chunk).
    # bbox_inches="tight" recomputes layout, so set_size_inches after user
    # code is fine even if user called tight_layout().
    #
    def _calepin_plot_path(base, index):
        if index <= 1:
            return base
        root, ext = os.path.splitext(base)
        if root.endswith("-1"):
            root = root[:-2]
        return f"{root}-{index}{ext}"

    has_plot = False
    if fig_path:
        try:
            import matplotlib.pyplot as plt
            figs = []
            seen_figs = set()
            if _calepin_is_matplotlib_figure(last_expr_result):
                figs.append(last_expr_result)
                seen_figs.add(id(last_expr_result))
            for num in plt.get_fignums():
                fig = plt.figure(num) if hasattr(plt, "figure") else plt.gcf()
                if id(fig) not in seen_figs:
                    figs.append(fig)
                    seen_figs.add(id(fig))
            for index, fig in enumerate(figs, start=1):
                fig.set_size_inches(width, height)
                path = _calepin_plot_path(fig_path, index)
                saved_plot = False
                try:
                    fig.savefig(path, dpi=dpi, bbox_inches="tight")
                    saved_plot = True
                except Exception as save_err:
                    parts.append(f"{sentinel}_WARNING:Failed to save figure: {save_err}")
                if saved_plot and os.path.exists(path) and os.path.getsize(path) > 0:
                    has_plot = True
                    parts.append(f"{sentinel}_PLOT:{path}")
            plt.close("all")
        except ImportError:
            pass
        except Exception as plot_err:
            parts.append(f"{sentinel}_WARNING:Failed to capture figure: {plot_err}")

    if err:
        parts.append(f"{sentinel}_ERROR:{err}")

    for ww in warns_list:
        parts.append(f"{sentinel}_WARNING:{ww}")

    result = ("\n" + sep + "\n").join(parts)
    print(result, flush=True)
    print(f"{sentinel}_DONE", flush=True)
"#;

/// RAII guard for the Python subprocess.
pub struct PythonSession {
    proc: SubprocessSession,
}

impl PythonSession {
    pub fn init_with_program(
        program: &Path,
        cwd: Option<&Path>,
        timeout: Option<std::time::Duration>,
    ) -> Result<Self> {
        process::validate_python_interpreter(program, "start Python", Some(&tools::PYTHON))
            .context("Failed to start Python")?;
        let proc = SubprocessSession::spawn(
            program,
            &["-s", "-u", "-c", PYTHON_BOOTSTRAP],
            &[("PYTHONDONTWRITEBYTECODE", "1"), ("PYTHONNOUSERSITE", "1")],
            cwd,
            timeout,
            Some(&tools::PYTHON),
        )
        .context("Failed to start Python")?;
        Ok(PythonSession { proc })
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
        let payload = build_payload(
            serde_json::json!({
                "fig_path": fig_path,
                "dev": "",
                "width": width,
                "height": height,
                "dpi": dpi,
            }),
            code,
        )?;
        self.proc.execute(&sentinel, &payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use crate::utils::testutil::command_available;

    fn session() -> PythonSession {
        PythonSession::init_with_program(Path::new("python3"), None, Some(Duration::from_secs(10)))
            .unwrap()
    }

    #[test]
    fn python_session_captures_stderr_as_warning() {
        if !command_available("python3") {
            return;
        }

        let mut session = session();
        let raw = session
            .capture(
                "import sys\nprint('stderr text', file=sys.stderr)",
                "",
                6.0,
                3.708,
                150.0,
            )
            .unwrap();

        assert!(raw.contains("_WARNING:stderr text"), "{raw}");
    }

    #[test]
    fn python_session_preserves_warnings_before_errors() {
        if !command_available("python3") {
            return;
        }

        let mut session = session();
        let raw = session
            .capture(
                "import warnings\nwarnings.warn('careful')\nraise ValueError('boom')",
                "",
                6.0,
                3.708,
                150.0,
            )
            .unwrap();

        assert!(raw.contains("_ERROR:"), "{raw}");
        assert!(raw.contains("ValueError: boom"), "{raw}");
        assert!(raw.contains("_WARNING:careful"), "{raw}");
    }

    #[test]
    fn python_session_removes_stale_figure_file() {
        if !command_available("python3") {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let fig_path = dir.path().join("stale.svg");
        std::fs::write(&fig_path, "<svg>old</svg>").unwrap();
        let fig_path = fig_path.to_string_lossy().replace('\\', "/");

        let mut session = session();
        let raw = session
            .capture("print('fresh')", &fig_path, 6.0, 3.708, 150.0)
            .unwrap();

        assert!(raw.contains("_OUTPUT:fresh"), "{raw}");
        assert!(!raw.contains("_PLOT:"), "{raw}");
        assert!(!std::path::Path::new(&fig_path).exists());
    }

    #[test]
    fn python_session_suppresses_matplotlib_artist_repr() {
        if !command_available("python3") {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let fig_path = dir.path().join("plot.svg");
        let fig_path = fig_path.to_string_lossy().replace('\\', "/");
        let mut session = session();
        let raw = session
            .capture(
                r#"import sys, types

matplotlib = types.ModuleType("matplotlib")
figure_mod = types.ModuleType("matplotlib.figure")
artist_mod = types.ModuleType("matplotlib.artist")
axes_mod = types.ModuleType("matplotlib.axes")
colorbar_mod = types.ModuleType("matplotlib.colorbar")
pyplot_mod = types.ModuleType("matplotlib.pyplot")

class Figure:
    def set_size_inches(self, width, height):
        self.size = (width, height)
    def savefig(self, path, dpi=None, bbox_inches=None):
        with open(path, "w") as handle:
            handle.write("<svg><path d='M0 0L1 1'/></svg>")

class Artist:
    pass

class Axes:
    pass

class Colorbar:
    pass

class Line2D(Artist):
    pass

Figure.__module__ = "matplotlib.figure"
Artist.__module__ = "matplotlib.artist"
Axes.__module__ = "matplotlib.axes"
Colorbar.__module__ = "matplotlib.colorbar"
Line2D.__module__ = "matplotlib.lines"

_fig = Figure()

def plot(values):
    return [Line2D()]

def colorbar():
    return Colorbar()

def get_fignums():
    return [1]

def gcf():
    return _fig

def close(target=None):
    pass

pyplot_mod.plot = plot
pyplot_mod.colorbar = colorbar
pyplot_mod.get_fignums = get_fignums
pyplot_mod.gcf = gcf
pyplot_mod.close = close
figure_mod.Figure = Figure
artist_mod.Artist = Artist
axes_mod.Axes = Axes
colorbar_mod.Colorbar = Colorbar
matplotlib.figure = figure_mod
matplotlib.artist = artist_mod
matplotlib.axes = axes_mod
matplotlib.colorbar = colorbar_mod
matplotlib.pyplot = pyplot_mod
sys.modules["matplotlib"] = matplotlib
sys.modules["matplotlib.figure"] = figure_mod
sys.modules["matplotlib.artist"] = artist_mod
sys.modules["matplotlib.axes"] = axes_mod
sys.modules["matplotlib.colorbar"] = colorbar_mod
sys.modules["matplotlib.pyplot"] = pyplot_mod

import matplotlib.pyplot as plt
plt.plot([1, 2, 3])
plt.colorbar()"#,
                &fig_path,
                6.0,
                3.708,
                150.0,
            )
            .unwrap();

        assert!(raw.contains("_PLOT:"), "{raw}");
        assert!(std::path::Path::new(&fig_path).exists());
        assert!(!raw.contains("_OUTPUT:"), "{raw}");
    }
}
