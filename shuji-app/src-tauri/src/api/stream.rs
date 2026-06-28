//! SSE stream parsing for OpenAI-compatible and Anthropic chat APIs.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures::StreamExt;
use serde::Serialize;

/// Incremental text chunk from a streaming LLM response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextDelta {
    pub text: String,
}

/// Event emitted to the frontend during discuss streaming.
#[derive(Debug, Clone, Serialize)]
pub struct ChatDeltaEvent {
    pub message_id: String,
    pub role: String,
    pub delta: String,
}

/// Parse a single OpenAI-style SSE `data:` payload line.
pub fn parse_openai_sse_data(data: &str) -> Option<TextDelta> {
    let trimmed = data.trim();
    if trimmed.is_empty() || trimmed == "[DONE]" {
        return None;
    }
    let json: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let content = json["choices"]
        .as_array()?
        .first()?
        .get("delta")
        .and_then(|d| d.get("content"))
        .and_then(|c| c.as_str())?;
    if content.is_empty() {
        return None;
    }
    Some(TextDelta {
        text: content.to_string(),
    })
}

/// Parse Anthropic SSE event + data pair.
pub fn parse_anthropic_sse_event(event: &str, data: &str) -> Option<TextDelta> {
    if event != "content_block_delta" {
        return None;
    }
    let json: serde_json::Value = serde_json::from_str(data).ok()?;
    if json["type"].as_str()? != "content_block_delta" {
        return None;
    }
    let text = json["delta"]["text"].as_str()?;
    if text.is_empty() {
        return None;
    }
    Some(TextDelta {
        text: text.to_string(),
    })
}

/// Consume an OpenAI-compatible byte stream, invoking `on_delta` for each text chunk.
pub async fn consume_openai_sse_stream<F>(
    mut byte_stream: impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
    cancel: Arc<AtomicBool>,
    mut on_delta: F,
) -> anyhow::Result<String>
where
    F: FnMut(&str) -> anyhow::Result<()>,
{
    let mut buffer = String::new();
    let mut full = String::new();

    while let Some(chunk) = byte_stream.next().await {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        let chunk = chunk?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buffer.find("\n\n") {
            let block = buffer[..pos].to_string();
            buffer = buffer[pos + 2..].to_string();

            for line in block.lines() {
                if let Some(data) = line.strip_prefix("data:") {
                    if let Some(delta) = parse_openai_sse_data(data) {
                        full.push_str(&delta.text);
                        on_delta(&delta.text)?;
                    }
                }
            }
        }
    }

    Ok(full)
}

/// Consume an Anthropic SSE byte stream.
pub async fn consume_anthropic_sse_stream<F>(
    mut byte_stream: impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
    cancel: Arc<AtomicBool>,
    mut on_delta: F,
) -> anyhow::Result<String>
where
    F: FnMut(&str) -> anyhow::Result<()>,
{
    let mut buffer = String::new();
    let mut full = String::new();
    let mut current_event = String::new();

    while let Some(chunk) = byte_stream.next().await {
        if cancel.load(Ordering::SeqCst) {
            break;
        }
        let chunk = chunk?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim_end_matches('\r').to_string();
            buffer = buffer[pos + 1..].to_string();

            if line.is_empty() {
                if !current_event.is_empty() {
                    // event/data pairs handled below
                }
                continue;
            }

            if let Some(event) = line.strip_prefix("event:") {
                current_event = event.trim().to_string();
            } else if let Some(data) = line.strip_prefix("data:") {
                if let Some(delta) = parse_anthropic_sse_event(&current_event, data.trim()) {
                    full.push_str(&delta.text);
                    on_delta(&delta.text)?;
                }
            }
        }
    }

    Ok(full)
}

/// Chunk kinds emitted while streaming an agent step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentStreamChunk {
    TextDelta(String),
    ReasoningDelta(String),
}

/// Assembled assistant message from an OpenAI-compatible agent stream.
#[derive(Debug, Clone, Default)]
pub struct StreamedAssistantMessage {
    pub content: String,
    pub reasoning_content: String,
    pub tool_calls: Vec<serde_json::Value>,
    pub finish_reason: String,
}

#[derive(Default)]
pub(crate) struct PartialToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

fn apply_openai_tool_call_delta(
    partials: &mut std::collections::HashMap<usize, PartialToolCall>,
    delta_tool_calls: &[serde_json::Value],
) {
    for tc in delta_tool_calls {
        let index = tc["index"].as_u64().unwrap_or(0) as usize;
        let entry = partials.entry(index).or_default();
        if let Some(id) = tc["id"].as_str() {
            entry.id = Some(id.to_string());
        }
        if let Some(name) = tc["function"]["name"].as_str() {
            entry.name = Some(name.to_string());
        }
        if let Some(args) = tc["function"]["arguments"].as_str() {
            entry.arguments.push_str(args);
        }
    }
}

