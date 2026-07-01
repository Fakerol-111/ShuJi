//! Request building and sending for the Session module.
//!
//! Extracted from `mod.rs`. Two `impl Session` methods:
//! - `build_step_body()` — constructs the JSON body shared by `step()` and
//!   `step_stream()`.
//! - `api_request()` — sends the HTTP request and returns the response JSON.
//!
//! Both were free-standing `fn`s in `mod.rs`; moved here unchanged.

use std::time::Duration;

use crate::api::client::AnthropicClient;
use crate::api::reasoning;

impl super::Session {
    /// Build the JSON body shared by `step()` and `step_stream()`.
    pub(super) fn build_step_body(&self) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": self.model(),
            "messages": self.messages_ref(),
            "temperature": 0.1,
            "top_p": 0.9,
            "frequency_penalty": 0.1,
            "seed": 42,
        });
        if let Some(max_tokens) = self.max_tokens() {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }

        // Apply reasoning/thinking fields via the centralized adapter
        reasoning::apply_reasoning_to_body(&mut body, self.provider(), self.reasoning_policy());

        if self.tool_choice_none() {
            body["tool_choice"] = serde_json::json!("none");
            if !self.tools_ref().is_empty() {
                body["tools"] = serde_json::to_value(self.tools_ref()).unwrap_or_default();
                body["parallel_tool_calls"] = serde_json::json!(false);
                body["temperature"] = serde_json::json!(0.0);
            }
        } else if !self.tools_ref().is_empty() {
            body["tools"] = serde_json::to_value(self.tools_ref()).unwrap_or_default();
            body["tool_choice"] = serde_json::json!("auto");
        }
        body
    }

    /// Send API request, returning the response JSON on success.
    pub(super) async fn api_request(
        client: &AnthropicClient,
        body: &serde_json::Value,
        timeout: Duration,
    ) -> anyhow::Result<serde_json::Value> {
        let resp = client
            .http_client
            .post(&client.api_url)
            .header("Authorization", format!("Bearer {}", client.api_key))
            .header("content-type", "application/json")
            .json(body)
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| {
                let kind = if e.is_connect() {
                    "connection failed"
                } else if e.is_timeout() {
                    "request timeout"
                } else if e.is_body() {
                    "request body error"
                } else {
                    "request error"
                };
                anyhow::anyhow!("[{}] {} {}", client.api_url, kind, e)
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status, text);
        }

        Ok(resp.json().await?)
    }
}
