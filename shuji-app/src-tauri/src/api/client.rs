use std::time::Duration;

use crate::api::reasoning::{self, LlmProvider};
use crate::config::ResolvedReasoningPolicy;
use serde::Serialize;

const API_TIMEOUT: Duration = Duration::from_secs(180);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("shuji/1.0")
        .timeout(API_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .expect("Failed to build reqwest Client")
}

/// Wrap reqwest error with URL context for better diagnostics.
fn map_reqwest_error(url: &str, e: reqwest::Error) -> anyhow::Error {
    let kind = if e.is_connect() {
        "连接失败"
    } else if e.is_timeout() {
        "请求超时"
    } else if e.is_body() {
        "请求体错误"
    } else if e.is_request() {
        "请求错误"
    } else {
        "未知错误"
    };
    anyhow::anyhow!("[{}] {} {}", url, kind, e)
}

/// Shared API client supporting both Anthropic and OpenAI-compatible formats.
/// Auto-detects format based on the URL (anthropic.com → Anthropic, else → OpenAI).
///
/// For tool-use agents, agents create a `Session` (from `session` module)
/// that owns an `Arc<AnthropicClient>` reference.  Text-only calls use
/// `send_message()` directly.
#[derive(Clone)]
pub struct AnthropicClient {
    pub(crate) api_key: String,
    pub(crate) api_url: String,
    pub(crate) http_client: reqwest::Client,
}

#[derive(Serialize)]
struct MessageItem {
    role: String,
    content: String,
}

/// Tool definition for OpenAI-compatible function calling.
#[derive(Clone, Serialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunction,
}

