use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use once_cell::sync::Lazy;
use dashmap::DashMap;

// Store active SSE connections
pub static SSE_CONNECTIONS: Lazy<DashMap<String, mpsc::Sender<()>>> = Lazy::new(DashMap::new);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseEvent {
    pub id: String,
    #[serde(rename = "eventId")]
    pub event_id: Option<String>,
    #[serde(rename = "eventType")]
    pub event_type: String,
    pub data: String,
    pub timestamp: u64,
    pub retry: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SseConnectionEvent {
    #[serde(rename = "connectionId")]
    pub connection_id: String,
    pub connected: bool,
    pub error: Option<String>,
}

/// Connect to an SSE endpoint
#[tauri::command]
pub async fn sse_connect(
    app: AppHandle,
    connection_id: String,
    url: String,
    headers: HashMap<String, String>,
) -> Result<(), String> {
    // Check if already connected
    if SSE_CONNECTIONS.contains_key(&connection_id) {
        return Err("Already connected".to_string());
    }

    // Create a channel to signal disconnection
    let (tx, mut rx) = mpsc::channel::<()>(1);
    SSE_CONNECTIONS.insert(connection_id.clone(), tx);

    let conn_id = connection_id.clone();
    let app_handle = app.clone();

    // Spawn the SSE listener task
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        
        let mut request = client.get(&url);
        for (key, value) in headers {
            request = request.header(&key, &value);
        }
        // Set SSE specific headers
        request = request.header("Accept", "text/event-stream");
        request = request.header("Cache-Control", "no-cache");

        match request.send().await {
            Ok(response) => {
                if !response.status().is_success() {
                    let _ = app_handle.emit("sse-connection", SseConnectionEvent {
                        connection_id: conn_id.clone(),
                        connected: false,
                        error: Some(format!("HTTP {}", response.status())),
                    });
                    SSE_CONNECTIONS.remove(&conn_id);
                    return;
                }

                // Emit connected event
                let _ = app_handle.emit("sse-connection", SseConnectionEvent {
                    connection_id: conn_id.clone(),
                    connected: true,
                    error: None,
                });

                // Read the SSE stream
                let mut stream = response.bytes_stream();
                use futures_util::StreamExt;
                
                let mut buffer = String::new();
                let mut event_type = String::from("message");
                let mut event_id: Option<String> = None;
                let mut event_data = String::new();
                let mut retry: Option<u64> = None;

                loop {
                    tokio::select! {
                        _ = rx.recv() => {
                            // Disconnect signal received
                            break;
                        }
                        chunk = stream.next() => {
                            match chunk {
                                Some(Ok(bytes)) => {
                                    buffer.push_str(&String::from_utf8_lossy(&bytes));
                                    
                                    // Process complete lines
                                    while let Some(pos) = buffer.find('\n') {
                                        let line = buffer[..pos].trim_end_matches('\r').to_string();
                                        buffer = buffer[pos + 1..].to_string();
                                        
                                        if line.is_empty() {
                                            // Empty line = dispatch event
                                            if !event_data.is_empty() {
                                                let event = SseEvent {
                                                    id: uuid::Uuid::new_v4().to_string(),
                                                    event_id: event_id.clone(),
                                                    event_type: event_type.clone(),
                                                    data: event_data.trim_end().to_string(),
                                                    timestamp: std::time::SystemTime::now()
                                                        .duration_since(std::time::UNIX_EPOCH)
                                                        .unwrap()
                                                        .as_millis() as u64,
                                                    retry,
                                                };
                                                
                                                let _ = app_handle.emit(&format!("sse-event-{}", conn_id), event);
                                            }
                                            
                                            // Reset for next event
                                            event_type = String::from("message");
                                            event_id = None;
                                            event_data = String::new();
                                            retry = None;
                                        } else if line.starts_with(':') {
                                            // Comment, ignore
                                        } else if let Some(value) = line.strip_prefix("event:") {
                                            event_type = value.trim().to_string();
                                        } else if let Some(value) = line.strip_prefix("data:") {
                                            if !event_data.is_empty() {
                                                event_data.push('\n');
                                            }
                                            event_data.push_str(value.trim_start());
                                        } else if let Some(value) = line.strip_prefix("id:") {
                                            event_id = Some(value.trim().to_string());
                                        } else if let Some(value) = line.strip_prefix("retry:") {
                                            retry = value.trim().parse().ok();
                                        }
                                    }
                                }
                                Some(Err(e)) => {
                                    let _ = app_handle.emit("sse-connection", SseConnectionEvent {
                                        connection_id: conn_id.clone(),
                                        connected: false,
                                        error: Some(e.to_string()),
                                    });
                                    break;
                                }
                                None => {
                                    // Stream ended
                                    let _ = app_handle.emit("sse-connection", SseConnectionEvent {
                                        connection_id: conn_id.clone(),
                                        connected: false,
                                        error: Some("Stream ended".to_string()),
                                    });
                                    break;
                                }
                            }
                        }
                    }
                }

                SSE_CONNECTIONS.remove(&conn_id);
            }
            Err(e) => {
                let _ = app_handle.emit("sse-connection", SseConnectionEvent {
                    connection_id: conn_id.clone(),
                    connected: false,
                    error: Some(e.to_string()),
                });
                SSE_CONNECTIONS.remove(&conn_id);
            }
        }
    });

    Ok(())
}

/// Disconnect from an SSE endpoint
#[tauri::command]
pub async fn sse_disconnect(connection_id: String) -> Result<(), String> {
    if let Some((_, tx)) = SSE_CONNECTIONS.remove(&connection_id) {
        let _ = tx.send(()).await;
        Ok(())
    } else {
        Err("Connection not found".to_string())
    }
}
