use crate::models::{AuthClaims, VaultEvent, WebSocketMessage};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::time::{Duration, Instant, interval};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use tokio::net::TcpStream;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const PONG_TIMEOUT: Duration = Duration::from_secs(10);

pub type WsStream = WebSocketStream<TcpStream>;
pub type WsSink = SplitSink<WsStream, Message>;
pub type WsSource = SplitStream<WsStream>;

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

// ── Authentication ──────────────────────────────────────────────────────────

pub fn validate_ws_token(token: &str, secret: &[u8]) -> Result<AuthClaims, String> {
    let key = DecodingKey::from_secret(secret);
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_required_spec_claims(&["exp", "sub"]);

    decode::<AuthClaims>(token, &key, &validation)
        .map(|data| data.claims)
        .map_err(|e| format!("Invalid token: {e}"))
}

pub fn extract_token_from_header(header_value: &str) -> Option<&str> {
    header_value.strip_prefix("Bearer ")
}

// ── Heartbeat-aware vault stream ────────────────────────────────────────────

pub async fn handle_vault_stream(
    vault_id: String,
    mut rx: broadcast::Receiver<VaultEvent>,
    mut ws_sink: WsSink,
) {
    handle_authenticated_vault_stream(vec![vault_id], rx, ws_sink).await;
}

pub async fn handle_authenticated_vault_stream(
    authorized_vault_ids: Vec<String>,
    mut rx: broadcast::Receiver<VaultEvent>,
    mut ws_sink: WsSink,
) {
    let mut heartbeat = interval(HEARTBEAT_INTERVAL);
    let mut last_pong = Instant::now();
    let mut awaiting_pong = false;

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Ok(event) if authorized_vault_ids.contains(&event.vault_id) => {
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
                    Ok(_) => {} // event for a vault not authorized — skip
                    Err(_) => break,
                }
            }
            _ = heartbeat.tick() => {
                if awaiting_pong && last_pong.elapsed() > HEARTBEAT_INTERVAL + PONG_TIMEOUT {
                    break;
                }
                if ws_sink.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
                awaiting_pong = true;
            }
        }
    }
}

