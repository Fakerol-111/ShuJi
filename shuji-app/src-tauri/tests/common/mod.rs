#![allow(dead_code)]

pub mod fixtures;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use shuji_app_lib::actor::ActorMessage;
use shuji_app_lib::config::RuntimeConfig;
use shuji_app_lib::models::role::Role;
use shuji_app_lib::pipeline::engine::PipelineEngine;
use shuji_app_lib::pipeline::{PipelinePlan, PlanStep};
use tokio::sync::mpsc;

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
    // On Windows, both root and resolved should be canonicalized before comparison
    // because resolve_scoped_path canonicalizes existing paths (which resolves 8.3
    // short names like RUNNER~1 → runneradmin), while root from tempfile retains
    // the original TEMP env var format. Canonicalizing both ensures consistency.

    // Normalize by removing \\?\ prefix if present (Windows)
    let normalize = |p: std::path::PathBuf| -> std::path::PathBuf {
        let s = p.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            std::path::PathBuf::from(stripped)
        } else {
            p
        }
    };

    // Canonicalize root to resolve 8.3 short names on Windows CI
    let canon_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());

    // For resolved, canonicalize if the file exists (matches resolve_scoped_path behavior);
    // otherwise strip the original root and reconstruct using canonicalized root
    let canon_resolved = if resolved.exists() {
        std::fs::canonicalize(resolved).unwrap_or_else(|_| resolved.to_path_buf())
    } else if let Ok(rel) = resolved.strip_prefix(root) {
        canon_root.join(rel)
    } else {
        resolved.to_path_buf()
    };

    let norm_root = normalize(canon_root.clone());
    let norm_resolved = normalize(canon_resolved.clone());

    assert!(
        norm_resolved.starts_with(&norm_root),
        "Path {:?} is not within root {:?}",
        norm_resolved,
        norm_root,
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

// ── Mock Pipeline Actor Harness ─────────────────────────────────

/// Simulates department actors for pipeline testing.
pub struct MockActorHarness {
    pub senders: HashMap<Role, mpsc::UnboundedSender<ActorMessage>>,
    _handles: Vec<tokio::task::JoinHandle<()>>,
}

impl MockActorHarness {
    /// Default mock output per role (appended doc id for pipeline artifact extraction).
    fn default_output(role: Role, role_name: &str, subject: &str) -> String {
        let doc = match role {
            Role::Zhongshuling => " plan_1",
            Role::MenxiaShizhong => " revw_1",
            _ => "",
        };
        format!("mock {role_name} completed: {subject}{doc}")
    }

    pub fn with_roles(roles: &[Role]) -> Self {
        let mut senders = HashMap::new();
        let mut handles = Vec::new();
        for role in roles {
            let (tx, mut rx) = mpsc::unbounded_channel::<ActorMessage>();
            senders.insert(*role, tx);
            let role_copy = *role;
            let role_name = role.name().to_string();
            let handle = tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    if let Some(reply) = msg.reply_to {
                        let body = Self::default_output(role_copy, &role_name, &msg.subject);
                        let _ = reply.send(body);
                    }
                }
            });
            handles.push(handle);
        }
        Self {
            senders,
            _handles: handles,
        }
    }

    pub fn all_roles() -> Self {
        Self::with_roles(&[
            Role::Neige,
            Role::Zhongshuling,
            Role::MenxiaShizhong,
            Role::Shangshuling,
            Role::LiBuShangshu,
            Role::BingbuShangshu,
            Role::GongbuShangshu,
            Role::XingbuShangshu,
            Role::LiBuRShangshu,
        ])
    }
}

/// Create a PipelineEngine with mock actors.
pub fn make_pipeline_engine(
    plan: PipelinePlan,
    harness: &MockActorHarness,
    dir: &Path,
) -> PipelineEngine {
    PipelineEngine::new(
        plan,
        harness.senders.clone(),
        Arc::new(HashMap::new()),
        Arc::new(std::sync::Mutex::new(HashMap::new())),
        Arc::new(AtomicBool::new(false)),
        dir.to_path_buf(),
        None,
        Arc::new(RuntimeConfig::default()),
    )
}

/// Build a PipelinePlan with a single self_execute step.
pub fn self_execute_plan(handler: &str, params: serde_json::Value) -> PipelinePlan {
    let mut ap = serde_json::json!({"handler": handler});
    if let Some(obj) = params.as_object() {
        for (k, v) in obj {
            ap[k] = v.clone();
        }
    }
    PipelinePlan {
        plan_id: "plan-test".into(),
        summary: "test".into(),
        estimated_complexity: "low".into(),
        created: "2026-06-13T12:00:00".into(),
        steps: vec![PlanStep {
            step_id: "s1".into(),
            description: "test step".into(),
            action: "self_execute".into(),
            action_params: ap,
            depends_on: vec![],
            require_approval: false,
            on_failure: "wake_cabinet".into(),
            retry: 1,
        }],
    }
}

/// Create a minimal Rust project for testing.
pub async fn create_mini_rust_project(dir: &Path) {
    tokio::fs::write(
        dir.join("Cargo.toml"),
        r#"
[package]
name = "test_crate"
version = "0.1.0"
edition = "2021"
"#,
    )
    .await
    .unwrap();
    let src = dir.join("src");
    tokio::fs::create_dir_all(&src).await.unwrap();
    tokio::fs::write(
        src.join("lib.rs"),
        r#"
pub fn greet() -> &'static str { "hello" }
#[test] fn test_greet() { assert_eq!(greet(), "hello"); }
"#,
    )
    .await
    .unwrap();
}

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
