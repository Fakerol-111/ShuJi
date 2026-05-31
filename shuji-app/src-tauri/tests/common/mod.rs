pub mod fixtures;

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Create a temporary test directory that will be cleaned up automatically.
pub fn create_temp_dir(name: &str) -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix(&format!("shuji_test_{}_", name))
        .tempdir()
        .expect("Failed to create temp dir")
}

/// Create a test project directory with .shuji structure.
pub fn create_test_project(name: &str) -> tempfile::TempDir {
    let temp = create_temp_dir(name);
    let shuji_dir = temp.path().join(".shuji");

    std::fs::create_dir_all(shuji_dir.join("designs")).unwrap();
    std::fs::create_dir_all(shuji_dir.join("designs/detail")).unwrap();
    std::fs::create_dir_all(shuji_dir.join("reviews")).unwrap();
    std::fs::create_dir_all(shuji_dir.join("tasks")).unwrap();
    std::fs::create_dir_all(shuji_dir.join("contracts")).unwrap();
    std::fs::create_dir_all(shuji_dir.join("reports")).unwrap();
    std::fs::create_dir_all(shuji_dir.join("logs")).unwrap();
    std::fs::create_dir_all(shuji_dir.join("context")).unwrap();

    std::fs::write(shuji_dir.join("_counter"), "0").unwrap();

    temp
}

/// Create a mock AnthropicClient for testing (returns predefined responses).
pub struct MockClient {
    pub responses: Arc<std::sync::Mutex<Vec<serde_json::Value>>>,
}

impl MockClient {
    pub fn new(responses: Vec<serde_json::Value>) -> Self {
        Self {
            responses: Arc::new(std::sync::Mutex::new(responses)),
        }
    }

    pub fn with_text_response(text: &str) -> Self {
        Self::new(vec![mock_text_response(text)])
    }

    pub fn with_tool_response(tool_name: &str, args: serde_json::Value) -> Self {
        Self::new(vec![mock_tool_response(tool_name, args)])
    }
}

/// Create a mock LLM response with text content.
pub fn mock_text_response(content: &str) -> serde_json::Value {
    serde_json::json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": content
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 100,
            "completion_tokens": 50
        }
    })
}

/// Create a mock LLM response with tool calls.
pub fn mock_tool_response(tool_name: &str, args: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "id": "call_test_123",
                    "type": "function",
                    "function": {
                        "name": tool_name,
                        "arguments": serde_json::to_string(&args).unwrap()
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {
            "prompt_tokens": 100,
            "completion_tokens": 50
        }
    })
}

/// Create a mock truncated response (finish_reason=length).
pub fn mock_truncated_response(
    content: &str,
    partial_tool: Option<(&str, &str)>,
) -> serde_json::Value {
    let mut msg = serde_json::json!({
        "role": "assistant",
        "content": content
    });

    if let Some((tool_name, broken_args)) = partial_tool {
        msg["tool_calls"] = serde_json::json!([{
            "id": "call_truncated_123",
            "type": "function",
            "function": {
                "name": tool_name,
                "arguments": broken_args
            }
        }]);
    }

    serde_json::json!({
        "choices": [{
            "message": msg,
            "finish_reason": "length"
        }],
        "usage": {
            "prompt_tokens": 100,
            "completion_tokens": 2048
        }
    })
}

/// Assert that a path is within the project root (for security tests).
pub fn assert_path_within_root(root: &Path, resolved: &Path) {
    // On Windows, canonicalize adds \\?\ prefix, so we need to normalize both paths
    // for comparison. We'll use the non-canonicalized form for comparison.

    // Get absolute paths without canonicalization
    let abs_root = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir().unwrap().join(root)
    };

    let abs_resolved = if resolved.is_absolute() {
        resolved.to_path_buf()
    } else {
        std::env::current_dir().unwrap().join(resolved)
    };

    // Normalize by removing \\?\ prefix if present (Windows)
    let normalize = |p: std::path::PathBuf| -> std::path::PathBuf {
        let s = p.to_string_lossy();
        if s.starts_with(r"\\?\") {
            std::path::PathBuf::from(&s[4..])
        } else {
            p
        }
    };

    let norm_root = normalize(abs_root);
    let norm_resolved = normalize(abs_resolved);

    assert!(
        norm_resolved.starts_with(&norm_root),
        "Path {:?} is not within root {:?}",
        norm_resolved,
        norm_root
    );
}

/// Assert that a path resolution fails with an error message containing the expected text.
pub fn assert_path_error_contains(result: &Result<PathBuf, String>, expected: &str) {
    match result {
        Ok(path) => panic!(
            "Expected error containing '{}', but got Ok({:?})",
            expected, path
        ),
        Err(e) => assert!(
            e.contains(expected),
            "Error message '{}' does not contain '{}'",
            e,
            expected
        ),
    }
}

// ── Mock HTTP API helpers (for wiremock-based E2E tests) ──────

use std::collections::VecDeque;
use std::sync::Mutex;

/// Create a mock OpenAI-format text response.
pub fn mock_api_text(content: &str) -> serde_json::Value {
    serde_json::json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": content
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5
        }
    })
}

/// Create a mock OpenAI-format tool call response.
pub fn mock_api_tool(name: &str, args: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_test_001",
                    "type": "function",
                    "function": {
                        "name": name,
                        "arguments": serde_json::to_string(&args).unwrap()
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5
        }
    })
}

/// A wiremock Responder that serves responses from a FIFO queue.
/// Each call pops the front and returns it. Empty queue returns a fallback
/// text response to prevent test hangs.
///
/// Supports Clone — clones share the same underlying queue via Arc.
#[derive(Clone)]
pub struct MockQueue {
    queue: Arc<Mutex<VecDeque<serde_json::Value>>>,
}

impl MockQueue {
    pub fn new(responses: Vec<serde_json::Value>) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::from(responses))),
        }
    }

    /// Push a response to the back of the queue.
    pub fn push(&self, response: serde_json::Value) {
        self.queue.lock().unwrap().push_back(response);
    }

    /// Number of responses remaining in the queue.
    pub fn remaining(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    /// Mount this queue as the sole responder on the mock server.
    /// Returns the original MockQueue (can still be used to push more responses).
    pub async fn mount(self, server: &wiremock::MockServer) -> Self {
        let clone = self.clone();
        wiremock::Mock::given(wiremock::matchers::any())
            .respond_with(clone)
            .mount(server)
            .await;
        self
    }
}

impl wiremock::Respond for MockQueue {
    fn respond(&self, _request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let response = self.queue.lock().unwrap().pop_front().unwrap_or_else(|| {
            serde_json::json!({
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "处理完成，请继续。"
                    },
                    "finish_reason": "stop"
                }],
                "usage": { "prompt_tokens": 1, "completion_tokens": 1 }
            })
        });
        wiremock::ResponseTemplate::new(200).set_body_json(response)
    }
}
