//! API connection health check.

use crate::commands::friendly_error::friendly_error;

/// Probe the given API endpoint with a minimal chat request.
#[tauri::command]
pub async fn check_api_connection(
    api_key: String,
    api_url: String,
    model: String,
) -> Result<String, String> {
    use crate::api::client::AnthropicClient;
    use crate::models::message::Message;
    use std::time::Duration;
    use tokio::time::timeout;

    let client = AnthropicClient::new(api_key, api_url.clone());

    let msg = Message::user("ping");
    let result = timeout(
        Duration::from_secs(10),
        client.send_message("respond with pong", &[msg], &model),
    )
    .await;

    match result {
        Ok(Ok(_response)) => Ok("ok".into()),
        Ok(Err(e)) => Err(friendly_error(e)),
        Err(_) => Err(
            "connection timed out (10s), please check the API URL and network connection"
                .to_string(),
        ),
    }
}