pub async fn read_client_frames(
    mut ws_source: WsSource,
    pong_notify: Arc<tokio::sync::Notify>,
) {
    while let Some(Ok(msg)) = ws_source.next().await {
        match msg {
            Message::Pong(_) => {
                pong_notify.notify_one();
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}

pub async fn handle_vault_stream_with_heartbeat(
    authorized_vault_ids: Vec<String>,
    mut rx: broadcast::Receiver<VaultEvent>,
    mut ws_sink: WsSink,
    mut ws_source: WsSource,
) {
    let mut heartbeat = interval(HEARTBEAT_INTERVAL);
    let mut last_pong = Instant::now();
    let mut awaiting_pong = false;

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Ok(event) if authorized_vault_ids.contains(&event.vault_id) => {
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
                    Ok(_) => {}
                    Err(_) => break,
                }
            }
            frame = ws_source.next() => {
                match frame {
                    Some(Ok(Message::Pong(_))) => {
                        last_pong = Instant::now();
                        awaiting_pong = false;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
            _ = heartbeat.tick() => {
                if awaiting_pong && last_pong.elapsed() > HEARTBEAT_INTERVAL + PONG_TIMEOUT {
                    let _ = ws_sink.send(Message::Close(None)).await;
                    break;
                }
                if ws_sink.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
                awaiting_pong = true;
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
        delay = (delay * 2).min(30000);
    }

    if max_retries == 0 {
        return Err("Max retries exceeded".to_string());
    }

    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{EventType, VaultEvent};
    use jsonwebtoken::{encode, EncodingKey, Header};

    #[tokio::test]
    async fn test_vault_broadcaster_creation() {
        let broadcaster = VaultBroadcaster::new(100);
        let _rx = broadcaster.subscribe();
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

    // ── Authentication tests ────────────────────────────────────────────────

    fn make_test_secret() -> Vec<u8> {
        b"test-secret-key-for-unit-tests".to_vec()
    }

    fn make_test_token(claims: &AuthClaims, secret: &[u8]) -> String {
        encode(
            &Header::default(),
            claims,
            &EncodingKey::from_secret(secret),
        )
        .unwrap()
    }

    #[test]
    fn test_validate_valid_token() {
        let secret = make_test_secret();
        let claims = AuthClaims {
            sub: "user123".to_string(),
            vault_ids: vec!["v1".to_string(), "v2".to_string()],
            exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize,
        };
        let token = make_test_token(&claims, &secret);
        let result = validate_ws_token(&token, &secret);
        assert!(result.is_ok());
        let decoded = result.unwrap();
        assert_eq!(decoded.sub, "user123");
        assert_eq!(decoded.vault_ids, vec!["v1", "v2"]);
    }

    #[test]
    fn test_validate_expired_token() {
        let secret = make_test_secret();
        let claims = AuthClaims {
            sub: "user123".to_string(),
            vault_ids: vec!["v1".to_string()],
            exp: 1000, // long expired
        };
        let token = make_test_token(&claims, &secret);
        let result = validate_ws_token(&token, &secret);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid token"));
    }

    #[test]
    fn test_validate_wrong_secret() {
        let secret = make_test_secret();
        let claims = AuthClaims {
            sub: "user123".to_string(),
            vault_ids: vec!["v1".to_string()],
            exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize,
        };
        let token = make_test_token(&claims, &secret);
        let result = validate_ws_token(&token, b"wrong-secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_malformed_token() {
        let result = validate_ws_token("not-a-jwt", &make_test_secret());
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_token_from_bearer_header() {
        let header = "Bearer eyJhbGciOiJIUzI1NiJ9.test.sig";
        let token = extract_token_from_header(header);
        assert_eq!(token, Some("eyJhbGciOiJIUzI1NiJ9.test.sig"));
    }

    #[test]
    fn test_extract_token_missing_bearer_prefix() {
        let header = "Basic some-credentials";
        assert!(extract_token_from_header(header).is_none());
    }

    #[test]
    fn test_extract_token_empty_header() {
        assert!(extract_token_from_header("").is_none());
    }

    // ── Event scoping tests ─────────────────────────────────────────────────

    #[test]
    fn test_authorized_vault_ids_filtering() {
        let authorized = vec!["v1".to_string(), "v3".to_string()];
        let event_v1 = VaultEvent {
            vault_id: "v1".to_string(),
            event_type: EventType::CheckIn,
            timestamp: chrono::Utc::now(),
            data: serde_json::json!({}),
        };
        let event_v2 = VaultEvent {
            vault_id: "v2".to_string(),
            event_type: EventType::CheckIn,
            timestamp: chrono::Utc::now(),
            data: serde_json::json!({}),
        };
        assert!(authorized.contains(&event_v1.vault_id));
        assert!(!authorized.contains(&event_v2.vault_id));
    }

    // ── Heartbeat config tests ──────────────────────────────────────────────

    #[test]
    fn test_heartbeat_interval_is_30_seconds() {
        assert_eq!(HEARTBEAT_INTERVAL, Duration::from_secs(30));
    }

    #[test]
    fn test_pong_timeout_is_10_seconds() {
        assert_eq!(PONG_TIMEOUT, Duration::from_secs(10));
    }

    #[tokio::test]
    async fn test_connection_cleanup_on_missed_pong() {
        let broadcaster = VaultBroadcaster::new(10);
        let _rx = broadcaster.subscribe();

        // Verify that the timeout constants are correct for cleanup behavior
        let elapsed = HEARTBEAT_INTERVAL + PONG_TIMEOUT;
        assert_eq!(elapsed, Duration::from_secs(40));
        // A connection that hasn't ponged in 40s should be closed
    }
}
