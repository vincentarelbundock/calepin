// Jupyter kernel bridge session via a persistent python3 subprocess.
//
// A single Python process manages one jupyter_client.KernelManager per
// named kernel (e.g. "octave", "ruby"). Code is sent via the sentinel
// protocol (see subprocess.rs and PROTOCOL.md); the bridge translates
// Jupyter message types to sentinel tags and writes them back.
//
// Timeout follows the same rule as every other engine: whatever duration
// SubprocessSession was given (unbounded by default) is what `wait_for_ready`
// and `get_iopub_msg` block on -- there is no separate hard-coded fallback
// here. On a Rust-side chunk timeout, subprocess.rs sends SIGTERM before
// SIGKILL; the bridge installs a SIGTERM handler that shuts down every
// kernel it started, so a slow-starting kernel does not get orphaned.

use anyhow::{Context, Result};
use std::path::Path;

use super::subprocess::SubprocessSession;
use super::{build_payload, make_sentinel, PLOT_PATH_HELPER_MARKER, PY_PLOT_INDEX_HELPER};
use crate::utils::process;
use crate::utils::tools;

const JUPYTER_BRIDGE_TEMPLATE: &str = r#"
import sys, base64, os, traceback, re, json, signal

try:
    from jupyter_client import KernelManager
    from jupyter_client.kernelspec import KernelSpecManager, NoSuchKernel
except ImportError:
    sys.stderr.write(
        "calepin: jupyter_client not found - "
        "install with: pip install jupyter_client\n"
    )
    sys.exit(1)

_managers = {}  # kernel_name -> (km, kc)
_resolved = {}  # requested name -> actual installed kernel name

def _shutdown_all():
    # Each step is independently guarded: a failure stopping channels must
    # not skip shutting down the kernel process itself, or vice versa.
    for km, kc in list(_managers.values()):
        try:
            kc.stop_channels()
        except Exception:
            pass
        try:
            km.shutdown_kernel(now=True)
        except Exception:
            pass
    _managers.clear()

def _handle_sigterm(signum, frame):
    # Give a graceful Rust-side timeout (subprocess.rs sends SIGTERM before
    # SIGKILL) a chance to shut every kernel this bridge started down cleanly,
    # instead of leaving them orphaned when the process is killed outright.
    _shutdown_all()
    sys.exit(0)

signal.signal(signal.SIGTERM, _handle_sigterm)

def _version_sort_key(spec):
    """Order version-suffixed kernel names numerically, not lexicographically.

    Plain sorting puts "julia-1.9" above "julia-1.11" because it compares "9"
    against "1" as text. Comparing the runs of digits as integers picks the
    genuinely highest version.
    """
    return [int(part) for part in re.split(r"[^0-9]+", spec) if part]

def _resolve_kernel_name(name):
    """Return the best matching installed kernel name for `name`.

    Tries an exact match first. If that fails, looks for installed kernels
    whose name starts with `name` followed by a version separator, either a
    period or a hyphen (e.g. "julia" -> "julia-1.11", which is how IJulia
    registers itself, and "python3.12"). When multiple candidates exist the
    highest version wins.
    """
    if name in _resolved:
        return _resolved[name]
    ksm = KernelSpecManager()
    all_specs = list(ksm.get_all_specs().keys())
    if name in all_specs:
        _resolved[name] = name
        return name
    prefixes = (name + ".", name + "-")
    candidates = [k for k in all_specs if k.startswith(prefixes)]
    if candidates:
        best = sorted(candidates, key=_version_sort_key)[-1]
        _resolved[name] = best
        return best
    # No match found; let KernelManager raise NoSuchKernel with the original name
    _resolved[name] = name
    return name

