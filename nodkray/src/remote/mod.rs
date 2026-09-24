//! Optional Control API (spec §86-§88).
//!
//! Disabled by default. When started it binds loopback, requires a bearer
//! token from the environment, and only translates HTTP into
//! [`service::ControlService`] calls. No business logic lives here.

pub mod service;

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serde::Serialize;

use crate::error::{NodkrayError, NodkrayResult};

pub use service::{
    ControlService, CoreControlService, CreateTaskRequest, MemoryControlService, StatusPayload,
};

/// How the control API should listen.
#[derive(Debug, Clone)]
pub struct ServeOptions {
    pub bind: String,
    pub token: String,
}

impl ServeOptions {
    /// Validate bind + token before opening a socket.
    pub fn validate(&self) -> NodkrayResult<SocketAddr> {
        if self.token.trim().is_empty() {
            return Err(NodkrayError::permission(
                "AUTH_TOKEN_MISSING",
                "set NODKRAY_REMOTE_TOKEN before starting the control API",
            ));
        }
        let addr: SocketAddr = self.bind.parse().map_err(|_| {
            NodkrayError::configuration(
                "REMOTE_BIND_INVALID",
                format!("invalid remote.bind `{}`", self.bind),
            )
        })?;
        if !addr.ip().is_loopback() && addr.ip() != IpAddr::from([0, 0, 0, 0]) {
            return Err(NodkrayError::permission(
                "REMOTE_BIND_REFUSED",
                "control API may only bind loopback in V1 (127.0.0.1 / ::1)",
            ));
        }
        if addr.ip() == IpAddr::from([0, 0, 0, 0]) {
            return Err(NodkrayError::permission(
                "REMOTE_BIND_REFUSED",
                "refusing 0.0.0.0; bind 127.0.0.1:8787",
            ));
        }
        Ok(addr)
    }
}

/// Handle for a running control server (tests + `nodkray serve`).
pub struct ControlServer {
    listener: TcpListener,
    token: String,
    service: Arc<dyn ControlService>,
    shutdown: Arc<AtomicBool>,
}

impl ControlServer {
    /// Bind and return the server. Does not accept connections yet.
    pub fn bind(options: ServeOptions, service: Arc<dyn ControlService>) -> NodkrayResult<Self> {
        let addr = options.validate()?;
        let listener = TcpListener::bind(addr).map_err(|err| {
            NodkrayError::network("REMOTE_BIND_FAILED", err.to_string())
        })?;
        listener.set_nonblocking(true).ok();
        Ok(Self {
            listener,
            token: options.token,
            service,
            shutdown: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn local_addr(&self) -> NodkrayResult<SocketAddr> {
        self.listener.local_addr().map_err(|err| {
            NodkrayError::network("REMOTE_BIND_FAILED", err.to_string())
        })
    }

    pub fn shutdown_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.shutdown)
    }

    /// Serve until `shutdown` is set or the listener fails.
    pub fn serve(&self) -> NodkrayResult<()> {
        while !self.shutdown.load(Ordering::Relaxed) {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    let token = self.token.clone();
                    let service = Arc::clone(&self.service);
                    thread::spawn(move || {
                        if let Err(error) = handle_connection(stream, &token, service.as_ref()) {
                            tracing::warn!(
                                code = error.code(),
                                message = error.message(),
                                "control API request failed"
                            );
                        }
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(20));
                }
                Err(err) => {
                    return Err(NodkrayError::network("REMOTE_ACCEPT_FAILED", err.to_string()));
                }
            }
        }
        Ok(())
    }
}

/// Blocking serve used by `nodkray serve`.
pub fn serve(options: ServeOptions, service: Arc<dyn ControlService>) -> NodkrayResult<()> {
    let server = ControlServer::bind(options, service)?;
    tracing::info!(addr = %server.local_addr()?, "control API listening");
    server.serve()
}

struct HttpRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl HttpRequest {
    fn header(&self, name: &str) -> Option<&str> {
        let needle = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(key, _)| key.to_ascii_lowercase() == needle)
            .map(|(_, value)| value.as_str())
    }
}

fn handle_connection(
    mut stream: TcpStream,
    token: &str,
    service: &dyn ControlService,
) -> NodkrayResult<()> {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok();
    let request = read_request(&mut stream)?;
    if !authorized(&request, token) {
        let error = NodkrayError::permission(
            "AUTH_FAILED",
            "missing or invalid control API token",
        );
        return write_error(&mut stream, 401, &error);
    }
    let (status, body, content_type) = dispatch(&request, service);
    write_response(&mut stream, status, content_type, &body)
}

fn authorized(request: &HttpRequest, token: &str) -> bool {
    if let Some(value) = request.header("authorization") {
        let expected = format!("Bearer {token}");
        if value == expected || value == token {
            return true;
        }
    }
    request.header("x-nodkray-token") == Some(token)
}

