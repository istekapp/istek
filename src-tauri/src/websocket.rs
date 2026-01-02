use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub struct WsConnection {
    pub sender: Arc<Mutex<futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        Message,
    >>>,
    pub close_tx: tokio::sync::oneshot::Sender<()>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WsMessage {
    pub id: String,
    pub connection_id: String,
    pub direction: String, // "sent" | "received"
    pub data: String,
    pub timestamp: u64,
    #[serde(rename = "type")]
    pub msg_type: String, // "text" | "binary" | "ping" | "pong" | "open" | "close" | "error"
}

fn get_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[tauri::command]
pub async fn ws_connect(
    app: AppHandle,
    connection_id: String,
    url: String,
    _headers: HashMap<String, String>,
) -> Result<String, String> {
    let (ws_stream, _response) = connect_async(&url)
        .await
        .map_err(|e| format!("Failed to connect: {}", e))?;

    let (write, mut read) = ws_stream.split();
    let write = Arc::new(Mutex::new(write));
    
    let (close_tx, mut close_rx) = tokio::sync::oneshot::channel::<()>();

    // Store connection
    crate::WS_CONNECTIONS.insert(
        connection_id.clone(),
        WsConnection {
            sender: write.clone(),
            close_tx,
        },
    );

    // Emit connected event
    let _ = app.emit("ws-message", WsMessage {
        id: uuid::Uuid::new_v4().to_string(),
        connection_id: connection_id.clone(),
        direction: "received".to_string(),
        data: "Connected".to_string(),
        timestamp: get_timestamp(),
        msg_type: "open".to_string(),
    });

    // Spawn reader task
    let app_clone = app.clone();
    let conn_id = connection_id.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = read.next() => {
                    match msg {
                        Some(Ok(message)) => {
                            let (data, msg_type): (String, &str) = match &message {
                                Message::Text(text) => (text.to_string(), "text"),
                                Message::Binary(bin) => (STANDARD.encode(bin), "binary"),
                                Message::Ping(_) => ("Ping".to_string(), "ping"),
                                Message::Pong(_) => ("Pong".to_string(), "pong"),
                                Message::Close(_) => ("Connection closed".to_string(), "close"),
                                _ => continue,
                            };

                            let _ = app_clone.emit("ws-message", WsMessage {
                                id: uuid::Uuid::new_v4().to_string(),
                                connection_id: conn_id.clone(),
                                direction: "received".to_string(),
                                data,
                                timestamp: get_timestamp(),
                                msg_type: msg_type.to_string(),
                            });

                            if matches!(message, Message::Close(_)) {
                                break;
                            }
                        }
                        Some(Err(e)) => {
                            let _ = app_clone.emit("ws-message", WsMessage {
                                id: uuid::Uuid::new_v4().to_string(),
                                connection_id: conn_id.clone(),
                                direction: "received".to_string(),
                                data: format!("Error: {}", e),
                                timestamp: get_timestamp(),
                                msg_type: "error".to_string(),
                            });
                            break;
                        }
                        None => break,
                    }
                }
                _ = &mut close_rx => {
                    break;
                }
            }
        }
        crate::WS_CONNECTIONS.remove(&conn_id);
    });

    Ok(connection_id)
}

#[tauri::command]
pub async fn ws_send(
    app: AppHandle,
    connection_id: String,
    message: String,
    message_type: String,
) -> Result<(), String> {
    let conn = crate::WS_CONNECTIONS
        .get(&connection_id)
        .ok_or("Connection not found")?;

    let msg = match message_type.as_str() {
        "binary" => {
            let bytes = STANDARD.decode(&message).map_err(|e| e.to_string())?;
            Message::Binary(bytes.into())
        }
        _ => Message::Text(message.clone().into()),
    };

    conn.sender
        .lock()
        .await
        .send(msg)
        .await
        .map_err(|e| e.to_string())?;

    // Emit sent message
    let _ = app.emit("ws-message", WsMessage {
        id: uuid::Uuid::new_v4().to_string(),
        connection_id,
        direction: "sent".to_string(),
        data: message,
        timestamp: get_timestamp(),
        msg_type: message_type,
    });

    Ok(())
}

#[tauri::command]
pub async fn ws_disconnect(connection_id: String) -> Result<(), String> {
    if let Some((_, conn)) = crate::WS_CONNECTIONS.remove(&connection_id) {
        let _ = conn.close_tx.send(());
    }
    Ok(())
}