def _get_kernel(kernel_name, timeout):
    actual = _resolve_kernel_name(kernel_name)
    if actual not in _managers:
        km = KernelManager(kernel_name=actual)
        km.start_kernel()
        kc = km.client()
        kc.start_channels()
        # Store before wait_for_ready: if it raises (e.g. a slow-starting
        # kernel exceeding the timeout), the kernel is already running and
        # must still be reachable for cleanup instead of leaking silently.
        _managers[actual] = (km, kc)
        try:
            kc.wait_for_ready(timeout=timeout)
        except Exception:
            del _managers[actual]
            try:
                kc.stop_channels()
            except Exception:
                pass
            try:
                km.shutdown_kernel(now=True)
            except Exception:
                pass
            raise
    return _managers[actual]

def _strip_ansi(text):
    return re.sub(r'\x1b\[[0-9;]*[mGKH]', '', text)

def _image_mime_for_format(fig_format):
    return {
        "png": "image/png",
        "svg": "image/svg+xml",
        "jpeg": "image/jpeg",
        "jpg": "image/jpeg",
    }.get(fig_format)

__CALEPIN_PLOT_PATH_HELPER__

def _save_image_bundle(data, fig_path, fig_format, plot_index):
    if not fig_path:
        return None, None
    requested_mime = _image_mime_for_format(fig_format)
    if not requested_mime:
        return None, f"unsupported Jupyter figure format {fig_format}"
    if requested_mime not in data:
        if any(m in data for m in ("image/png", "image/svg+xml", "image/jpeg")):
            return None, f"kernel emitted an image, but not requested format {fig_format}"
        return None, None

    raw = data[requested_mime]
    path = _calepin_plot_path(fig_path, plot_index)
    try:
        if requested_mime in ("image/png", "image/jpeg"):
            payload = base64.b64decode(raw) if isinstance(raw, str) else raw
            with open(path, "wb") as fh:
                fh.write(payload)
        else:
            svg = raw if isinstance(raw, str) else raw.decode()
            with open(path, "w", encoding="utf-8") as fh:
                fh.write(svg)
        if os.path.getsize(path) > 0:
            return path, None
        return None, "kernel emitted an empty image"
    except Exception as save_exc:
        return None, f"failed to save figure: {save_exc}"

def _execute(kc, code, fig_path, fig_format, width, height, dpi, sentinel, timeout):
    sep = sentinel + "_SEP"
    parts = [sentinel + "_SOURCE:" + code]
    msg_id = kc.execute(code, store_history=True)
    plot_index = 1
    stream_texts = set()  # deduplicate execute_result vs stream stdout

    while True:
        try:
            msg = kc.get_iopub_msg(timeout=timeout)
        except Exception as exc:
            parts.append(f"{sentinel}_ERROR:kernel timeout: {exc}")
            break

        if msg["parent_header"].get("msg_id") != msg_id:
            continue

        mtype = msg["msg_type"]
        content = msg.get("content", {})

        if mtype == "stream":
            text = content.get("text", "").rstrip("\n")
            if text:
                tag = "OUTPUT" if content["name"] == "stdout" else "WARNING"
                parts.append(f"{sentinel}_{tag}:{text}")
                if content["name"] == "stdout":
                    stream_texts.add(text)

        elif mtype in ("execute_result", "display_data"):
            data = content.get("data", {})
            image_path, image_warning = _save_image_bundle(data, fig_path, fig_format, plot_index)
            if image_path:
                parts.append(f"{sentinel}_PLOT:{image_path}")
                plot_index += 1
            elif image_warning:
                parts.append(f"{sentinel}_WARNING:{image_warning}")

            # For rich display bundles, text/plain is the fallback. If an image
            # was captured, emitting the fallback as stream output would duplicate
            # plot object reprs for kernels such as Julia, Python, and R.
            if not image_path and "text/plain" in data:
                text = data["text/plain"].rstrip("\n")
                if text and text not in stream_texts:
                    parts.append(f"{sentinel}_OUTPUT:{text}")

        elif mtype == "error":
            tb_lines = content.get("traceback", [content.get("evalue", "error")])
            tb = _strip_ansi("\n".join(tb_lines))
            parts.append(f"{sentinel}_ERROR:{tb}")

        elif mtype == "status" and content.get("execution_state") == "idle":
            break

    return ("\n" + sep + "\n").join(parts)

