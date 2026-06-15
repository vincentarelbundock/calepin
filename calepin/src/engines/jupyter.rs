// Jupyter kernel bridge session via a persistent python3 subprocess.
//
// A single Python process manages one jupyter_client.KernelManager per
// named kernel (e.g. "octave", "ruby"). Code is sent via the sentinel
// protocol (see subprocess.rs); the bridge translates Jupyter message
// types to sentinel tags and writes them back.

use anyhow::{Context, Result};
use std::path::Path;

use super::{build_payload, make_sentinel};
use super::subprocess::SubprocessSession;
use crate::utils::tools;

pub(crate) const JUPYTER_BRIDGE: &str = r#"
import sys, base64, os, traceback, re, json

try:
    from jupyter_client import KernelManager
    from jupyter_client.kernelspec import KernelSpecManager
except ImportError:
    sys.stderr.write(
        "calepin: jupyter_client not found - "
        "install with: pip install jupyter_client\n"
    )
    sys.exit(1)

_managers = {}  # kernel_name -> (km, kc)
_resolved = {}  # requested name -> actual installed kernel name

def _shutdown_all():
    for km, kc in list(_managers.values()):
        try:
            kc.stop_channels()
            km.shutdown_kernel(now=True)
        except Exception:
            pass
    _managers.clear()

def _resolve_kernel_name(name):
    """Return the best matching installed kernel name for `name`.

    Tries an exact match first. If that fails, looks for installed kernels
    whose name starts with `name` followed by a period (e.g. "julia-1" ->
    "julia-1.11"). When multiple candidates exist the lexicographically
    largest one is returned so that the highest minor/patch version wins.
    """
    if name in _resolved:
        return _resolved[name]
    ksm = KernelSpecManager()
    all_specs = list(ksm.get_all_specs().keys())
    if name in all_specs:
        _resolved[name] = name
        return name
    prefix = name + "."
    candidates = [k for k in all_specs if k.startswith(prefix)]
    if candidates:
        best = sorted(candidates)[-1]
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
        kc.wait_for_ready(timeout=timeout)
        _managers[actual] = (km, kc)
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

def _plot_path(base, index):
    if index <= 1:
        return base
    root, ext = os.path.splitext(base)
    if root.endswith("-1"):
        root = root[:-2]
    return f"{root}-{index}{ext}"

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
    path = _plot_path(fig_path, plot_index)
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
    timeout = float(meta.get("timeout", "30"))

    try:
        km, kc = _get_kernel(kernel_name, timeout)
        result = _execute(kc, code, fig_path, fig_format, width, height, dpi, sentinel, timeout)
    except Exception:
        tb = traceback.format_exc()
        sep = sentinel + "_SEP"
        result = (sentinel + "_SOURCE:" + code + "\n" + sep + "\n"
                  + sentinel + "_ERROR:" + tb)

    print(result, flush=True)
    print(sentinel + "_DONE", flush=True)

_shutdown_all()
"#;

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
        params_path: Option<&Path>,
    ) -> Result<Self> {
        // Set CALEPIN_PARAMS_PATH on the bridge process before it launches any
        // kernel, so every kernel (whatever its language) inherits it and can
        // read params.json. This is the only reliable transport for kernels
        // Calepin cannot auto-bind.
        let mut env: Vec<(&str, &str)> =
            vec![("PYTHONDONTWRITEBYTECODE", "1"), ("PYTHONNOUSERSITE", "1")];
        let params_path = params_path.map(|path| path.to_string_lossy().into_owned());
        if let Some(path) = params_path.as_deref() {
            env.push(("CALEPIN_PARAMS_PATH", path));
        }
        let mut proc = SubprocessSession::spawn(
            program,
            &["-s", "-u", "-c", JUPYTER_BRIDGE],
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
            "jupyter_client Python package not found — install with: pip install jupyter_client",
        )?;
        Ok(Self { proc })
    }

    pub fn capture(&mut self, request: JupyterCapture<'_>) -> Result<String> {
        let sentinel = make_sentinel();
        let timeout_secs = self.proc.timeout().map(|d| d.as_secs_f64()).unwrap_or(30.0);
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
        let status = Command::new("python3")
            .args([
                "-c",
                &format!("compile({:?}, '<bootstrap>', 'exec')", JUPYTER_BRIDGE),
            ])
            .status();
        match status {
            Ok(s) => assert!(s.success(), "JUPYTER_BRIDGE has a Python syntax error"),
            Err(_) => eprintln!("python3 not found — skipping bootstrap syntax check"),
        }
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
            None,
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
            None,
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