fn dispatch(request: &HttpRequest, service: &dyn ControlService) -> (u16, String, &'static str) {
    let path = request.path.split('?').next().unwrap_or(&request.path);
    match (request.method.as_str(), path) {
        ("GET", "/health") => json(200, serde_json::json!({ "ok": true })),
        ("GET", "/status") => map_result(service.status()),
        ("GET", "/workers") => map_result(service.list_workers()),
        ("GET", "/reviews") => map_result(service.list_reviews()),
        ("GET", "/events") => map_result(service.events(100)),
        ("GET", "/events/stream") => match service.events(100) {
            Ok(events) => {
                let mut sse = String::new();
                for event in events {
                    if let Ok(data) = serde_json::to_string(&event) {
                        sse.push_str("data: ");
                        sse.push_str(&data);
                        sse.push_str("\n\n");
                    }
                }
                (200, sse, "text/event-stream")
            }
            Err(error) => error_body(&error),
        },
        ("POST", "/tasks") => {
            let parsed: CreateTaskRequest = match serde_json::from_str(&request.body) {
                Ok(body) => body,
                Err(err) => {
                    return error_body(&NodkrayError::user_input(
                        "INVALID_JSON",
                        format!("invalid task body: {err}"),
                    ));
                }
            };
            map_result(service.create_task(parsed))
        }
        (method, path) if path.starts_with("/tasks/") => {
            let rest = &path["/tasks/".len()..];
            if let Some(id) = rest.strip_suffix("/cancel") {
                if method == "POST" {
                    return map_result(service.cancel_task(id));
                }
            }
            if method == "GET" && !rest.is_empty() && !rest.contains('/') {
                return map_result(service.get_task(rest));
            }
            error_body(&NodkrayError::user_input(
                "NOT_FOUND",
                format!("unknown control API path {path}"),
            ))
        }
        (_, path) => error_body(&NodkrayError::user_input(
            "NOT_FOUND",
            format!("unknown control API path {path}"),
        )),
    }
}

fn map_result<T: Serialize>(result: NodkrayResult<T>) -> (u16, String, &'static str) {
    match result {
        Ok(value) => json(200, value),
        Err(error) => error_body(&error),
    }
}

fn json(status: u16, value: impl Serialize) -> (u16, String, &'static str) {
    (
        status,
        serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string()),
        "application/json",
    )
}

fn error_body(error: &NodkrayError) -> (u16, String, &'static str) {
    let status = match error.code() {
        "AUTH_FAILED" | "AUTH_TOKEN_MISSING" => 401,
        "TASK_NOT_FOUND" | "NOT_FOUND" => 404,
        _ => 400,
    };
    (
        status,
        serde_json::to_string(&error.to_json()).unwrap_or_else(|_| "{}".to_string()),
        "application/json",
    )
}

fn read_request(stream: &mut TcpStream) -> NodkrayResult<HttpRequest> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let n = stream.read(&mut chunk).map_err(|err| {
            NodkrayError::network("REMOTE_READ_FAILED", err.to_string())
        })?;
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..n]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if buffer.len() > 64 * 1024 {
            return Err(NodkrayError::user_input(
                "REQUEST_TOO_LARGE",
                "control API request headers are too large",
            ));
        }
    }
    let raw = String::from_utf8_lossy(&buffer);
    let (head, rest) = raw.split_once("\r\n\r\n").ok_or_else(|| {
        NodkrayError::user_input("BAD_REQUEST", "malformed HTTP request")
    })?;
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("/").to_string();
    let mut headers = Vec::new();
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            headers.push((key.trim().to_string(), value.trim().to_string()));
        }
    }
    let mut body = rest.to_string();
    let content_length = headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    while body.len() < content_length {
        let n = stream.read(&mut chunk).map_err(|err| {
            NodkrayError::network("REMOTE_READ_FAILED", err.to_string())
        })?;
        if n == 0 {
            break;
        }
        body.push_str(&String::from_utf8_lossy(&chunk[..n]));
    }
    if content_length > 0 {
        body.truncate(content_length);
    }
    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