try:
    while True:
        header = sys.stdin.readline()
        if not header:
            break
        _h = header.strip()
        sentinel = _h[:-len("_BEGIN")] if _h.endswith("_BEGIN") else _h
        end_marker = sentinel + "_END"

        lines = []
        while True:
            line = sys.stdin.readline()
            if not line or line.strip() == end_marker:
                break
            lines.append(line)

        if not lines:
            print(sentinel + "_DONE", flush=True)
            continue

        meta_line = lines[0].strip()
        code = "".join(lines[1:])

        # META:{"kernel":"python3","fig_path":"/tmp/ch1.svg","fig_format":"svg",...}
        # JSON avoids corrupting paths that contain ';' or '='.
        meta = {}
        if meta_line.startswith("META:"):
            meta = json.loads(meta_line[5:])

        command = meta.get("command", "execute")
        if command == "ping":
            print(sentinel + "_DONE", flush=True)
            continue
        if command == "shutdown":
            print(sentinel + "_DONE", flush=True)
            break

        kernel_name = meta.get("kernel", "python3")
        fig_path = meta.get("fig_path", "")
        fig_format = meta.get("fig_format", "svg")
        width = float(meta.get("width", "6"))
        height = float(meta.get("height", "4"))
        dpi = float(meta.get("dpi", "150"))
        # Same timeout rule as every other engine: unbounded (None -> block
        # indefinitely) unless the caller configured one. No engine-specific
        # fallback here.
        timeout = meta.get("timeout")
        if timeout is not None:
            timeout = float(timeout)

        try:
            km, kc = _get_kernel(kernel_name, timeout)
            result = _execute(kc, code, fig_path, fig_format, width, height, dpi, sentinel, timeout)
        except NoSuchKernel:
            sep = sentinel + "_SEP"
            result = (sentinel + "_SOURCE:" + code + "\n" + sep + "\n"
                      + sentinel + "_UNAVAILABLE:"
                      + f"Jupyter kernel `{kernel_name}` is not installed")
        except Exception:
            tb = traceback.format_exc()
            sep = sentinel + "_SEP"
            result = (sentinel + "_SOURCE:" + code + "\n" + sep + "\n"
                      + sentinel + "_ERROR:" + tb)

        print(result, flush=True)
        print(sentinel + "_DONE", flush=True)
finally:
    # Guaranteed even if the loop above exits through an unexpected path, not
    # just the normal "shutdown" command or stdin EOF.
    _shutdown_all()
"#;

/// The bootstrap script actually sent to the subprocess: the template with
/// the shared plot-path helper spliced in.
fn jupyter_bridge_script() -> String {
    JUPYTER_BRIDGE_TEMPLATE.replace(PLOT_PATH_HELPER_MARKER, PY_PLOT_INDEX_HELPER)
}

pub struct JupyterBridgeSession {
    proc: SubprocessSession,
}

pub struct JupyterCapture<'a> {
    pub kernel: &'a str,
    pub code: &'a str,
    pub fig_path: &'a str,
    pub fig_format: &'a str,
    pub width: f64,
    pub height: f64,
    pub dpi: f64,
}

impl JupyterBridgeSession {
    pub fn init_with_program(
        program: &Path,
        cwd: Option<&Path>,
        timeout: Option<std::time::Duration>,
    ) -> Result<Self> {
        process::validate_python_interpreter(program, "start Jupyter bridge", Some(&tools::PYTHON))
            .context("failed to start Jupyter bridge")?;
        // No `-s` and no PYTHONNOUSERSITE. The install docs tell users to run
        // `pip install jupyter_client`, which outside a virtualenv lands in
        // the per-user site directory; suppressing that directory made the
        // bridge fail to import the very package it just asked for.
        let env: Vec<(&str, &str)> = vec![("PYTHONDONTWRITEBYTECODE", "1")];
        let bootstrap = jupyter_bridge_script();
        let mut proc = SubprocessSession::spawn(
            program,
            &["-u", "-c", &bootstrap],
            &env,
            cwd,
            timeout,
            Some(&tools::JUPYTER_CLIENT),
        )
        .context("failed to start Jupyter bridge")?;
        let sentinel = make_sentinel();
        proc.execute(
            &sentinel,
            &build_payload(
                serde_json::json!({
                    "command": "ping",
                }),
                "",
            )?,
        )
        .context(
            "jupyter_client Python package not found: install with pip install jupyter_client",
        )?;
        Ok(Self { proc })
    }

