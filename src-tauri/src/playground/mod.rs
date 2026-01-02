pub mod data;
pub mod graphql;
pub mod grpc_server;
pub mod http_server;
pub mod mqtt_broker;
pub mod unix_socket_server;

use data::PlaygroundData;
use grpc_server::GrpcServer;
use http_server::create_http_router;
use mqtt_broker::MqttBroker;
use unix_socket_server::UnixSocketServer;

use hyper::server::conn::http1;
use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, RwLock};

/// Playground port configuration
pub const HTTP_PORT: u16 = 19510;
pub const MQTT_PORT: u16 = 19511;
pub const GRPC_PORT: u16 = 19512;
pub const UNIX_SOCKET_PATH: &str = "/tmp/istek-playground.sock";

/// Global playground manager instance
static PLAYGROUND: Lazy<RwLock<Option<PlaygroundManager>>> = Lazy::new(|| RwLock::new(None));

/// Playground status returned to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaygroundStatus {
    pub running: bool,
    pub http_url: Option<String>,
    pub ws_url: Option<String>,
    pub graphql_url: Option<String>,
    pub mqtt_url: Option<String>,
    pub grpc_url: Option<String>,
    pub unix_socket: Option<String>,
    pub openapi_url: Option<String>,
    pub sse_url: Option<String>,
}

impl PlaygroundStatus {
    pub fn stopped() -> Self {
        Self {
            running: false,
            http_url: None,
            ws_url: None,
            graphql_url: None,
            mqtt_url: None,
            grpc_url: None,
            unix_socket: None,
            openapi_url: None,
            sse_url: None,
        }
    }

    pub fn running() -> Self {
        Self {
            running: true,
            http_url: Some(format!("http://localhost:{}", HTTP_PORT)),
            ws_url: Some(format!("ws://localhost:{}/ws/echo", HTTP_PORT)),
            graphql_url: Some(format!("http://localhost:{}/graphql", HTTP_PORT)),
            mqtt_url: Some(format!("mqtt://localhost:{}", MQTT_PORT)),
            grpc_url: Some(format!("grpc://localhost:{}", GRPC_PORT)),
            unix_socket: if UnixSocketServer::is_supported() {
                Some(UNIX_SOCKET_PATH.to_string())
            } else {
                None
            },
            openapi_url: Some(format!("http://localhost:{}/openapi.json", HTTP_PORT)),
            sse_url: Some(format!("http://localhost:{}/sse/events", HTTP_PORT)),
        }
    }
}

/// Manages all playground servers
struct PlaygroundManager {
    data: Arc<PlaygroundData>,
    http_shutdown: Option<oneshot::Sender<()>>,
    mqtt_broker: Option<MqttBroker>,
    grpc_server: Option<GrpcServer>,
    unix_server: Option<UnixSocketServer>,
}

impl PlaygroundManager {
    fn new() -> Self {
        Self {
            data: PlaygroundData::new(),
            http_shutdown: None,
            mqtt_broker: None,
            grpc_server: None,
            unix_server: None,
        }
    }

    async fn start(&mut self) -> Result<(), String> {
        // Start HTTP server (includes REST, WebSocket, GraphQL)
        self.start_http_server().await?;

        // Start MQTT broker
        self.start_mqtt_broker().await?;

        // Start gRPC server
        self.start_grpc_server().await?;

        // Start Unix socket server (Unix only)
        if UnixSocketServer::is_supported() {
            self.start_unix_socket_server().await?;
        }

        Ok(())
    }

    async fn start_http_server(&mut self) -> Result<(), String> {
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        self.http_shutdown = Some(shutdown_tx);

        let router = create_http_router(self.data.clone());

        let addr = format!("127.0.0.1:{}", HTTP_PORT);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| format!("Failed to bind HTTP server to {}: {}", addr, e))?;

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((stream, _addr)) => {
                                let router = router.clone();
                                tokio::spawn(async move {
                                    let io = TokioIo::new(stream);
                                    let service = TowerToHyperService::new(router);
                                    if let Err(e) = http1::Builder::new()
                                        .serve_connection(io, service)
                                        .with_upgrades()
                                        .await
                                    {
                                        // Ignore connection reset errors (normal for WebSocket)
                                        let err_str: String = e.to_string();
                                    if !err_str.contains("connection reset") {
                                            eprintln!("HTTP connection error: {}", e);
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                eprintln!("HTTP accept error: {}", e);
                            }
                        }
                    }
                    _ = &mut shutdown_rx => {
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    async fn start_mqtt_broker(&mut self) -> Result<(), String> {
        let mut broker = MqttBroker::new(MQTT_PORT);
        broker.start().await?;
        self.mqtt_broker = Some(broker);
        Ok(())
    }

    async fn start_grpc_server(&mut self) -> Result<(), String> {
        let mut server = GrpcServer::new(GRPC_PORT);
        server.start().await?;
        self.grpc_server = Some(server);
        Ok(())
    }

    async fn start_unix_socket_server(&mut self) -> Result<(), String> {
        let mut server = UnixSocketServer::new(UNIX_SOCKET_PATH);
        server.start(self.data.clone()).await?;
        self.unix_server = Some(server);
        Ok(())
    }

    fn stop(&mut self) {
        // Stop HTTP server
        if let Some(tx) = self.http_shutdown.take() {
            let _ = tx.send(());
        }

        // Stop MQTT broker
        if let Some(mut broker) = self.mqtt_broker.take() {
            broker.stop();
        }

        // Stop gRPC server
        if let Some(mut server) = self.grpc_server.take() {
            server.stop();
        }

        // Stop Unix socket server
        if let Some(mut server) = self.unix_server.take() {
            server.stop();
        }
    }
}

// --- Tauri Commands ---

/// Start all playground servers
#[tauri::command]
pub async fn playground_start() -> Result<PlaygroundStatus, String> {
    let mut guard = PLAYGROUND.write().await;

    // Check if already running
    if guard.is_some() {
        return Ok(PlaygroundStatus::running());
    }

    // Create and start playground
    let mut manager = PlaygroundManager::new();
    manager.start().await?;

    *guard = Some(manager);

    Ok(PlaygroundStatus::running())
}

/// Stop all playground servers
#[tauri::command]
pub async fn playground_stop() -> Result<(), String> {
    let mut guard = PLAYGROUND.write().await;

    if let Some(mut manager) = guard.take() {
        manager.stop();
    }

    Ok(())
}

/// Get current playground status
#[tauri::command]
pub async fn playground_status() -> Result<PlaygroundStatus, String> {
    let guard = PLAYGROUND.read().await;

    if guard.is_some() {
        Ok(PlaygroundStatus::running())
    } else {
        Ok(PlaygroundStatus::stopped())
    }
}