fn write_error(stream: &mut TcpStream, status: u16, error: &NodkrayError) -> NodkrayResult<()> {
    let body = serde_json::to_string(&error.to_json()).unwrap_or_else(|_| "{}".to_string());
    write_response(stream, status, "application/json", &body)
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &str,
) -> NodkrayResult<()> {
    let reason = match status {
        200 => "OK",
        401 => "Unauthorized",
        404 => "Not Found",
        _ => "Bad Request",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {len}\r\nConnection: close\r\nCache-Control: no-cache\r\n\r\n{body}",
        len = body.len(),
    );
    stream.write_all(response.as_bytes()).map_err(|err| {
        NodkrayError::network("REMOTE_WRITE_FAILED", err.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::task::{Task, TaskEvent, TaskStatus};
    use crate::memory::Worker;
    use std::io::{Read, Write};
    use std::net::TcpStream;

    fn start(service: MemoryControlService) -> (ControlServer, SocketAddr, Arc<AtomicBool>) {
        let server = ControlServer::bind(
            ServeOptions {
                bind: "127.0.0.1:0".to_string(),
                token: "secret".to_string(),
            },
            Arc::new(service),
        )
        .expect("bind");
        let addr = server.local_addr().expect("addr");
        let shutdown = server.shutdown_handle();
        (server, addr, shutdown)
    }

    fn spawn_server(server: ControlServer) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let _ = server.serve();
        })
    }

    fn request(addr: SocketAddr, raw: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(addr).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .ok();
        stream.write_all(raw.as_bytes()).expect("write");
        let mut body = String::new();
        stream.read_to_string(&mut body).ok();
        let status = body
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .unwrap_or(0);
        let payload = body
            .split("\r\n\r\n")
            .nth(1)
            .unwrap_or_default()
            .to_string();
        (status, payload)
    }

    #[test]
    fn defaults_are_disabled_and_loopback() {
        let config = crate::config::Config::default();
        assert!(!config.remote.enabled);
        assert_eq!(config.remote.bind, "127.0.0.1:8787");
    }

    #[test]
    fn refuses_non_loopback_bind() {
        let err = ServeOptions {
            bind: "8.8.8.8:8787".to_string(),
            token: "secret".to_string(),
        }
        .validate()
        .expect_err("refused");
        assert_eq!(err.code(), "REMOTE_BIND_REFUSED");
    }

    #[test]
    fn missing_token_is_auth_error() {
        let err = ServeOptions {
            bind: "127.0.0.1:8787".to_string(),
            token: String::new(),
        }
        .validate()
        .expect_err("token");
        assert_eq!(err.code(), "AUTH_TOKEN_MISSING");
        assert_eq!(err.exit_code(), 10);
    }

    #[test]
    fn unauthenticated_requests_are_rejected() {
        let service = MemoryControlService::default();
        let (server, addr, shutdown) = start(service);
        let handle = spawn_server(server);
        let (status, body) = request(
            addr,
            "GET /status HTTP/1.1\r\nHost: localhost\r\n\r\n",
        );
        shutdown.store(true, Ordering::Relaxed);
        let _ = handle.join();
        assert_eq!(status, 401);
        assert!(body.contains("AUTH_FAILED"));
    }

    #[test]
    fn authenticated_status_task_cancel_and_events() {
        let service = MemoryControlService::default();
        service.seed_task(Task {
            id: "task_seed".to_string(),
            project_id: "p".to_string(),
            session_id: None,
            parent_task_id: None,
            title: "seed".to_string(),
            description: "seed".to_string(),
            workflow: "st".to_string(),
            effort: None,
            role: None,
            status: TaskStatus::Pending,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        });
        service.events.lock().expect("lock").push(TaskEvent {
            id: "event_1".to_string(),
            task_id: "task_seed".to_string(),
            event: "task.created".to_string(),
            payload: None,
            created_at: "now".to_string(),
        });
        service.workers.lock().expect("lock").push(Worker {
            id: "worker_1".to_string(),
            task_id: "task_seed".to_string(),
            role: "default".to_string(),
            agent: "generic".to_string(),
            worktree_path: None,
            status: "running".to_string(),
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        });

        let (server, addr, shutdown) = start(service);
        let handle = spawn_server(server);
        let auth = "Authorization: Bearer secret\r\n";

        let (status, body) = request(
            addr,
            &format!("GET /status HTTP/1.1\r\nHost: localhost\r\n{auth}\r\n"),
        );
        assert_eq!(status, 200, "{body}");
        assert!(body.contains("task_seed"));

        let create = "{\"description\":\"fix typo in readme\"}";
        let (status, body) = request(
            addr,
            &format!(
                "POST /tasks HTTP/1.1\r\nHost: localhost\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{create}",
                create.len()
            ),
        );
        assert_eq!(status, 200, "{body}");
        let created: serde_json::Value = serde_json::from_str(&body).expect("create json");
        assert!(
            created["task"]["id"]
                .as_str()
                .unwrap_or_default()
                .starts_with("task_"),
            "{body}"
        );
        assert_eq!(created["task"]["title"], "fix typo in readme");

        let (status, body) = request(
            addr,
            &format!("GET /tasks/task_seed HTTP/1.1\r\nHost: localhost\r\n{auth}\r\n"),
        );
        assert_eq!(status, 200, "{body}");
        assert!(body.contains("task.created"));

        let (status, body) = request(
            addr,
            &format!("GET /workers HTTP/1.1\r\nHost: localhost\r\n{auth}\r\n"),
        );
        assert_eq!(status, 200, "{body}");
        assert!(body.contains("worker_1"));

        let (status, body) = request(
            addr,
            &format!("POST /tasks/task_seed/cancel HTTP/1.1\r\nHost: localhost\r\n{auth}\r\n"),
        );
        assert_eq!(status, 200, "{body}");
        assert!(body.contains("CANCELLED"));

        let (status, body) = request(
            addr,
            &format!("GET /events/stream HTTP/1.1\r\nHost: localhost\r\n{auth}\r\n"),
        );
        assert_eq!(status, 200, "{body}");
        assert!(body.contains("data: "));
        assert!(body.contains("task.created"));

        shutdown.store(true, Ordering::Relaxed);
        let _ = handle.join();
    }
}
