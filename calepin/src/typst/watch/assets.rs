use std::fs;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::utils::static_files::{
    content_type, is_sensitive_served_path, raw_http_response_head, resolve_existing_file,
    TEXT_PLAIN_UTF8,
};

pub(crate) struct AssetServer {
    base_url: String,
    handle: thread::JoinHandle<()>,
}

impl AssetServer {
    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    pub(crate) fn join(self) {
        if self.handle.join().is_err() {
            cwarn!("Calepin asset server panicked");
        }
    }
}

pub(crate) fn start(root: PathBuf, stop: Arc<AtomicBool>) -> Result<AssetServer> {
    let root = root
        .canonicalize()
        .with_context(|| format!("asset server root not found: {}", root.display()))?;
    let listener =
        TcpListener::bind(("127.0.0.1", 0)).context("failed to bind Calepin HTML asset server")?;
    listener
        .set_nonblocking(true)
        .context("failed to configure Calepin HTML asset server")?;
    let port = listener
        .local_addr()
        .context("failed to read Calepin HTML asset server address")?
        .port();
    let handle = thread::spawn(move || serve(listener, root, stop));

    Ok(AssetServer {
        base_url: format!("http://127.0.0.1:{port}"),
        handle,
    })
}

fn serve(listener: TcpListener, root: PathBuf, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _addr)) => {
                if let Err(error) = handle_connection(stream, &root) {
                    cwarn!("Calepin asset request failed: {}", error);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => {
                cwarn!("Calepin asset server failed: {}", error);
                break;
            }
        }
    }
}

fn handle_connection(mut stream: TcpStream, root: &Path) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;

    let mut buffer = [0_u8; 8192];
    let read = stream.read(&mut buffer)?;
    if read == 0 {
        return Ok(());
    }
    let request = String::from_utf8_lossy(&buffer[..read]);
    let request_line = request.lines().next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    let is_head = method == "HEAD";
    let origin = allowed_cors_origin(&request);

    if method != "GET" && !is_head {
        return write_response(
            &mut stream,
            "405 Method Not Allowed",
            TEXT_PLAIN_UTF8,
            b"method not allowed\n",
            is_head,
            origin.as_deref(),
        );
    }

    let Some(path) = resolve_existing_file(root, target, None)
        .filter(|path| !is_sensitive_served_path(root, path))
    else {
        return write_response(
            &mut stream,
            "404 Not Found",
            TEXT_PLAIN_UTF8,
            b"not found\n",
            is_head,
            origin.as_deref(),
        );
    };

    match fs::read(&path) {
        Ok(body) => write_response(
            &mut stream,
            "200 OK",
            &content_type(&path),
            &body,
            is_head,
            origin.as_deref(),
        ),
        Err(error) => {
            let body = format!("failed to read asset: {error}\n");
            write_response(
                &mut stream,
                "500 Internal Server Error",
                TEXT_PLAIN_UTF8,
                body.as_bytes(),
                is_head,
                origin.as_deref(),
            )
        }
    }
}

/// Only ever grant CORS to a request whose `Origin` is itself loopback: the
/// preview tooling that legitimately fetches these assets cross-port (e.g. a
/// webview or a local preview server) always runs on 127.0.0.1/localhost. A
/// page served from a real remote origin gets no CORS header at all, so the
/// browser refuses to let its script read the response.
fn allowed_cors_origin(request: &str) -> Option<String> {
    for line in request.lines().skip(1) {
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("origin") {
            let origin = value.trim();
            return is_loopback_origin(origin).then(|| origin.to_string());
        }
    }
    None
}

