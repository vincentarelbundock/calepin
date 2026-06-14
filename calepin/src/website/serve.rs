use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::cli::ServeArgs;

const DEFAULT_PORT: u16 = 8000;
const AUTO_PORT_ATTEMPTS: u16 = 50;
const STATUS_ENDPOINT: &str = "/__calepin/status";

/// Shared build status polled by the injected reload script: a version that
/// bumps on successful rebuilds, and the latest rebuild error, if any.
pub(crate) struct LiveReload {
    version: AtomicU64,
    error: Mutex<Option<String>>,
}

impl LiveReload {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            version: AtomicU64::new(1),
            error: Mutex::new(None),
        })
    }

    pub(crate) fn rebuilt(&self) {
        *self.error.lock().unwrap() = None;
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn set_error(&self, message: String) {
        *self.error.lock().unwrap() = Some(message);
    }

    fn status_json(&self) -> String {
        serde_json::json!({
            "version": self.version.load(Ordering::Relaxed),
            "error": *self.error.lock().unwrap(),
        })
        .to_string()
    }
}

pub(crate) struct ServeHandle {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl ServeHandle {
    pub(crate) fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

pub(crate) fn serve(args: ServeArgs) -> Result<()> {
    let root = validate_root(&args.dir)?;
    let base_path_prefix = local_base_path_prefix(&root);
    let (server, bind) = bind_server(&args.host, args.port)?;
    let url = browser_url(&args.host, &bind);
    eprintln!("Serving {} at {url}", root.display());
    if args.open {
        open_browser(&url);
    }
    eprintln!("Press Ctrl+C to stop.");
    run_server(
        server,
        root,
        base_path_prefix,
        None,
        Arc::new(AtomicBool::new(false)),
    )
}

pub(crate) fn start(
    dir: &Path,
    host: &str,
    port: Option<u16>,
    live: Arc<LiveReload>,
    open: bool,
) -> Result<ServeHandle> {
    let root = validate_root(dir)?;
    let base_path_prefix = local_base_path_prefix(&root);
    let (server, bind) = bind_server(host, port)?;
    let url = browser_url(host, &bind);
    eprintln!("Serving {} at {url}", root.display());
    if open {
        open_browser(&url);
    }
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let join = thread::spawn(move || {
        if let Err(error) = run_server(server, root, base_path_prefix, Some(live), thread_stop) {
            cwarn!("serve failed: {}", error);
        }
    });
    Ok(ServeHandle {
        stop,
        join: Some(join),
    })
}

fn validate_root(dir: &Path) -> Result<PathBuf> {
    let root =
        fs::canonicalize(dir).with_context(|| format!("failed to resolve {}", dir.display()))?;
    if !root.is_dir() {
        return Err(anyhow!("serve path is not a directory: {}", root.display()));
    }
    Ok(root)
}

fn local_base_path_prefix(root: &Path) -> Option<String> {
    let config_path = root.join(super::DEFAULT_CONFIG);
    let config = super::load_website_config(&config_path, false).ok()?;
    config
        .base_url
        .as_deref()
        .and_then(super::base_url_path_prefix)
}

/// Binds the requested port, or scans for a free one starting at
/// `DEFAULT_PORT` when no port was requested. Returns the bound address.
fn bind_server(host: &str, port: Option<u16>) -> Result<(Server, String)> {
    match port {
        Some(port) => {
            let bind = format!("{host}:{port}");
            let server = Server::http(&bind).map_err(|error| {
                let message = error.to_string();
                if is_port_in_use(&message) {
                    anyhow!(
                        "failed to bind {bind}: address already in use; choose another port with `--port`"
                    )
                } else {
                    anyhow!("failed to bind {bind}: {message}")
                }
            })?;
            Ok((server, bind))
        }
        None => auto_bind(host, DEFAULT_PORT, AUTO_PORT_ATTEMPTS),
    }
}

fn auto_bind(host: &str, start: u16, attempts: u16) -> Result<(Server, String)> {
    let mut last_message = String::new();
    for port in start..start.saturating_add(attempts) {
        let bind = format!("{host}:{port}");
        match Server::http(&bind) {
            Ok(server) => return Ok((server, bind)),
            Err(error) => {
                last_message = error.to_string();
                if !is_port_in_use(&last_message) {
                    return Err(anyhow!("failed to bind {bind}: {last_message}"));
                }
            }
        }
    }
    Err(anyhow!(
        "failed to bind {host}: no free port between {start} and {}: {last_message}",
        start.saturating_add(attempts).saturating_sub(1)
    ))
}

fn is_port_in_use(message: &str) -> bool {
    // EADDRINUSE: macOS (48), Linux (98), Windows (10048).
    message.contains("in use")
        || message.contains("os error 48")
        || message.contains("os error 98")
        || message.contains("os error 10048")
}

fn browser_url(host: &str, bind: &str) -> String {
    let Some((_, port)) = bind.rsplit_once(':') else {
        return format!("http://{bind}/");
    };
    let browser_host = match host {
        "0.0.0.0" => "127.0.0.1".to_string(),
        "::" | "[::]" => "[::1]".to_string(),
        value if value.contains(':') && !value.starts_with('[') => format!("[{value}]"),
        value => value.to_string(),
    };
    format!("http://{browser_host}:{port}/")
}

fn open_browser(url: &str) {
    if let Err(error) = launch_browser(url) {
        cwarn!("failed to open browser: {}", error);
    }
}

fn launch_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .spawn()
            .context("failed to run `open`")?;
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .context("failed to run `cmd /C start`")?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .context("failed to run `xdg-open`")?;
    }
    Ok(())
}

