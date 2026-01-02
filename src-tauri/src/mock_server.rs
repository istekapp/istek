use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, Method, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use chrono::Utc;
use dashmap::DashMap;
use hyper::server::conn::http1;
use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use tower_http::cors::{Any as CorsAny, CorsLayer};

// Global mock server state
static MOCK_SERVERS: Lazy<DashMap<String, MockServerHandle>> = Lazy::new(DashMap::new);

struct MockServerHandle {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MockEndpoint {
    pub id: String,
    pub method: String,
    pub path: String,           // Can include path params like /users/:id
    pub response_status: u16,
    pub response_headers: HashMap<String, String>,
    pub response_body: String,
    pub delay_ms: Option<u64>,  // Optional delay to simulate slow responses
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MockServerConfig {
    pub id: String,
    pub name: String,
    pub port: u16,
    pub endpoints: Vec<MockEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MockServerInfo {
    pub id: String,
    pub name: String,
    pub port: u16,
    pub endpoint_count: usize,
    pub running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MockRequestLog {
    pub id: String,
    pub server_id: String,
    pub timestamp: i64,
    pub method: String,
    pub path: String,
    pub query: Option<String>,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub matched_endpoint: Option<String>,
    pub response_status: u16,
    pub response_time_ms: u64,
}

// Shared state for the mock server
struct MockServerState {
    config: MockServerConfig,
    app_handle: AppHandle,
    logs: RwLock<Vec<MockRequestLog>>,
}

fn generate_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

// Convert path pattern to regex (e.g., /users/:id -> /users/([^/]+))
fn path_to_regex(path: &str) -> Regex {
    let pattern = path
        .split('/')
        .map(|segment| {
            if segment.starts_with(':') {
                "([^/]+)".to_string()
            } else if segment.starts_with('{') && segment.ends_with('}') {
                "([^/]+)".to_string()
            } else {
                regex::escape(segment)
            }
        })
        .collect::<Vec<_>>()
        .join("/");
    
    Regex::new(&format!("^{}$", pattern)).unwrap_or_else(|_| Regex::new("^$").unwrap())
}

// Find matching endpoint for a request
fn find_matching_endpoint(
    endpoints: &[MockEndpoint],
    method: &Method,
    path: &str,
) -> Option<MockEndpoint> {
    for endpoint in endpoints {
        if endpoint.method.to_uppercase() == method.as_str() {
            let regex = path_to_regex(&endpoint.path);
            if regex.is_match(path) {
                return Some(endpoint.clone());
            }
        }
    }
    None
}

// Handler for all mock requests
async fn mock_handler(
    State(state): State<Arc<MockServerState>>,
    method: Method,
    headers: HeaderMap,
    Path(path): Path<String>,
    request: Request<Body>,
) -> Response {
    let start_time = std::time::Instant::now();
    let path_with_slash = format!("/{}", path);
    let query = request.uri().query().map(|s| s.to_string());
    
    // Read request body
    let body_bytes = axum::body::to_bytes(request.into_body(), usize::MAX)
        .await
        .unwrap_or_default();
    let body_str = String::from_utf8_lossy(&body_bytes).to_string();
    
    // Convert headers to HashMap
    let headers_map: HashMap<String, String> = headers
        .iter()
        .filter_map(|(k, v)| {
            v.to_str().ok().map(|v| (k.to_string(), v.to_string()))
        })
        .collect();
    
    // Find matching endpoint
    let matched = find_matching_endpoint(&state.config.endpoints, &method, &path_with_slash);
    
    let (response_status, response_body, matched_id) = match &matched {
        Some(endpoint) => {
            // Apply delay if configured
            if let Some(delay) = endpoint.delay_ms {
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }
            
            (
                StatusCode::from_u16(endpoint.response_status).unwrap_or(StatusCode::OK),
                endpoint.response_body.clone(),
                Some(endpoint.id.clone()),
            )
        }
        None => (
            StatusCode::NOT_FOUND,
            format!(
                r#"{{"error": "No mock endpoint found", "method": "{}", "path": "{}"}}"#,
                method, path_with_slash
            ),
            None,
        ),
    };
    
    let elapsed = start_time.elapsed().as_millis() as u64;
    
    // Create log entry
    let log = MockRequestLog {
        id: generate_id(),
        server_id: state.config.id.clone(),
        timestamp: Utc::now().timestamp_millis(),
        method: method.to_string(),
        path: path_with_slash.clone(),
        query,
        headers: headers_map,
        body: if body_str.is_empty() { None } else { Some(body_str) },
        matched_endpoint: matched_id,
        response_status: response_status.as_u16(),
        response_time_ms: elapsed,
    };
    
    // Store log
    {
        let mut logs = state.logs.write().await;
        logs.push(log.clone());
        // Keep only last 100 logs
        if logs.len() > 100 {
            logs.remove(0);
        }
    }
    
    // Emit event to frontend
    let _ = state.app_handle.emit("mock-request-log", &log);
    
    // Build response
    let mut response_builder = Response::builder().status(response_status);
    
    // Add response headers from endpoint config
    if let Some(endpoint) = matched {
        for (key, value) in &endpoint.response_headers {
            response_builder = response_builder.header(key.as_str(), value.as_str());
        }
    }
    
    // Add default content-type if not set
    response_builder = response_builder.header("content-type", "application/json");
    response_builder = response_builder.header("x-mock-server", "istek");
    
    response_builder
        .body(Body::from(response_body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

// Root handler
async fn root_handler(
    State(state): State<Arc<MockServerState>>,
    method: Method,
    headers: HeaderMap,
    request: Request<Body>,
) -> Response {
    let start_time = std::time::Instant::now();
    let query = request.uri().query().map(|s| s.to_string());
    
    // Read request body
    let body_bytes = axum::body::to_bytes(request.into_body(), usize::MAX)
        .await
        .unwrap_or_default();
    let body_str = String::from_utf8_lossy(&body_bytes).to_string();
    
    // Convert headers to HashMap
    let headers_map: HashMap<String, String> = headers
        .iter()
        .filter_map(|(k, v)| {
            v.to_str().ok().map(|v| (k.to_string(), v.to_string()))
        })
        .collect();
    
    // Find matching endpoint for root
    let matched = find_matching_endpoint(&state.config.endpoints, &method, "/");
    
    let (response_status, response_body, matched_id) = match &matched {
        Some(endpoint) => {
            if let Some(delay) = endpoint.delay_ms {
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }
            (
                StatusCode::from_u16(endpoint.response_status).unwrap_or(StatusCode::OK),
                endpoint.response_body.clone(),
                Some(endpoint.id.clone()),
            )
        }
        None => (
            StatusCode::OK,
            format!(
                r#"{{"message": "Istek Mock Server", "name": "{}", "endpoints": {}}}"#,
                state.config.name,
                state.config.endpoints.len()
            ),
            None,
        ),
    };
    
    let elapsed = start_time.elapsed().as_millis() as u64;
    
    // Create log entry
    let log = MockRequestLog {
        id: generate_id(),
        server_id: state.config.id.clone(),
        timestamp: Utc::now().timestamp_millis(),
        method: method.to_string(),
        path: "/".to_string(),
        query,
        headers: headers_map,
        body: if body_str.is_empty() { None } else { Some(body_str) },
        matched_endpoint: matched_id,
        response_status: response_status.as_u16(),
        response_time_ms: elapsed,
    };
    
    {
        let mut logs = state.logs.write().await;
        logs.push(log.clone());
        if logs.len() > 100 {
            logs.remove(0);
        }
    }
    
    let _ = state.app_handle.emit("mock-request-log", &log);
    
    let mut response_builder = Response::builder().status(response_status);
    
    if let Some(endpoint) = matched {
        for (key, value) in &endpoint.response_headers {
            response_builder = response_builder.header(key.as_str(), value.as_str());
        }
    }
    
    response_builder = response_builder.header("content-type", "application/json");
    response_builder = response_builder.header("x-mock-server", "istek");
    
    response_builder
        .body(Body::from(response_body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[tauri::command]
pub async fn mock_server_start(
    app: AppHandle,
    config: MockServerConfig,
) -> Result<MockServerInfo, String> {
    // Check if server with this ID is already running
    if MOCK_SERVERS.contains_key(&config.id) {
        return Err(format!("Mock server {} is already running", config.id));
    }
    
    let port = config.port;
    let server_id = config.id.clone();
    let server_name = config.name.clone();
    let endpoint_count = config.endpoints.len();
    
    let state = Arc::new(MockServerState {
        config: config.clone(),
        app_handle: app,
        logs: RwLock::new(Vec::new()),
    });
    
    // Build router with CORS support
    let cors = CorsLayer::new()
        .allow_origin(CorsAny)
        .allow_methods(CorsAny)
        .allow_headers(CorsAny);
    
    let router = Router::new()
        .route("/", any(root_handler))
        .route("/*path", any(mock_handler))
        .layer(cors)
        .with_state(state);
    
    // Create shutdown channel
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    
    // Bind to port
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Failed to bind to port {}: {}", port, e))?;
    
    // Spawn server task using HTTP/1.1 explicitly
    tokio::spawn(async move {
        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _addr)) => {
                            let tower_service = router.clone();
                            tokio::spawn(async move {
                                let io = TokioIo::new(stream);
                                let hyper_service = TowerToHyperService::new(tower_service);
                                if let Err(e) = http1::Builder::new()
                                    .serve_connection(io, hyper_service)
                                    .await
                                {
                                    eprintln!("Error serving connection: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            eprintln!("Error accepting connection: {}", e);
                        }
                    }
                }
                _ = &mut shutdown_rx => {
                    break;
                }
            }
        }
    });
    
    // Store server handle
    MOCK_SERVERS.insert(
        server_id.clone(),
        MockServerHandle { shutdown_tx, port },
    );
    
    Ok(MockServerInfo {
        id: server_id,
        name: server_name,
        port,
        endpoint_count,
        running: true,
    })
}

#[tauri::command]
pub async fn mock_server_stop(server_id: String) -> Result<(), String> {
    if let Some((_, handle)) = MOCK_SERVERS.remove(&server_id) {
        let _ = handle.shutdown_tx.send(());
        Ok(())
    } else {
        Err(format!("Mock server {} not found", server_id))
    }
}

#[tauri::command]
pub async fn mock_server_list() -> Result<Vec<MockServerInfo>, String> {
    let servers: Vec<MockServerInfo> = MOCK_SERVERS
        .iter()
        .map(|entry| MockServerInfo {
            id: entry.key().clone(),
            name: entry.key().clone(), // We don't store name separately
            port: entry.value().port,
            endpoint_count: 0,
            running: true,
        })
        .collect();
    
    Ok(servers)
}

#[tauri::command]
pub async fn mock_server_stop_all() -> Result<(), String> {
    let ids: Vec<String> = MOCK_SERVERS.iter().map(|e| e.key().clone()).collect();
    
    for id in ids {
        if let Some((_, handle)) = MOCK_SERVERS.remove(&id) {
            let _ = handle.shutdown_tx.send(());
        }
    }
    
    Ok(())
}

// Helper to create mock endpoints from a collection request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMockFromRequest {
    pub method: String,
    pub url: String,
    pub response_status: u16,
    pub response_body: String,
    pub response_headers: HashMap<String, String>,
}

#[tauri::command]
pub fn create_mock_endpoint(request: CreateMockFromRequest) -> Result<MockEndpoint, String> {
    // Parse URL to get path
    let path = if request.url.starts_with("http") {
        url::Url::parse(&request.url)
            .map(|u| u.path().to_string())
            .unwrap_or("/".to_string())
    } else {
        request.url.clone()
    };
    
    Ok(MockEndpoint {
        id: generate_id(),
        method: request.method.to_uppercase(),
        path,
        response_status: request.response_status,
        response_headers: request.response_headers,
        response_body: request.response_body,
        delay_ms: None,
    })
}
