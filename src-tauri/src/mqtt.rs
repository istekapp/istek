use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS, Transport};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

pub struct MqttConnection {
    pub client: AsyncClient,
    pub close_tx: oneshot::Sender<()>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MqttMessage {
    pub id: String,
    pub connection_id: String,
    pub topic: String,
    pub payload: String,
    pub qos: u8,
    pub retained: bool,
    pub timestamp: u64,
    pub direction: String, // "sent" | "received"
    #[serde(rename = "type")]
    pub msg_type: String, // "message" | "connect" | "disconnect" | "subscribe" | "error"
}

fn qos_from_u8(qos: u8) -> QoS {
    match qos {
        0 => QoS::AtMostOnce,
        1 => QoS::AtLeastOnce,
        2 => QoS::ExactlyOnce,
        _ => QoS::AtMostOnce,
    }
}

#[tauri::command]
pub async fn mqtt_connect(
    app: AppHandle,
    connection_id: String,
    broker: String,
    port: u16,
    client_id: String,
    username: Option<String>,
    password: Option<String>,
    use_tls: bool,
) -> Result<String, String> {
    let mut mqttoptions = MqttOptions::new(&client_id, &broker, port);
    mqttoptions.set_keep_alive(Duration::from_secs(30));

    if let (Some(user), Some(pass)) = (username, password) {
        if !user.is_empty() {
            mqttoptions.set_credentials(user, pass);
        }
    }

    if use_tls {
        mqttoptions.set_transport(Transport::tls_with_default_config());
    }

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    let (close_tx, mut close_rx) = oneshot::channel::<()>();

    crate::MQTT_CONNECTIONS.insert(
        connection_id.clone(),
        MqttConnection {
            client: client.clone(),
            close_tx,
        },
    );

    // Emit connected event
    let _ = app.emit("mqtt-message", MqttMessage {
        id: uuid::Uuid::new_v4().to_string(),
        connection_id: connection_id.clone(),
        topic: "".to_string(),
        payload: format!("Connected to {}:{}", broker, port),
        qos: 0,
        retained: false,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
        direction: "received".to_string(),
        msg_type: "connect".to_string(),
    });

    // Spawn event loop
    let app_clone = app.clone();
    let conn_id = connection_id.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                event = eventloop.poll() => {
                    match event {
                        Ok(Event::Incoming(Packet::Publish(publish))) => {
                            let payload = String::from_utf8_lossy(&publish.payload).to_string();
                            let _ = app_clone.emit("mqtt-message", MqttMessage {
                                id: uuid::Uuid::new_v4().to_string(),
                                connection_id: conn_id.clone(),
                                topic: publish.topic.clone(),
                                payload,
                                qos: publish.qos as u8,
                                retained: publish.retain,
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis() as u64,
                                direction: "received".to_string(),
                                msg_type: "message".to_string(),
                            });
                        }
                        Ok(Event::Incoming(Packet::SubAck(suback))) => {
                            let _ = app_clone.emit("mqtt-message", MqttMessage {
                                id: uuid::Uuid::new_v4().to_string(),
                                connection_id: conn_id.clone(),
                                topic: "".to_string(),
                                payload: format!("Subscribed (pkid: {})", suback.pkid),
                                qos: 0,
                                retained: false,
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis() as u64,
                                direction: "received".to_string(),
                                msg_type: "subscribe".to_string(),
                            });
                        }
                        Ok(Event::Incoming(Packet::ConnAck(_))) => {
                            // Already handled
                        }
                        Err(e) => {
                            let _ = app_clone.emit("mqtt-message", MqttMessage {
                                id: uuid::Uuid::new_v4().to_string(),
                                connection_id: conn_id.clone(),
                                topic: "".to_string(),
                                payload: format!("Error: {}", e),
                                qos: 0,
                                retained: false,
                                timestamp: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis() as u64,
                                direction: "received".to_string(),
                                msg_type: "error".to_string(),
                            });
                            break;
                        }
                        _ => {}
                    }
                }
                _ = &mut close_rx => {
                    break;
                }
            }
        }
        crate::MQTT_CONNECTIONS.remove(&conn_id);
    });

    Ok(connection_id)
}

#[tauri::command]
pub async fn mqtt_subscribe(
    connection_id: String,
    topic: String,
    qos: u8,
) -> Result<(), String> {
    let conn = crate::MQTT_CONNECTIONS
        .get(&connection_id)
        .ok_or("Connection not found")?;

    conn.client
        .subscribe(&topic, qos_from_u8(qos))
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn mqtt_publish(
    app: AppHandle,
    connection_id: String,
    topic: String,
    payload: String,
    qos: u8,
    retain: bool,
) -> Result<(), String> {
    let conn = crate::MQTT_CONNECTIONS
        .get(&connection_id)
        .ok_or("Connection not found")?;

    conn.client
        .publish(&topic, qos_from_u8(qos), retain, payload.as_bytes())
        .await
        .map_err(|e| e.to_string())?;

    // Emit sent message
    let _ = app.emit("mqtt-message", MqttMessage {
        id: uuid::Uuid::new_v4().to_string(),
        connection_id,
        topic,
        payload,
        qos,
        retained: retain,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
        direction: "sent".to_string(),
        msg_type: "message".to_string(),
    });

    Ok(())
}

#[tauri::command]
pub async fn mqtt_disconnect(connection_id: String) -> Result<(), String> {
    if let Some((_, conn)) = crate::MQTT_CONNECTIONS.remove(&connection_id) {
        let _ = conn.client.disconnect().await;
        let _ = conn.close_tx.send(());
    }
    Ok(())
}