    /// True once the underlying subprocess is known dead (e.g. killed after a
    /// chunk timeout). The pool must respawn rather than reuse it: a fresh
    /// bridge process means every kernel it manages is respawned too.
    pub fn is_dead(&self) -> bool {
        self.proc.is_dead()
    }

    pub fn capture(&mut self, request: JupyterCapture<'_>) -> Result<String> {
        let sentinel = make_sentinel();
        // Same timeout as every other engine: unbounded (JSON null, which
        // the bridge treats as "block indefinitely") unless the caller
        // configured one. No Jupyter-specific fallback.
        let timeout_secs = self.proc.timeout().map(|d| d.as_secs_f64());
        let payload = build_payload(
            serde_json::json!({
                "kernel": request.kernel,
                "fig_path": request.fig_path,
                "fig_format": request.fig_format,
                "width": request.width,
                "height": request.height,
                "dpi": request.dpi,
                "timeout": timeout_secs,
            }),
            request.code,
        )?;
        self.proc.execute(&sentinel, &payload)
    }

    fn shutdown(&mut self) -> Result<()> {
        let sentinel = make_sentinel();
        self.proc
            .execute(
                &sentinel,
                &build_payload(
                    serde_json::json!({
                        "command": "shutdown",
                    }),
                    "",
                )?,
            )
            .map(|_| ())
    }
}

impl Drop for JupyterBridgeSession {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::time::Duration;

    #[test]
    fn jupyter_bridge_bootstrap_is_valid_python() {
        let bootstrap = jupyter_bridge_script();
        let status = Command::new("python3")
            .args([
                "-c",
                &format!("compile({:?}, '<bootstrap>', 'exec')", bootstrap),
            ])
            .status();
        match status {
            Ok(s) => assert!(s.success(), "JUPYTER_BRIDGE has a Python syntax error"),
            Err(_) => eprintln!("python3 not found, skipping bootstrap syntax check"),
        }
    }