#[derive(Clone, Serialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl AnthropicClient {
    pub fn new(api_key: String, api_url: String) -> Self {
        Self {
            api_key,
            api_url,
            http_client: build_http_client(),
        }
    }

    /// Simple text-only send (no tools). Auto-detects format.
    pub async fn send_message(
        &self,
        system_prompt: &str,
        messages: &[crate::models::message::Message],
        model: &str,
    ) -> anyhow::Result<String> {
        let policy = ResolvedReasoningPolicy::disabled();
        self.send_message_with_reasoning(system_prompt, messages, model, policy)
            .await
    }

    /// Text-only send with explicit reasoning policy.
    pub async fn send_message_with_reasoning(
        &self,
        system_prompt: &str,
        messages: &[crate::models::message::Message],
        model: &str,
        policy: ResolvedReasoningPolicy,
    ) -> anyhow::Result<String> {
        if self.api_url.contains("anthropic.com") {
            self.send_anthropic_with_reasoning(system_prompt, messages, model, policy)
                .await
        } else {
            self.send_openai_with_reasoning(system_prompt, messages, model, policy)
                .await
        }
    }

    // ── Anthropic format ─────────────────────────────────────

    async fn send_anthropic_with_reasoning(
        &self,
        system_prompt: &str,
        messages: &[crate::models::message::Message],
        model: &str,
        policy: ResolvedReasoningPolicy,
    ) -> anyhow::Result<String> {
        let api_messages: Vec<MessageItem> = self.build_messages(messages);
        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": 32_768,
            "system": system_prompt,
            "messages": api_messages.iter().map(|m| {
                serde_json::json!({"role": m.role, "content": m.content})
            }).collect::<Vec<_>>(),
        });
        reasoning::apply_reasoning_to_body(&mut body, LlmProvider::Anthropic, policy);

        log_console!("[api] anthropic send (model={})", model);
        let resp = self
            .http_client
            .post(&self.api_url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .timeout(API_TIMEOUT)
            .send()
            .await
            .map_err(|e| map_reqwest_error(&self.api_url, e))?;
        log_console!("[api] response status={}", resp.status());

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status, text);
        }

        let data: serde_json::Value = resp.json().await?;
        if let Some(usage) = data.get("usage") {
            let prompt = usage["input_tokens"].as_u64().unwrap_or(0);
            let cached = usage["cache_read_input_tokens"].as_u64().unwrap_or(0);
            let completion = usage["output_tokens"].as_u64().unwrap_or(0);
            crate::token_tracker::record("text", prompt, cached, completion, model);
        }
        let text = data["content"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter(|b| b["type"] == "text")
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        Ok(text)
    }

    // ── OpenAI format (no tools) ─────────────────────────────

    async fn send_openai_with_reasoning(
        &self,
        system_prompt: &str,
        messages: &[crate::models::message::Message],
        model: &str,
        policy: ResolvedReasoningPolicy,
    ) -> anyhow::Result<String> {
        let mut api_messages = Vec::new();
        if !system_prompt.is_empty() {
            api_messages.push(MessageItem {
                role: "system".into(),
                content: system_prompt.to_string(),
            });
        }
        for m in messages.iter().filter(|m| m.role != "system") {
            api_messages.push(MessageItem {
                role: m.role.clone(),
                content: m.content.clone(),
            });
        }
        if api_messages.is_empty() {
            api_messages.push(MessageItem {
                role: "user".into(),
                content: "请继续".into(),
            });
        }

        let mut body = serde_json::json!({
            "model": model,
            "messages": api_messages,
        });
        let provider = reasoning::detect_provider(&self.api_url, model);
        reasoning::apply_reasoning_to_body(&mut body, provider, policy);

        log_console!("[api] openai send (model={})", model);
        let resp = self
            .http_client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .timeout(API_TIMEOUT)
            .send()
            .await
            .map_err(|e| map_reqwest_error(&self.api_url, e))?;
        log_console!("[api] response status={}", resp.status());

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status, text);
        }

        let data: serde_json::Value = resp.json().await?;
        if let Some(usage) = data.get("usage") {
            let prompt = usage["prompt_tokens"].as_u64().unwrap_or(0);
            let cached = usage["prompt_tokens_details"]["cached_tokens"]
                .as_u64()
                .unwrap_or(0);
            let completion = usage["completion_tokens"].as_u64().unwrap_or(0);
            crate::token_tracker::record("text", prompt, cached, completion, model);
        }
        Ok(data["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .to_string())
    }

    fn build_messages(&self, messages: &[crate::models::message::Message]) -> Vec<MessageItem> {
        let mut api_messages: Vec<MessageItem> = messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| MessageItem {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();
        if api_messages.is_empty() {
            api_messages.push(MessageItem {
                role: "user".into(),
                content: "请继续".into(),
            });
        }
        api_messages
    }

    /// Stream text-only chat completion. Falls back to non-streaming `send_message` on failure.
    pub async fn stream_message<F>(
        &self,
        system_prompt: &str,
        messages: &[crate::models::message::Message],
        model: &str,
        cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        on_delta: F,
    ) -> anyhow::Result<String>
    where
        F: FnMut(&str) -> anyhow::Result<()>,
    {
        let policy = ResolvedReasoningPolicy::disabled();
        self.stream_message_with_reasoning(system_prompt, messages, model, cancel, policy, on_delta)
            .await
    }

    /// Stream text-only chat completion with reasoning policy.
    pub async fn stream_message_with_reasoning<F>(
        &self,
        system_prompt: &str,
        messages: &[crate::models::message::Message],
        model: &str,
        cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        policy: ResolvedReasoningPolicy,
        mut on_delta: F,
    ) -> anyhow::Result<String>
    where
        F: FnMut(&str) -> anyhow::Result<()>,
    {
        let result = if self.api_url.contains("anthropic.com") {
            self.stream_anthropic_with_reasoning(
                system_prompt,
                messages,
                model,
                cancel.clone(),
                policy,
                &mut on_delta,
            )
            .await
        } else {
            self.stream_openai_with_reasoning(
                system_prompt,
                messages,
                model,
                cancel.clone(),
                policy,
                &mut on_delta,
            )
            .await
        };

        match result {
            Ok(text) => Ok(text),
            Err(e) => {
                log_console!("[api] stream failed ({}), falling back to send_message", e);
                let text = self
                    .send_message_with_reasoning(system_prompt, messages, model, policy)
                    .await?;
                on_delta(&text)?;
                Ok(text)
            }
        }
    }

    async fn stream_openai_with_reasoning<F>(
        &self,
        system_prompt: &str,
        messages: &[crate::models::message::Message],
        model: &str,
        cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        policy: ResolvedReasoningPolicy,
        on_delta: &mut F,
    ) -> anyhow::Result<String>
    where
        F: FnMut(&str) -> anyhow::Result<()>,
    {
        let mut api_messages = Vec::new();
        if !system_prompt.is_empty() {
            api_messages.push(serde_json::json!({"role": "system", "content": system_prompt}));
        }
        for m in messages.iter().filter(|m| m.role != "system") {
            api_messages.push(serde_json::json!({"role": m.role, "content": m.content}));
        }
        if api_messages.is_empty() {
            api_messages.push(serde_json::json!({"role": "user", "content": "请继续"}));
        }

        let mut body = serde_json::json!({
            "model": model,
            "messages": api_messages,
            "stream": true,
        });
        let provider = reasoning::detect_provider(&self.api_url, model);
        reasoning::apply_reasoning_to_body(&mut body, provider, policy);

        log_console!("[api] openai stream send (model={})", model);
        let resp = self
            .http_client
            .post(&self.api_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .timeout(API_TIMEOUT)
            .send()
            .await
            .map_err(|e| map_reqwest_error(&self.api_url, e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status, text);
        }

        crate::api::stream::consume_openai_sse_stream(resp.bytes_stream(), cancel, on_delta).await
    }

    async fn stream_anthropic_with_reasoning<F>(
        &self,
        system_prompt: &str,
        messages: &[crate::models::message::Message],
        model: &str,
        cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        policy: ResolvedReasoningPolicy,
        on_delta: &mut F,
    ) -> anyhow::Result<String>
    where
        F: FnMut(&str) -> anyhow::Result<()>,
    {
        let api_messages: Vec<MessageItem> = self.build_messages(messages);
        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": 32768,
            "stream": true,
            "system": system_prompt,
            "messages": api_messages.iter().map(|m| {
                serde_json::json!({"role": m.role, "content": m.content})
            }).collect::<Vec<_>>(),
        });
        reasoning::apply_reasoning_to_body(&mut body, LlmProvider::Anthropic, policy);

        log_console!("[api] anthropic stream send (model={})", model);
        let resp = self
            .http_client
            .post(&self.api_url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .timeout(API_TIMEOUT)
            .send()
            .await
            .map_err(|e| map_reqwest_error(&self.api_url, e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status, text);
        }

        crate::api::stream::consume_anthropic_sse_stream(resp.bytes_stream(), cancel, on_delta)
            .await
    }
}
