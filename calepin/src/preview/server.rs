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

/// Start a static file server for a website directory.
/// All HTML responses get the live-reload script injected.
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

            // Resolve the file path with fallbacks
            let rel = url.split('?').next().unwrap_or(&url).trim_start_matches('/');
            let file_path = if rel.is_empty() {
                // / → index.html
                serve_dir.join("index.html")
            } else {
                let direct = serve_dir.join(rel);
                if direct.is_file() {
                    direct
                } else {
                    // Try with .html extension (e.g., /tutorial → tutorial.html)
                    let with_ext = serve_dir.join(format!("{}.html", rel));
                    if with_ext.is_file() {
                        with_ext
                    } else {
                        // Try /dir/index.html
                        let index = direct.join("index.html");
                        if index.is_file() { index } else { direct }
                    }
                }
            };

            if file_path.is_file() {
                match std::fs::read(&file_path) {
                    Ok(data) => {
                        let mime = guess_mime(&file_path);
                        // Inject reload script into HTML responses
                        if mime.starts_with("text/html") {
                            let v = version.load(Ordering::Relaxed);
                            let html = String::from_utf8_lossy(&data);
                            let html = super::reload::inject_reload_script(&html, v);
                            let header = Header::from_bytes("Content-Type", mime).unwrap();
                            let response = Response::from_string(html).with_header(header);
                            let _ = request.respond(response);
                        } else {
                            let header = Header::from_bytes("Content-Type", mime).unwrap();
                            let response = Response::from_data(data).with_header(header);
                            let _ = request.respond(response);
                        }
                    }
                    Err(_) => {
                        let _ = request.respond(
                            Response::from_string("Not found").with_status_code(StatusCode(404)),
                        );
                    }
                }
            } else {
                let _ = request.respond(
                    Response::from_string("Not found").with_status_code(StatusCode(404)),
                );
            }
        }
    });

    Ok((ServerHandle { _server: server }, actual_port))
}

/// Try the requested port, then fall back to nearby ports.
fn try_bind(port: u16) -> Result<(Server, u16)> {
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
                let mime = guess_mime(&file_path);
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

fn guess_mime(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("pdf") => "application/pdf",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}
