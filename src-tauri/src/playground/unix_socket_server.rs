use std::sync::Arc;

#[cfg(unix)]
use {
    hyper::server::conn::http1,
    hyper_util::rt::TokioIo,
    hyper_util::service::TowerToHyperService,
    tokio::net::UnixListener,
    tokio::sync::oneshot,
};

#[cfg(unix)]
use super::data::PlaygroundData;
#[cfg(unix)]
use super::http_server::create_http_router;

/// Unix Socket Server (only on Unix systems)
pub struct UnixSocketServer {
    #[cfg(unix)]
    shutdown_tx: Option<oneshot::Sender<()>>,
    pub socket_path: String,
}

impl UnixSocketServer {
    pub fn new(socket_path: &str) -> Self {
        Self {
            #[cfg(unix)]
            shutdown_tx: None,
            socket_path: socket_path.to_string(),
        }
    }

    /// Start the Unix socket server
    #[cfg(unix)]
    pub async fn start(&mut self, data: Arc<PlaygroundData>) -> Result<(), String> {
        // Remove existing socket file if it exists
        let _ = std::fs::remove_file(&self.socket_path);

        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        let listener = UnixListener::bind(&self.socket_path)
            .map_err(|e| format!("Failed to bind Unix socket: {}", e))?;

        let router = create_http_router(data);
        let socket_path = self.socket_path.clone();

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
                                        .await
                                    {
                                        eprintln!("Unix socket connection error: {}", e);
                                    }
                                });
                            }
                            Err(e) => {
                                eprintln!("Unix socket accept error: {}", e);
                            }
                        }
                    }
                    _ = &mut shutdown_rx => {
                        break;
                    }
                }
            }
            
            // Clean up socket file
            let _ = std::fs::remove_file(&socket_path);
        });

        // Give server time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        Ok(())
    }

    /// Start the Unix socket server (no-op on Windows)
    #[cfg(not(unix))]
    pub async fn start(&mut self, _data: Arc<super::data::PlaygroundData>) -> Result<(), String> {
        Err("Unix sockets are not supported on Windows".to_string())
    }

    /// Stop the Unix socket server
    #[cfg(unix)]
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        // Clean up socket file
        let _ = std::fs::remove_file(&self.socket_path);
    }

    /// Stop the Unix socket server (no-op on Windows)
    #[cfg(not(unix))]
    pub fn stop(&mut self) {
        // No-op on Windows
    }

    /// Check if Unix sockets are supported
    pub fn is_supported() -> bool {
        cfg!(unix)
    }
}

impl Drop for UnixSocketServer {
    fn drop(&mut self) {
        self.stop();
    }
}
