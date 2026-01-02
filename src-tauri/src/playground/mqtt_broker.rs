use tokio::sync::oneshot;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Simple MQTT-like broker stub
/// This provides a basic TCP endpoint that accepts MQTT connections
/// but just echoes back for testing purposes
pub struct MqttBroker {
    shutdown_tx: Option<oneshot::Sender<()>>,
    pub port: u16,
}

impl MqttBroker {
    pub fn new(port: u16) -> Self {
        Self {
            shutdown_tx: None,
            port,
        }
    }

    /// Start the MQTT broker stub
    pub async fn start(&mut self) -> Result<(), String> {
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        let port = self.port;
        let addr = format!("127.0.0.1:{}", port);

        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| format!("Failed to bind MQTT broker to {}: {}", addr, e))?;

        // Spawn broker in background task
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((mut socket, _addr)) => {
                                // Handle MQTT connection in a separate task
                                tokio::spawn(async move {
                                    let mut buf = [0u8; 1024];
                                    loop {
                                        match socket.read(&mut buf).await {
                                            Ok(0) => break, // Connection closed
                                            Ok(n) => {
                                                // Check for MQTT CONNECT packet (first byte 0x10)
                                                if buf[0] == 0x10 {
                                                    // Send CONNACK (0x20 0x02 0x00 0x00)
                                                    let connack = [0x20, 0x02, 0x00, 0x00];
                                                    let _ = socket.write_all(&connack).await;
                                                }
                                                // Check for SUBSCRIBE packet (first byte 0x82)
                                                else if buf[0] == 0x82 {
                                                    // Send SUBACK
                                                    // Get packet identifier from bytes 2-3
                                                    let packet_id = if n >= 4 {
                                                        [buf[2], buf[3]]
                                                    } else {
                                                        [0x00, 0x01]
                                                    };
                                                    let suback = [0x90, 0x03, packet_id[0], packet_id[1], 0x00];
                                                    let _ = socket.write_all(&suback).await;
                                                }
                                                // Check for PUBLISH packet (first byte 0x30-0x3F)
                                                else if (buf[0] & 0xF0) == 0x30 {
                                                    // QoS 0 - no response needed
                                                    // For QoS 1 (0x32), send PUBACK
                                                    if (buf[0] & 0x06) == 0x02 {
                                                        // Get remaining length
                                                        let remaining_len = buf[1] as usize;
                                                        if remaining_len >= 2 && n >= 4 {
                                                            // Topic length at bytes 2-3
                                                            let topic_len = ((buf[2] as usize) << 8) | (buf[3] as usize);
                                                            // Packet ID is after topic
                                                            let pid_offset = 4 + topic_len;
                                                            if n >= pid_offset + 2 {
                                                                let puback = [0x40, 0x02, buf[pid_offset], buf[pid_offset + 1]];
                                                                let _ = socket.write_all(&puback).await;
                                                            }
                                                        }
                                                    }
                                                }
                                                // Check for PINGREQ (0xC0)
                                                else if buf[0] == 0xC0 {
                                                    // Send PINGRESP
                                                    let pingresp = [0xD0, 0x00];
                                                    let _ = socket.write_all(&pingresp).await;
                                                }
                                                // Check for DISCONNECT (0xE0)
                                                else if buf[0] == 0xE0 {
                                                    break;
                                                }
                                            }
                                            Err(_) => break,
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                eprintln!("MQTT broker accept error: {}", e);
                            }
                        }
                    }
                    _ = &mut shutdown_rx => {
                        break;
                    }
                }
            }
        });

        // Give broker time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        Ok(())
    }

    /// Stop the MQTT broker
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for MqttBroker {
    fn drop(&mut self) {
        self.stop();
    }
}
