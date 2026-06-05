// Jupyter kernel bridge session via a persistent python3 subprocess.
//
// A single Python process manages one jupyter_client.KernelManager per
// named kernel (e.g. "octave", "ruby"). Code is sent via the sentinel
// protocol (see subprocess.rs); the bridge translates Jupyter message
// types to sentinel tags and writes them back.

use anyhow::{Context, Result};

use super::make_sentinel;
use super::subprocess::SubprocessSession;

pub(crate) const JUPYTER_BRIDGE: &str = r#"
import sys, base64, os, traceback, re, json

try:
    from jupyter_client import KernelManager
except ImportError:
    sys.stderr.write(
        "calepin: jupyter_client not found - "
        "install with: pip install jupyter_client\n"
    )
    sys.exit(1)

_managers = {}  # kernel_name -> (km, kc)

def _shutdown_all():
    for km, kc in list(_managers.values()):
        try:
            kc.stop_channels()
            km.shutdown_kernel(now=True)
        except Exception:
            pass
    _managers.clear()

def _get_kernel(kernel_name, timeout):
    if kernel_name not in _managers:
        km = KernelManager(kernel_name=kernel_name)
        km.start_kernel()
        kc = km.client()
        kc.start_channels()
        kc.wait_for_ready(timeout=timeout)
        _managers[kernel_name] = (km, kc)
    return _managers[kernel_name]

def _strip_ansi(text):
    return re.sub(r'\x1b\[[0-9;]*[mGKH]', '', text)

def _execute(kc, code, fig_path, fig_format, width, height, dpi, sentinel, timeout):
    sep = sentinel + "_SEP"
    parts = [sentinel + "_SOURCE:" + code]
    msg_id = kc.execute(code, store_history=True)
    plot_saved = False
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
            if "text/plain" in data:
                text = data["text/plain"].rstrip("\n")
                if text and text not in stream_texts:
                    parts.append(f"{sentinel}_OUTPUT:{text}")
            if fig_path and not plot_saved:
                requested_mime = {
                    "png": "image/png",
                    "svg": "image/svg+xml",
                }.get(fig_format)
                if requested_mime and requested_mime in data:
                    raw = data[requested_mime]
                    try:
                        if requested_mime == "image/png":
                            payload = base64.b64decode(raw) if isinstance(raw, str) else raw
                            with open(fig_path, "wb") as fh:
                                fh.write(payload)
                        else:  # svg
                            svg = raw if isinstance(raw, str) else raw.decode()
                            with open(fig_path, "w", encoding="utf-8") as fh:
                                fh.write(svg)
                        if os.path.getsize(fig_path) > 0:
                            parts.append(f"{sentinel}_PLOT:{fig_path}")
                            plot_saved = True
                    except Exception as save_exc:
                        parts.append(f"{sentinel}_WARNING:failed to save figure: {save_exc}")
                elif any(m in data for m in ("image/png", "image/svg+xml")):
                    parts.append(
                        f"{sentinel}_WARNING:kernel emitted an image, but not requested format {fig_format}"
                    )

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

impl JupyterBridgeSession {
    pub fn init_with_program(
        program: &str,
        cwd: Option<&std::path::Path>,
        timeout: Option<std::time::Duration>,
    ) -> Result<Self> {
        let mut proc = SubprocessSession::spawn(
            program,
            &["-s", "-u", "-c", JUPYTER_BRIDGE],
            &[
                ("PYTHONDONTWRITEBYTECODE", "1"),
                ("PYTHONNOUSERSITE", "1"),
            ],
            cwd,
            timeout,
        )
        .context("failed to start Jupyter bridge")?;
        let sentinel = make_sentinel();
        proc.execute(&sentinel, r#"META:{"command":"ping"}"#)
            .context("jupyter_client Python package not found — install with: pip install jupyter_client")?;
        Ok(Self { proc })
    }

    pub fn capture(
        &mut self,
        kernel: &str,
        code: &str,
        fig_path: &str,
        fig_format: &str,
        width: f64,
        height: f64,
        dpi: f64,
    ) -> Result<String> {
        let sentinel = make_sentinel();
        let timeout_secs = self
            .proc
            .timeout()
            .map(|d| d.as_secs_f64())
            .unwrap_or(30.0);
        let meta = serde_json::json!({
            "kernel": kernel,
            "fig_path": fig_path,
            "fig_format": fig_format,
            "width": width,
            "height": height,
            "dpi": dpi,
            "timeout": timeout_secs,
        });
        let payload = format!("META:{meta}\n{code}");
        self.proc.execute(&sentinel, &payload)
    }

    fn shutdown(&mut self) -> Result<()> {
        let sentinel = make_sentinel();
        self.proc
            .execute(&sentinel, r#"META:{"command":"shutdown"}"#)
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
    #[test]
    fn jupyter_bridge_bootstrap_is_valid_python() {
        use std::process::Command;
        let status = Command::new("python3")
            .args(["-c", &format!(
                "compile({:?}, '<bootstrap>', 'exec')",
                super::JUPYTER_BRIDGE
            )])
            .status();
        match status {
            Ok(s) => assert!(s.success(), "JUPYTER_BRIDGE has a Python syntax error"),
            Err(_) => eprintln!("python3 not found — skipping bootstrap syntax check"),
        }
    }
}
