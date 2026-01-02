use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct UnixSocketResponse {
    pub status: u16,
    #[serde(rename = "statusText")]
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub time: u64,
    pub size: usize,
}

// Unix socket is only supported on Unix-like systems (Linux, macOS)
#[cfg(unix)]
#[tauri::command]
pub async fn send_unix_socket_request(
    socket_path: String,
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Option<String>,
) -> Result<UnixSocketResponse, String> {
    use http_body_util::{BodyExt, Full};
    use hyper::body::Bytes;
    use hyper::Request;
    use hyper_util::rt::TokioIo;
    use std::time::Instant;
    use tokio::net::UnixStream;

    let start = Instant::now();

    // Connect to Unix socket
    let stream = UnixStream::connect(&socket_path)
        .await
        .map_err(|e| format!("Failed to connect to socket: {}", e))?;

    let io = TokioIo::new(stream);

    // Create HTTP connection
    let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
        .await
        .map_err(|e| format!("Handshake failed: {}", e))?;

    // Spawn connection handler
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            log::error!("Connection error: {}", e);
        }
    });

    // Build request
    let method = method.parse::<hyper::Method>().map_err(|e| e.to_string())?;
    
    let body_bytes = body.unwrap_or_default();
    let mut req_builder = Request::builder()
        .method(method)
        .uri(&path)
        .header("Host", "localhost");

    // Add headers
    for (key, value) in &headers {
        req_builder = req_builder.header(key.as_str(), value.as_str());
    }

    let request = req_builder
        .body(Full::new(Bytes::from(body_bytes)))
        .map_err(|e| e.to_string())?;

    // Send request
    let response = sender
        .send_request(request)
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let elapsed = start.elapsed().as_millis() as u64;

    // Get status
    let status = response.status().as_u16();
    let status_text = response
        .status()
        .canonical_reason()
        .unwrap_or("Unknown")
        .to_string();

    // Get headers
    let mut response_headers = HashMap::new();
    for (key, value) in response.headers() {
        if let Ok(v) = value.to_str() {
            response_headers.insert(key.to_string(), v.to_string());
        }
    }

    // Get body
    let body_bytes = response
        .into_body()
        .collect()
        .await
        .map_err(|e| e.to_string())?
        .to_bytes();
    
    let size = body_bytes.len();
    let body = String::from_utf8_lossy(&body_bytes).to_string();

    Ok(UnixSocketResponse {
        status,
        status_text,
        headers: response_headers,
        body,
        time: elapsed,
        size,
    })
}

// Windows stub - Unix sockets are not supported
#[cfg(not(unix))]
#[tauri::command]
pub async fn send_unix_socket_request(
    _socket_path: String,
    _method: String,
    _path: String,
    _headers: HashMap<String, String>,
    _body: Option<String>,
) -> Result<UnixSocketResponse, String> {
    Err("Unix sockets are not supported on Windows".to_string())
}
