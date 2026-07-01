//! Streaming step for the Session module.
//!
//! Extracted from `mod.rs`. `step_stream()` does one API round-trip with
//! streaming text/reasoning deltas (OpenAI-compatible APIs only). Falls back
//! to `step()` on any failure, Anthropic URLs, or `finish_reason=length`.

use crate::api::stream::AgentStreamChunk;

use super::types::StepResult;

impl super::Session {
    /// One API round-trip with streaming text/reasoning deltas when supported.
    /// OpenAI-compatible APIs only; falls back to [`step()`] on failure or Anthropic URLs.
    pub async fn step_stream<F>(&mut self, mut on_chunk: F) -> anyhow::Result<StepResult>
    where
        F: FnMut(AgentStreamChunk),
    {
        let client = self.client().clone();
        if client.api_url.contains("anthropic.com") {
            return self.step().await;
        }

        let mut body = self.build_step_body();
        body["stream"] = serde_json::json!(true);

        log_console!(
            "[{}] step_stream: sending {} messages",
            self.role(),
            self.messages_len()
        );

        let resp = match client
            .http_client
            .post(&client.api_url)
            .header("Authorization", format!("Bearer {}", client.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .timeout(self.config().api_timeout())
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                log_console!(
                    "[{}] step_stream request failed: {}, fallback step()",
                    self.role(),
                    e
                );
                return self.step().await;
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            log_console!(
                "[{}] step_stream API {}: {}, fallback step()",
                self.role(),
                status,
                text.chars().take(120).collect::<String>()
            );
            return self.step().await;
        }

        let streamed =
            match crate::api::stream::consume_openai_agent_stream(resp.bytes_stream(), |chunk| {
                on_chunk(chunk);
                Ok(())
            })
            .await
            {
                Ok(s) => s,
                Err(e) => {
                    log_console!(
                        "[{}] step_stream parse failed: {}, fallback step()",
                        self.role(),
                        e
                    );
                    return self.step().await;
                }
            };

        if streamed.finish_reason == "length" {
            log_console!(
                "[{}] step_stream truncated (length), fallback step()",
                self.role()
            );
            return self.step().await;
        }

        if !streamed.reasoning_content.is_empty() {
            log_console!(
                "[{}] reasoning(stream): {} chars",
                self.role(),
                streamed.reasoning_content.chars().count()
            );
        }

        let msg = serde_json::json!({
            "content": streamed.content,
            "reasoning_content": streamed.reasoning_content,
            "tool_calls": streamed.tool_calls,
        });

        Ok(self.process_assistant_into_result(&msg))
    }
}