fn is_loopback_origin(origin: &str) -> bool {
    const LOOPBACK_HOSTS: &[&str] = &["http://127.0.0.1", "http://localhost", "http://[::1]"];
    LOOPBACK_HOSTS
        .iter()
        .any(|host| origin == *host || origin.starts_with(&format!("{host}:")))
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
    is_head: bool,
    allowed_origin: Option<&str>,
) -> io::Result<()> {
    stream.write_all(
        raw_http_response_head(status, content_type, body.len(), allowed_origin).as_bytes(),
    )?;
    if !is_head {
        stream.write_all(body)?;
    }
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_relative_path_accepts_root_relative_assets() {
        assert_eq!(
            crate::utils::static_files::request_relative_path(
                "/.calepin/paper/figures/fig%2Ddemo.svg?cache=1",
                None,
                false
            )
            .unwrap(),
            PathBuf::from(".calepin")
                .join("paper")
                .join("figures")
                .join("fig-demo.svg")
        );
    }

    #[test]
    fn request_relative_path_rejects_traversal() {
        assert!(
            crate::utils::static_files::request_relative_path("/../secret.txt", None, false)
                .is_none()
        );
        assert!(crate::utils::static_files::request_relative_path(
            "/.calepin/%2e%2e/secret.txt",
            None,
            false
        )
        .is_none());
        assert!(crate::utils::static_files::request_relative_path(
            "/.calepin\\secret.txt",
            None,
            false
        )
        .is_none());
    }

    #[test]
    fn asset_server_serves_root_relative_files() {
        let dir = tempfile::tempdir().unwrap();
        let figures = dir.path().join(".calepin/paper/figures");
        std::fs::create_dir_all(&figures).unwrap();
        std::fs::write(figures.join("fig-demo.svg"), "<svg></svg>").unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let server = start(dir.path().to_path_buf(), Arc::clone(&stop)).unwrap();
        let address = server.base_url().strip_prefix("http://").unwrap();
        let mut stream = TcpStream::connect(address).unwrap();

        stream
            .write_all(b"GET /.calepin/paper/figures/fig-demo.svg HTTP/1.1\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();

        stop.store(true, Ordering::Relaxed);
        server.join();
        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
        assert!(
            response.contains("Content-Type: image/svg+xml"),
            "{response}"
        );
        assert!(response.ends_with("<svg></svg>"), "{response}");
        assert!(
            !response.contains("Access-Control-Allow-Origin: *"),
            "{response}"
        );
    }

    #[test]
    fn asset_server_reflects_only_loopback_origins() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "hi").unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let server = start(dir.path().to_path_buf(), Arc::clone(&stop)).unwrap();
        let address = server.base_url().strip_prefix("http://").unwrap();

        let mut loopback_stream = TcpStream::connect(address).unwrap();
        loopback_stream
            .write_all(b"GET /index.html HTTP/1.1\r\nOrigin: http://127.0.0.1:9999\r\n\r\n")
            .unwrap();
        let mut loopback_response = String::new();
        loopback_stream
            .read_to_string(&mut loopback_response)
            .unwrap();

        let mut remote_stream = TcpStream::connect(address).unwrap();
        remote_stream
            .write_all(b"GET /index.html HTTP/1.1\r\nOrigin: https://evil.example.com\r\n\r\n")
            .unwrap();
        let mut remote_response = String::new();
        remote_stream.read_to_string(&mut remote_response).unwrap();

        stop.store(true, Ordering::Relaxed);
        server.join();

        assert!(
            loopback_response.contains("Access-Control-Allow-Origin: http://127.0.0.1:9999"),
            "{loopback_response}"
        );
        assert!(
            !remote_response.contains("Access-Control-Allow-Origin"),
            "{remote_response}"
        );
    }

    #[test]
    fn asset_server_refuses_sensitive_project_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".calepin")).unwrap();
        std::fs::write(dir.path().join(".calepin/config.toml"), "secret").unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join(".git/config"), "secret").unwrap();
        std::fs::write(dir.path().join(".env"), "SECRET=1").unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let server = start(dir.path().to_path_buf(), Arc::clone(&stop)).unwrap();
        let address = server.base_url().strip_prefix("http://").unwrap();

        for target in ["/.calepin/config.toml", "/.git/config", "/.env"] {
            let mut stream = TcpStream::connect(address).unwrap();
            stream
                .write_all(format!("GET {target} HTTP/1.1\r\n\r\n").as_bytes())
                .unwrap();
            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();
            assert!(response.starts_with("HTTP/1.1 404"), "{target}: {response}");
        }

        stop.store(true, Ordering::Relaxed);
        server.join();
    }
}
