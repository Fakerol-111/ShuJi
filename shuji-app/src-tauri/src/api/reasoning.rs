use crate::config::{ReasoningEffort, ResolvedReasoningPolicy};

/// LLM provider family — used to select the correct reasoning/thinking field layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmProvider {
    Anthropic,
    OpenAi,
    DeepSeek,
    OpenAiCompatible,
}

/// Detect the provider family from the API URL and model name.
pub fn detect_provider(api_url: &str, model: &str) -> LlmProvider {
    if api_url.contains("anthropic.com") {
        return LlmProvider::Anthropic;
    }
    let model_lower = model.to_lowercase();
    if model_lower.starts_with("deepseek") {
        return LlmProvider::DeepSeek;
    }
    if model_lower.starts_with("gpt")
        || model_lower.starts_with("o1")
        || model_lower.starts_with("o3")
    {
        return LlmProvider::OpenAi;
    }
    LlmProvider::OpenAiCompatible
}

/// Apply reasoning/thinking fields to the request body according to the provider and policy.
pub fn apply_reasoning_to_body(
    body: &mut serde_json::Value,
    provider: LlmProvider,
    policy: ResolvedReasoningPolicy,
) {
    if !policy.enabled || policy.effort == ReasoningEffort::None {
        return; // No reasoning fields
    }

    match provider {
        LlmProvider::Anthropic => {
            let mut thinking = serde_json::json!({"type": "enabled"});
            if policy.budget_tokens > 0 {
                thinking["budget_tokens"] = serde_json::json!(policy.budget_tokens);
            }
            body["thinking"] = thinking;
        }
        LlmProvider::OpenAi => {
            // OpenAI Responses API: reasoning = { "effort": "low|medium|high" }
            // For Chat Completions (current path), use reasoning_effort field
            body["reasoning_effort"] = serde_json::json!(match policy.effort {
                ReasoningEffort::Low => "low",
                ReasoningEffort::Medium => "medium",
                ReasoningEffort::High => "high",
                ReasoningEffort::None => unreachable!(),
            });
        }
        LlmProvider::DeepSeek | LlmProvider::OpenAiCompatible => {
            // DeepSeek / OpenAI-compatible: thinking = { "type": "enabled" }
            body["thinking"] = serde_json::json!({"type": "enabled"});
        }
    }
}

/// Check if an API error response looks like an unsupported reasoning/thinking parameter error.
/// Returns true if the error is likely caused by the reasoning field and we should retry without it.
pub fn looks_like_unsupported_reasoning_error(status: u16, body: &str) -> bool {
    if status != 400 && status != 422 {
        return false;
    }
    let body_lower = body.to_lowercase();
    body_lower.contains("reasoning")
        || body_lower.contains("thinking")
        || body_lower.contains("unsupported")
        || body_lower.contains("unknown parameter")
        || body_lower.contains("unknown field")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ReasoningEffort, ResolvedReasoningPolicy};

    fn policy(enabled: bool, effort: ReasoningEffort) -> ResolvedReasoningPolicy {
        ResolvedReasoningPolicy {
            enabled,
            effort,
            budget_tokens: 0,
        }
    }

    #[test]
    fn detect_anthropic() {
        assert_eq!(
            detect_provider(
                "https://api.anthropic.com/v1/messages",
                "claude-sonnet-4-20250514"
            ),
            LlmProvider::Anthropic
        );
    }

    #[test]
    fn detect_deepseek() {
        assert_eq!(
            detect_provider(
                "https://api.deepseek.com/chat/completions",
                "deepseek-v4-flash"
            ),
            LlmProvider::DeepSeek
        );
    }

    #[test]
    fn detect_openai() {
        assert_eq!(
            detect_provider("https://api.openai.com/v1/chat/completions", "gpt-4o"),
            LlmProvider::OpenAi
        );
    }

    #[test]
    fn detect_compatible() {
        assert_eq!(
            detect_provider("https://custom.api.com/v1/chat/completions", "qwen-72b"),
            LlmProvider::OpenAiCompatible
        );
    }

    #[test]
    fn anthropic_disabled_no_fields() {
        let mut body = serde_json::json!({"model": "test"});
        let p = policy(false, ReasoningEffort::None);
        apply_reasoning_to_body(&mut body, LlmProvider::Anthropic, p);
        assert!(body.get("thinking").is_none());
    }

    #[test]
    fn anthropic_enabled_no_budget() {
        let mut body = serde_json::json!({"model": "test"});
        let p = policy(true, ReasoningEffort::Medium);
        apply_reasoning_to_body(&mut body, LlmProvider::Anthropic, p);
        assert_eq!(body["thinking"]["type"], "enabled");
        assert!(body["thinking"].get("budget_tokens").is_none());
    }

    #[test]
    fn anthropic_enabled_with_budget() {
        let mut body = serde_json::json!({"model": "test"});
        let p = ResolvedReasoningPolicy {
            enabled: true,
            effort: ReasoningEffort::High,
            budget_tokens: 10000,
        };
        apply_reasoning_to_body(&mut body, LlmProvider::Anthropic, p);
        assert_eq!(body["thinking"]["type"], "enabled");
        assert_eq!(body["thinking"]["budget_tokens"], 10000);
    }

    #[test]
    fn openai_uses_reasoning_effort() {
        let mut body = serde_json::json!({"model": "gpt-4o"});
        let p = policy(true, ReasoningEffort::High);
        apply_reasoning_to_body(&mut body, LlmProvider::OpenAi, p);
        assert_eq!(body["reasoning_effort"], "high");
        assert!(body.get("thinking").is_none());
    }

    #[test]
    fn deepseek_uses_thinking_field() {
        let mut body = serde_json::json!({"model": "deepseek"});
        let p = policy(true, ReasoningEffort::Low);
        apply_reasoning_to_body(&mut body, LlmProvider::DeepSeek, p);
        assert_eq!(body["thinking"]["type"], "enabled");
        assert!(body.get("reasoning_effort").is_none());
    }

    #[test]
    fn unsupported_error_detection() {
        assert!(looks_like_unsupported_reasoning_error(
            400,
            "unknown parameter: thinking"
        ));
        assert!(looks_like_unsupported_reasoning_error(
            422,
            "unsupported field: reasoning_effort"
        ));
        assert!(!looks_like_unsupported_reasoning_error(
            400,
            "invalid api key"
        ));
        assert!(!looks_like_unsupported_reasoning_error(500, "server error"));
    }
}
