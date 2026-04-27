use crate::models::{VaultEvent, WebSocketMessage};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use tokio::net::TcpStream;

pub type WsStream = WebSocketStream<TcpStream>;
pub type WsSink = SplitSink<WsStream, Message>;
pub type WsStream_ = SplitStream<WsStream>;

pub struct VaultBroadcaster {
    tx: broadcast::Sender<VaultEvent>,
}

impl VaultBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        VaultBroadcaster { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<VaultEvent> {
        self.tx.subscribe()
    }

    pub async fn broadcast(&self, event: VaultEvent) {
        let _ = self.tx.send(event);
    }
}

pub async fn handle_vault_stream(
    vault_id: String,
    mut rx: broadcast::Receiver<VaultEvent>,
    mut ws_sink: WsSink,
) {
    while let Ok(event) = rx.recv().await {
        if event.vault_id == vault_id {
            let msg = WebSocketMessage {
                message_type: format!("{:?}", event.event_type),
                data: event.data,
            };
            if let Ok(json) = serde_json::to_string(&msg) {
                if ws_sink.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    }
}

pub async fn reconnect_with_backoff(
    max_retries: u32,
    initial_delay_ms: u64,
) -> Result<(), String> {
    let mut retries = 0;
    let mut delay = initial_delay_ms;

    while retries < max_retries {
        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        retries += 1;
        delay = (delay * 2).min(30000); // Cap at 30s
    }

    if retries >= max_retries {
        Err("Max retries exceeded".to_string())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vault_broadcaster_creation() {
        let broadcaster = VaultBroadcaster::new(100);
        let _rx = broadcaster.subscribe();
        // Broadcaster created successfully
    }

    #[tokio::test]
    async fn test_reconnect_backoff() {
        let result = reconnect_with_backoff(3, 10).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_reconnect_max_retries() {
        let result = reconnect_with_backoff(0, 10).await;
        assert!(result.is_err());
    }
}
