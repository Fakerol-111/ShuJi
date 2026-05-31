use std::sync::LazyLock;

use tiktoken_rs::CoreBPE;

/// OpenAI-compatible chat message overhead (cl100k convention).
const MESSAGE_TOKENS: usize = 4;
const REPLY_PRIMER: usize = 3;

static BPE: LazyLock<CoreBPE> =
    LazyLock::new(|| tiktoken_rs::cl100k_base().expect("failed to load cl100k_base tokenizer"));

/// Count tokens in plain text using cl100k (OpenAI / DeepSeek-compatible approximation).
pub fn count_text_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    BPE.encode_ordinary(text).len()
}

/// Count tokens for one chat message object (role, content, tool_calls, tool_call_id).
pub fn count_message_tokens(msg: &serde_json::Value) -> usize {
    let mut tokens = MESSAGE_TOKENS;

    if let Some(role) = msg["role"].as_str() {
        tokens += count_text_tokens(role);
    }
    if let Some(name) = msg["name"].as_str() {
        tokens += count_text_tokens(name);
    }

    match msg.get("content") {
        Some(serde_json::Value::String(content)) => {
            tokens += count_text_tokens(content);
        }
        Some(value) if !value.is_null() => {
            tokens += count_text_tokens(&value.to_string());
        }
        _ => {}
    }

    if let Some(tool_calls) = msg["tool_calls"].as_array() {
        for tc in tool_calls {
            tokens += count_text_tokens(&tc.to_string());
        }
    }
    if let Some(id) = msg["tool_call_id"].as_str() {
        tokens += count_text_tokens(id);
    }

    tokens
}

/// Count tokens for a message list as sent in Chat Completions `messages`.
pub fn count_messages_tokens(msgs: &[serde_json::Value]) -> usize {
    let mut total = REPLY_PRIMER;
    for msg in msgs {
        total += count_message_tokens(msg);
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_is_zero() {
        assert_eq!(count_text_tokens(""), 0);
    }

    #[test]
    fn messages_include_overhead() {
        let msgs = vec![serde_json::json!({"role": "user", "content": "hello"})];
        assert!(count_messages_tokens(&msgs) > count_text_tokens("hello"));
    }
}
