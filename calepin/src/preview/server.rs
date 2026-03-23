use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;

use anyhow::Result;
use tiny_http::{Header, Response, Server, StatusCode};

pub struct ServerHandle {
    _server: Arc<Server>,
}

pub fn start(
    port: u16,
    content: Arc<RwLock<String>>,
    version: Arc<AtomicU64>,
    serve_dir: PathBuf,
) -> Result<(ServerHandle, u16)> {
    let (server, actual_port) = try_bind(port)?;
    let server = Arc::new(server);

    let server_clone = Arc::clone(&server);
    thread::spawn(move || {
        for request in server_clone.incoming_requests() {
            let url = request.url().to_string();

            if url == "/__version" {
                let v = version.load(Ordering::Relaxed).to_string();
                let _ = request.respond(Response::from_string(v));
            } else if url == "/" {
                let html = content.read().unwrap().clone();
                let header =
                    Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap();
                let response = Response::from_string(html).with_header(header);
                let _ = request.respond(response);
            } else {
                // Serve static files relative to the document directory
                serve_static(request, &url, &serve_dir);
            }
        }
    });

    Ok((ServerHandle { _server: server }, actual_port))
}

/// Start an HTTP server that serves files from a directory on disk, with
/// live-reload support via `/__version` endpoint and reload script injection.
pub fn start_site(
    port: u16,
    version: Arc<AtomicU64>,
    serve_dir: PathBuf,
) -> Result<(ServerHandle, u16)> {
    let (server, actual_port) = try_bind(port)?;
    let server = Arc::new(server);

    let server_clone = Arc::clone(&server);
    thread::spawn(move || {
        for request in server_clone.incoming_requests() {
            let url = request.url().to_string();

            if url == "/__version" {
                let v = version.load(Ordering::Relaxed).to_string();
                let _ = request.respond(Response::from_string(v));
                continue;
            }

            let rel = url.split('?').next().unwrap_or(&url).trim_start_matches('/');
            let mut file_path = serve_dir.join(rel);
            if file_path.is_dir() {
                file_path = file_path.join("index.html");
            }

            let data = if file_path.is_file() {
                std::fs::read(&file_path).ok()
            } else {
                None
            };

            if let Some(data) = data {
                let mime = resolve_mime(&file_path);
                if mime.starts_with("text/html") {
                    let html = String::from_utf8_lossy(&data);
                    let v = version.load(Ordering::Relaxed);
                    let html = super::reload::inject_reload_script(&html, v);
                    let header = Header::from_bytes("Content-Type", mime).unwrap();
                    let _ = request.respond(Response::from_string(html).with_header(header));
                } else {
                    let header = Header::from_bytes("Content-Type", mime).unwrap();
                    let _ = request.respond(Response::from_data(data).with_header(header));
                }
            } else {
                // Serve 404.html if it exists, otherwise plain text
                let page_404 = serve_dir.join("404.html");
                if let Ok(body) = std::fs::read_to_string(&page_404) {
                    let v = version.load(Ordering::Relaxed);
                    let html = super::reload::inject_reload_script(&body, v);
                    let header = Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap();
                    let _ = request.respond(Response::from_string(html).with_header(header).with_status_code(StatusCode(404)));
                } else {
                    let _ = request.respond(Response::from_string("Not found").with_status_code(StatusCode(404)));
                }
            }
        }
    });

    Ok((ServerHandle { _server: server }, actual_port))
}

/// Try the requested port, then fall back to nearby ports.
pub(crate) fn try_bind(port: u16) -> Result<(Server, u16)> {
    // Try the requested port first
    if let Ok(server) = Server::http(format!("0.0.0.0:{}", port)) {
        return Ok((server, port));
    }

    // Try the next 10 ports
    for p in (port + 1)..=(port + 10) {
        if let Ok(server) = Server::http(format!("0.0.0.0:{}", p)) {
            eprintln!(
                "\x1b[33mWarning:\x1b[0m port {} in use, using {} instead",
                port, p
            );
            return Ok((server, p));
        }
    }

    anyhow::bail!("Could not find an available port in range {}–{}", port, port + 10)
}

fn serve_static(request: tiny_http::Request, url: &str, serve_dir: &PathBuf) {
    let rel_path = url.split('?').next().unwrap_or(url).trim_start_matches('/');
    let file_path = serve_dir.join(rel_path);
    if file_path.is_file() {
        match std::fs::read(&file_path) {
            Ok(data) => {
                let mime = resolve_mime(&file_path);
                let header = Header::from_bytes("Content-Type", mime).unwrap();
                let response = Response::from_data(data).with_header(header);
                let _ = request.respond(response);
            }
            Err(_) => {
                let _ = request.respond(
                    Response::from_string("Not found").with_status_code(StatusCode(404)),
                );
            }
        }
    } else {
        let _ =
            request.respond(Response::from_string("Not found").with_status_code(StatusCode(404)));
    }
}

pub(crate) fn resolve_mime(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("pdf") => "application/pdf",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("qmd") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
