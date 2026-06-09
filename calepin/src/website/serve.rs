use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::cli::ServeArgs;

pub(crate) type ReloadVersion = Arc<AtomicU64>;

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
    let bind = format!("{}:{}", args.host, args.port);
    let server = bind_server(&bind)?;
    eprintln!("Serving {} at http://{bind}/", root.display());
    eprintln!("Press Ctrl+C to stop.");
    run_server(server, root, None, Arc::new(AtomicBool::new(false)))
}

pub(crate) fn start(
    dir: &Path,
    host: &str,
    port: u16,
    reload_version: ReloadVersion,
) -> Result<ServeHandle> {
    let root = validate_root(dir)?;
    let bind = format!("{host}:{port}");
    let server = bind_server(&bind)?;
    eprintln!("Serving {} at http://{bind}/", root.display());
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let join = thread::spawn(move || {
        if let Err(error) = run_server(server, root, Some(reload_version), thread_stop) {
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

fn bind_server(bind: &str) -> Result<Server> {
    Server::http(bind).map_err(|error| {
        let message = error.to_string();
        if message.contains("Address already in use") || message.contains("os error 48") {
            anyhow!(
                "failed to bind {bind}: address already in use; choose another port with `--port`"
            )
        } else {
            anyhow!("failed to bind {bind}: {message}")
        }
    })
}

fn run_server(
    server: Server,
    root: PathBuf,
    reload_version: Option<ReloadVersion>,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    while !stop.load(Ordering::Relaxed) {
        match server.recv_timeout(Duration::from_millis(200)) {
            Ok(Some(request)) => {
                if let Err(error) = respond(request, &root, reload_version.as_ref()) {
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

fn respond(request: Request, root: &Path, reload_version: Option<&ReloadVersion>) -> Result<()> {
    if request.url() == "/__calepin/reload-version" {
        let version = reload_version
            .map(|version| version.load(Ordering::Relaxed))
            .unwrap_or(0);
        return send_text(
            request,
            200,
            "application/json; charset=utf-8",
            &version.to_string(),
        );
    }

    if request.method() != &Method::Get && request.method() != &Method::Head {
        return send_text(
            request,
            405,
            "text/plain; charset=utf-8",
            "Method Not Allowed",
        );
    }

    let Some(path) = resolve_request_path(root, request.url()) else {
        return send_text(request, 403, "text/plain; charset=utf-8", "Forbidden");
    };
    let path = if path.is_dir() {
        path.join("index.html")
    } else {
        path
    };
    if !path.is_file() {
        return send_text(request, 404, "text/plain; charset=utf-8", "Not Found");
    }

    let mime = mime_guess::from_path(&path)
        .first_or_octet_stream()
        .essence_str()
        .to_string();
    let is_html = mime == "text/html";
    let mut body = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    if is_html && reload_version.is_some() {
        body = inject_reload_script(body);
    }

    if request.method() == &Method::Head {
        let response = Response::empty(StatusCode(200)).with_header(content_type_header(&mime)?);
        request.respond(response)?;
    } else {
        let response = Response::from_data(body)
            .with_status_code(StatusCode(200))
            .with_header(content_type_header(&mime)?);
        request.respond(response)?;
    }
    Ok(())
}

fn send_text(request: Request, status: u16, content_type: &str, body: &str) -> Result<()> {
    let response = Response::from_string(body.to_string())
        .with_status_code(StatusCode(status))
        .with_header(content_type_header(content_type)?);
    request.respond(response)?;
    Ok(())
}

fn content_type_header(value: &str) -> Result<Header> {
    Header::from_bytes("Content-Type", value)
        .map_err(|_| anyhow!("failed to create content-type header"))
}

fn inject_reload_script(mut body: Vec<u8>) -> Vec<u8> {
    const SCRIPT: &[u8] = br#"<script>
(() => {
  let version = null;
  async function poll() {
    try {
      const response = await fetch("/__calepin/reload-version", { cache: "no-store" });
      if (!response.ok) return;
      const next = await response.text();
      if (version === null) version = next;
      else if (next !== version) window.location.reload();
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

fn resolve_request_path(root: &Path, target: &str) -> Option<PathBuf> {
    let path = target
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(target);
    let decoded = percent_decode(path)?;
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

    #[test]
    fn request_path_rejects_traversal() {
        let root = Path::new("/tmp/site");
        assert!(resolve_request_path(root, "/index.html").is_some());
        assert!(resolve_request_path(root, "/../secret").is_none());
        assert!(resolve_request_path(root, "/%2e%2e/secret").is_none());
    }

    #[test]
    fn injects_reload_script_before_body_close() {
        let html = inject_reload_script(b"<html><body>Hello</body></html>".to_vec());
        let text = String::from_utf8(html).unwrap();
        assert!(text.contains("/__calepin/reload-version"));
        assert!(text.contains("</script></body>"));
    }
}