    /// Runs the real bootstrap's `_resolve_kernel_name` against a stubbed
    /// kernel list, so the matching rules can be checked without installing
    /// IJulia (or any other kernel) on the machine running the tests.
    fn resolve_kernel_name_with_specs(requested: &str, installed: &[&str]) -> Option<String> {
        if !crate::utils::testtools::command_available("python3") {
            return None;
        }
        let specs: Vec<String> = installed.iter().map(|name| format!("{name:?}")).collect();
        let harness = format!(
            "import sys, types\n\
             fake = types.ModuleType('jupyter_client')\n\
             ks = types.ModuleType('jupyter_client.kernelspec')\n\
             class KernelSpecManager:\n\
             \x20   def get_all_specs(self):\n\
             \x20       return {{name: {{}} for name in [{}]}}\n\
             ks.KernelSpecManager = KernelSpecManager\n\
             ks.NoSuchKernel = type('NoSuchKernel', (Exception,), {{}})\n\
             fake.KernelManager = object\n\
             fake.kernelspec = ks\n\
             sys.modules['jupyter_client'] = fake\n\
             sys.modules['jupyter_client.kernelspec'] = ks\n\
             ns = {{'__name__': 'bridge'}}\n\
             exec(compile(BOOTSTRAP, '<bootstrap>', 'exec'), ns)\n\
             print(ns['_resolve_kernel_name']({requested:?}))\n",
            specs.join(", ")
        );
        let program = format!("BOOTSTRAP = {:?}\n{harness}", jupyter_bridge_script());
        let output = Command::new("python3")
            .args(["-c", &program])
            .output()
            .ok()?;
        assert!(
            output.status.success(),
            "resolver harness failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    #[test]
    fn julia_resolves_to_a_hyphenated_ijulia_kernel() {
        // IJulia registers itself as `julia-1.11`, so a bare `julia` fence has
        // to match across the hyphen or it never finds an installed kernel.
        let Some(resolved) = resolve_kernel_name_with_specs("julia", &["python3", "julia-1.11"])
        else {
            return;
        };
        assert_eq!(resolved, "julia-1.11");
    }

    #[test]
    fn a_version_suffixed_kernel_picks_the_highest_version() {
        let Some(resolved) =
            resolve_kernel_name_with_specs("julia", &["julia-1.9", "julia-1.11", "julia-1.10"])
        else {
            return;
        };
        // Lexicographic ordering would pick 1.9 here.
        assert_eq!(resolved, "julia-1.11");
    }

    #[test]
    fn an_exact_kernel_name_still_wins_over_a_version_suffixed_one() {
        let Some(resolved) = resolve_kernel_name_with_specs("python3", &["python3", "python3.12"])
        else {
            return;
        };
        assert_eq!(resolved, "python3");
    }

    #[test]
    fn an_unknown_kernel_resolves_to_itself_so_the_caller_reports_it() {
        let Some(resolved) = resolve_kernel_name_with_specs("nosuch", &["python3"]) else {
            return;
        };
        assert_eq!(resolved, "nosuch");
    }

    fn has_python_jupyter_kernel() -> bool {
        Command::new("python3")
            .args([
                "-c",
                "import jupyter_client; from jupyter_client.kernelspec import KernelSpecManager; KernelSpecManager().get_kernel_spec('python3')",
            ])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn jupyter_bridge_captures_image_bundle_without_text_fallback() {
        if !has_python_jupyter_kernel() {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let fig_path = dir.path().join("bundle-1.svg");
        let fig_path = fig_path.to_string_lossy().replace('\\', "/");
        let mut session = JupyterBridgeSession::init_with_program(
            Path::new("python3"),
            None,
            Some(Duration::from_secs(10)),
        )
        .unwrap();

        let raw = session
            .capture(JupyterCapture {
                kernel: "python3",
                code: r#"from IPython.display import display
display({
    "image/svg+xml": "<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'><circle cx='5' cy='5' r='4'/></svg>",
    "text/plain": "fallback text",
}, raw=True)"#,
                fig_path: &fig_path,
                fig_format: "svg",
                width: 6.0,
                height: 3.708,
                dpi: 150.0,
            })
            .unwrap();

        assert!(raw.contains("_PLOT:"), "{raw}");
        assert!(!raw.contains("_OUTPUT:fallback text"), "{raw}");
        assert!(std::path::Path::new(&fig_path).exists());
    }

    #[test]
    fn jupyter_bridge_captures_multiple_image_bundles() {
        if !has_python_jupyter_kernel() {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let fig_path = dir.path().join("bundle-1.svg");
        let second_fig_path = dir.path().join("bundle-2.svg");
        let fig_path = fig_path.to_string_lossy().replace('\\', "/");
        let second_fig_path = second_fig_path.to_string_lossy().replace('\\', "/");
        let mut session = JupyterBridgeSession::init_with_program(
            Path::new("python3"),
            None,
            Some(Duration::from_secs(10)),
        )
        .unwrap();

        let raw = session
            .capture(JupyterCapture {
                kernel: "python3",
                code: r#"from IPython.display import display
display({"image/svg+xml": "<svg xmlns='http://www.w3.org/2000/svg'><path d='M0 0L1 1'/></svg>"}, raw=True)
display({"image/svg+xml": "<svg xmlns='http://www.w3.org/2000/svg'><path d='M1 0L0 1'/></svg>"}, raw=True)"#,
                fig_path: &fig_path,
                fig_format: "svg",
                width: 6.0,
                height: 3.708,
                dpi: 150.0,
            })
            .unwrap();

        assert_eq!(raw.matches("_PLOT:").count(), 2, "{raw}");
        assert!(std::path::Path::new(&fig_path).exists());
        assert!(std::path::Path::new(&second_fig_path).exists());
    }
}