fn run_server(
    server: Server,
    root: PathBuf,
    base_path_prefix: Option<String>,
    live: Option<Arc<LiveReload>>,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    while !stop.load(Ordering::Relaxed) {
        match server.recv_timeout(Duration::from_millis(200)) {
            Ok(Some(request)) => {
                if let Err(error) =
                    respond(request, &root, base_path_prefix.as_deref(), live.as_deref())
                {
                    cwarn!("serve request failed: {}", error);
                }
            }
            Ok(None) => {}
            Err(error) => {
                cwarn!("serve connection failed: {}", error);
            }
        }
    }
    Ok(())
}

fn respond(
    request: Request,
    root: &Path,
    base_path_prefix: Option<&str>,
    live: Option<&LiveReload>,
) -> Result<()> {
    if request.url() == STATUS_ENDPOINT {
        let status = live
            .map(LiveReload::status_json)
            .unwrap_or_else(|| "{\"version\":0,\"error\":null}".to_string());
        return send_text(request, 200, "application/json; charset=utf-8", &status);
    }

    if request.method() != &Method::Get && request.method() != &Method::Head {
        return send_text(
            request,
            405,
            "text/plain; charset=utf-8",
            "Method Not Allowed",
        );
    }

    let Some(path) = resolve_request_path(root, request.url(), base_path_prefix) else {
        return send_text(request, 403, "text/plain; charset=utf-8", "Forbidden");
    };
    let path = if path.is_dir() {
        path.join("index.html")
    } else {
        path
    };
    if !path.is_file() {
        let fallback = root.join("404.html");
        if fallback.is_file() {
            return send_file(request, &fallback, 404, live);
        }
        return send_text(request, 404, "text/plain; charset=utf-8", "Not Found");
    }

    send_file(request, &path, 200, live)
}

fn send_file(request: Request, path: &Path, status: u16, live: Option<&LiveReload>) -> Result<()> {
    let mime = mime_guess::from_path(&path)
        .first_or_octet_stream()
        .essence_str()
        .to_string();
    let is_html = mime == "text/html";
    let mut body = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if is_html && live.is_some() {
        body = inject_reload_script(body);
    }

    if request.method() == &Method::Head {
        let response = Response::empty(StatusCode(status))
            .with_header(content_type_header(&mime)?)
            .with_header(no_store_header()?);
        request.respond(response)?;
    } else {
        let response = Response::from_data(body)
            .with_status_code(StatusCode(status))
            .with_header(content_type_header(&mime)?)
            .with_header(no_store_header()?);
        request.respond(response)?;
    }
    Ok(())
}

fn send_text(request: Request, status: u16, content_type: &str, body: &str) -> Result<()> {
    let response = Response::from_string(body)
        .with_status_code(StatusCode(status))
        .with_header(content_type_header(content_type)?)
        .with_header(no_store_header()?);
    request.respond(response)?;
    Ok(())
}

fn content_type_header(value: &str) -> Result<Header> {
    Header::from_bytes("Content-Type", value)
        .map_err(|_| anyhow!("failed to create content-type header"))
}

fn no_store_header() -> Result<Header> {
    Header::from_bytes("Cache-Control", "no-store")
        .map_err(|_| anyhow!("failed to create cache-control header"))
}

fn inject_reload_script(mut body: Vec<u8>) -> Vec<u8> {
    const SCRIPT: &[u8] = br#"<script>
(() => {
  let version = null;
  const overlayId = "calepin-error-overlay";
  function setOverlay(message) {
    let overlay = document.getElementById(overlayId);
    if (!message) {
      if (overlay) overlay.remove();
      return;
    }
    if (!overlay) {
      overlay = document.createElement("pre");
      overlay.id = overlayId;
      overlay.style.cssText =
        "position:fixed;inset:0;z-index:2147483647;margin:0;padding:2rem;" +
        "overflow:auto;background:rgba(15,15,15,0.95);color:#ff8a8a;" +
        "font:14px/1.5 ui-monospace,monospace;white-space:pre-wrap;";
      document.body.appendChild(overlay);
    }
    overlay.textContent = "calepin build failed\n\n" + message;
  }
  async function poll() {
    try {
      const response = await fetch("/__calepin/status", { cache: "no-store" });
      if (!response.ok) return;
      const status = await response.json();
      setOverlay(status.error);
      if (version === null) version = status.version;
      else if (!status.error && status.version !== version) window.location.reload();
    } catch {}
  }
  window.setInterval(poll, 1000);
  poll();
})();
</script>"#;

    if let Some(index) = find_case_insensitive(&body, b"</body>") {
        let mut out = Vec::with_capacity(body.len() + SCRIPT.len());
        out.extend_from_slice(&body[..index]);
        out.extend_from_slice(SCRIPT);
        out.extend_from_slice(&body[index..]);
        out
    } else {
        body.extend_from_slice(SCRIPT);
        body
    }
}