fn partials_to_tool_calls(
    partials: std::collections::HashMap<usize, PartialToolCall>,
) -> Vec<serde_json::Value> {
    let mut indices: Vec<_> = partials.keys().copied().collect();
    indices.sort_unstable();
    indices
        .into_iter()
        .filter_map(|idx| {
            let p = partials.get(&idx)?;
            let id = p.id.clone().unwrap_or_default();
            let name = p.name.clone().unwrap_or_default();
            if id.is_empty() || name.is_empty() {
                return None;
            }
            Some(serde_json::json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": p.arguments,
                }
            }))
        })
        .collect()
}

/// Parse one OpenAI agent SSE payload into accumulated message + optional live chunks.
pub(crate) fn apply_openai_agent_sse_data(
    data: &str,
    message: &mut StreamedAssistantMessage,
    partials: &mut std::collections::HashMap<usize, PartialToolCall>,
) -> Vec<AgentStreamChunk> {
    let trimmed = data.trim();
    if trimmed.is_empty() || trimmed == "[DONE]" {
        return Vec::new();
    }
    let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return Vec::new();
    };
    let choice = match json["choices"].as_array().and_then(|a| a.first()) {
        Some(c) => c,
        None => return Vec::new(),
    };
    if let Some(reason) = choice["finish_reason"].as_str() {
        if !reason.is_empty() {
            message.finish_reason = reason.to_string();
        }
    }
    let delta = &choice["delta"];
    let mut chunks = Vec::new();
    if let Some(text) = delta["content"].as_str() {
        if !text.is_empty() {
            message.content.push_str(text);
            chunks.push(AgentStreamChunk::TextDelta(text.to_string()));
        }
    }
    if let Some(text) = delta["reasoning_content"].as_str() {
        if !text.is_empty() {
            message.reasoning_content.push_str(text);
            chunks.push(AgentStreamChunk::ReasoningDelta(text.to_string()));
        }
    }
    if let Some(arr) = delta["tool_calls"].as_array() {
        apply_openai_tool_call_delta(partials, arr);
    }
    chunks
}

/// Consume an OpenAI agent SSE stream into a complete assistant message.
pub async fn consume_openai_agent_stream<F>(
    mut byte_stream: impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
    mut on_chunk: F,
) -> anyhow::Result<StreamedAssistantMessage>
where
    F: FnMut(AgentStreamChunk) -> anyhow::Result<()>,
{
    let mut buffer = String::new();
    let mut message = StreamedAssistantMessage::default();
    let mut partials: std::collections::HashMap<usize, PartialToolCall> =
        std::collections::HashMap::new();

    while let Some(chunk) = byte_stream.next().await {
        let chunk = chunk?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buffer.find("\n\n") {
            let block = buffer[..pos].to_string();
            buffer = buffer[pos + 2..].to_string();
            for line in block.lines() {
                if let Some(data) = line.strip_prefix("data:") {
                    for chunk in apply_openai_agent_sse_data(data, &mut message, &mut partials) {
                        on_chunk(chunk)?;
                    }
                }
            }
        }
    }

    message.tool_calls = partials_to_tool_calls(partials);
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_openai_delta_extracts_content() {
        let data = r#"{"choices":[{"delta":{"content":"你好"}}]}"#;
        let delta = parse_openai_sse_data(data).unwrap();
        assert_eq!(delta.text, "你好");
    }

    #[test]
    fn parse_openai_done_returns_none() {
        assert!(parse_openai_sse_data("[DONE]").is_none());
    }

    #[test]
    fn parse_anthropic_text_delta() {
        let data =
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}"#;
        let delta = parse_anthropic_sse_event("content_block_delta", data).unwrap();
        assert_eq!(delta.text, "Hi");
    }

    #[test]
    fn parse_anthropic_ignores_non_text_events() {
        let data = r#"{"type":"message_start"}"#;
        assert!(parse_anthropic_sse_event("message_start", data).is_none());
    }

    #[test]
    fn agent_stream_accumulates_tool_call_args() {
        let mut msg = StreamedAssistantMessage::default();
        let mut partials = std::collections::HashMap::new();
        apply_openai_agent_sse_data(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"read_file","arguments":""}}]}}]}"#,
            &mut msg,
            &mut partials,
        );
        apply_openai_agent_sse_data(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"path\":"}}]}}]}"#,
            &mut msg,
            &mut partials,
        );
        apply_openai_agent_sse_data(
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"main.rs\"}"}}]}}]}"#,
            &mut msg,
            &mut partials,
        );
        let calls = partials_to_tool_calls(partials);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["function"]["name"], "read_file");
    }
}
