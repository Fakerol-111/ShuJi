use serde::{Deserialize, Serialize};

const API_URL: &str = "https://api.anthropic.com/v1/messages";

pub struct AnthropicClient {
    api_key: String,
    model: String,
}

#[derive(Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<MessageItem>,
}

#[derive(Serialize, Deserialize)]
struct MessageItem {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

impl AnthropicClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "claude-sonnet-4-20250514".into(),
        }
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }

    pub async fn send_message(
        &self,
        system_prompt: &str,
        messages: &[crate::models::message::Message],
    ) -> anyhow::Result<String> {
        let mut api_messages: Vec<MessageItem> = messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| MessageItem {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();

        // Ensure we have at least one user message
        if api_messages.is_empty() {
            api_messages.push(MessageItem {
                role: "user".into(),
                content: "请继续".into(),
            });
        }

        let body = MessagesRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system: system_prompt.to_string(),
            messages: api_messages,
        };

        let client = reqwest::Client::new();
        let resp = client
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("API error ({}): {}", status, text);
        }

        let data: MessagesResponse = resp.json().await?;
        let text = data.content
            .into_iter()
            .filter(|b| b.block_type == "text")
            .filter_map(|b| b.text)
            .collect::<Vec<_>>()
            .join("\n");

        Ok(text)
    }
}
