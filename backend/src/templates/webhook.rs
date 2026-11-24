use serde_json::json;

/// Build a JSON payload for a generic webhook notification.
pub fn webhook_payload(event: &str, message: &str) -> serde_json::Value {
    json!({
        "event": event,
        "message": message,
    })
}

/// Build a JSON payload for a webhook notification that includes structured data.
pub fn webhook_payload_with_data(
    event: &str,
    message: &str,
    data: serde_json::Value,
) -> serde_json::Value {
    json!({
        "event": event,
        "message": message,
        "data": data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_payload() {
        let payload = webhook_payload("user.created", "A new user was created");
        assert_eq!(payload["event"], "user.created");
        assert_eq!(payload["message"], "A new user was created");
    }

    #[test]
    fn test_webhook_payload_with_data() {
        let data = json!({ "user_id": 42 });
        let payload = webhook_payload_with_data("user.created", "A new user was created", data);
        assert_eq!(payload["event"], "user.created");
        assert_eq!(payload["message"], "A new user was created");
        assert_eq!(payload["data"]["user_id"], 42);
    }
}