fn find_case_insensitive(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
}

fn resolve_request_path(
    root: &Path,
    target: &str,
    base_path_prefix: Option<&str>,
) -> Option<PathBuf> {
    let path = target
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(target);
    let decoded = percent_decode(path)?;
    let decoded = strip_base_path_prefix(&decoded, base_path_prefix);
    let mut out = root.to_path_buf();
    for component in Path::new(decoded.trim_start_matches('/')).components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(out)
}

fn strip_base_path_prefix<'a>(path: &'a str, base_path_prefix: Option<&str>) -> &'a str {
    let Some(prefix) = base_path_prefix else {
        return path;
    };
    let prefix = prefix.trim_end_matches('/');
    if prefix.is_empty() || prefix == "/" {
        return path;
    }
    if path == prefix {
        return "/";
    }
    path.strip_prefix(prefix)
        .filter(|rest| rest.starts_with('/'))
        .unwrap_or(path)
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let high = hex_value(bytes[index + 1])?;
            let low = hex_value(bytes[index + 2])?;
            out.push(high * 16 + low);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    #[test]
    fn request_path_rejects_traversal() {
        let root = Path::new("/tmp/site");
        assert!(resolve_request_path(root, "/index.html", None).is_some());
        assert!(resolve_request_path(root, "/../secret", None).is_none());
        assert!(resolve_request_path(root, "/%2e%2e/secret", None).is_none());
    }

    #[test]
    fn request_path_accepts_configured_base_url_prefix() {
        let root = Path::new("/tmp/site");

        assert_eq!(
            resolve_request_path(root, "/calepin/notebooks/guide.html", Some("/calepin")).unwrap(),
            root.join("notebooks").join("guide.html")
        );
        assert_eq!(
            resolve_request_path(root, "/calepin", Some("/calepin")).unwrap(),
            root
        );
        assert_eq!(
            resolve_request_path(root, "/other/notebooks/guide.html", Some("/calepin")).unwrap(),
            root.join("other").join("notebooks").join("guide.html")
        );
    }

    #[test]
    fn injects_reload_script_before_body_close() {
        let html = inject_reload_script(b"<html><body>Hello</body></html>".to_vec());
        let text = String::from_utf8(html).unwrap();
        assert!(text.contains(STATUS_ENDPOINT));
        assert!(text.contains("calepin-error-overlay"));
        assert!(text.contains("</script></body>"));
    }

    #[test]
    fn live_reload_reports_error_then_clears_on_rebuild() {
        let live = LiveReload::new();
        let status: serde_json::Value = serde_json::from_str(&live.status_json()).unwrap();
        assert!(status["error"].is_null());
        let initial_version = status["version"].as_u64().unwrap();

        live.set_error("boom".to_string());
        let status: serde_json::Value = serde_json::from_str(&live.status_json()).unwrap();
        assert_eq!(status["error"], "boom");

        live.rebuilt();
        let status: serde_json::Value = serde_json::from_str(&live.status_json()).unwrap();
        assert!(status["error"].is_null());
        assert!(status["version"].as_u64().unwrap() > initial_version);
    }

    #[test]
    fn browser_url_uses_loopback_for_wildcard_hosts() {
        assert_eq!(
            browser_url("0.0.0.0", "0.0.0.0:8000"),
            "http://127.0.0.1:8000/"
        );
        assert_eq!(browser_url("::", ":::8000"), "http://[::1]:8000/");
    }

    #[test]
    fn browser_url_brackets_ipv6_hosts() {
        assert_eq!(browser_url("::1", "::1:8000"), "http://[::1]:8000/");
    }

    #[test]
    fn missing_path_serves_404_html_when_present() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("404.html"),
            "<html><body>Custom not found</body></html>",
        )
        .unwrap();
        let root = dir.path().to_path_buf();
        let server = Server::http("127.0.0.1:0").unwrap();
        let addr = server.server_addr().to_string();

        let handle = thread::spawn(move || {
            let request = server.recv().unwrap();
            respond(request, &root, None, None).unwrap();
        });

        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .write_all(
                b"GET /missing-page HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        handle.join().unwrap();

        assert!(response.starts_with("HTTP/1.1 404"));
        assert!(response.contains("Content-Type: text/html"));
        assert!(response.contains("Custom not found"));
    }

    #[test]
    fn auto_bind_skips_busy_port() {
        let busy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let busy_port = busy.local_addr().unwrap().port();

        let (_server, bind) = auto_bind("127.0.0.1", busy_port, 10).unwrap();

        assert_ne!(bind, format!("127.0.0.1:{busy_port}"));
    }
}
