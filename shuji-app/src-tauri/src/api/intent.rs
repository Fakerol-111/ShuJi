use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// Agent's intent declaration (transparently constructed by the interception layer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    pub intent_id: String,
    pub agent: String,
    pub tool: String,
    pub params: serde_json::Value,
    pub session_id: String,
    pub timestamp: String,
}

/// Result of checking an intent against a rule.
#[derive(Debug, Clone)]
pub enum IntentVerdict {
    Allow,
    Reject { reason: String, rule_id: String },
    RequireApproval { reason: String },
}

/// A composable checker in the intent validation chain.
#[async_trait::async_trait]
pub trait IntentChecker: Send + Sync {
    async fn check(&self, intent: &Intent, working_dir: &Path) -> IntentVerdict;
}

// ── Checker: BoundaryChecker ──────────────────────────────────

/// Checks that the agent is allowed to call the tool and that parameters
/// match the expected schema. Currently uses a simplified allow-list model;
/// in Phase 3 this is replaced by ContractBoundaryChecker from AGENT_CONTRACT.yaml.
pub struct BoundaryChecker;

#[async_trait::async_trait]
impl IntentChecker for BoundaryChecker {
    async fn check(&self, _intent: &Intent, _working_dir: &Path) -> IntentVerdict {
        // Phase 2 simplified version — always allow.
        // Phase 3 replaces this with ContractBoundaryChecker.
        IntentVerdict::Allow
    }
}

// ── Checker: ImmutabilityChecker ──────────────────────────────

/// Prevents modification of documents that are referenced by downstream docs.
pub struct ImmutabilityChecker;

#[async_trait::async_trait]
impl IntentChecker for ImmutabilityChecker {
    async fn check(&self, intent: &Intent, wd: &Path) -> IntentVerdict {
        let write_doc_tools = ["modify_document", "append_document", "set_document_status"];
        if !write_doc_tools.contains(&intent.tool.as_str()) {
            return IntentVerdict::Allow;
        }
        let doc_id = match intent.params.get("id").and_then(|v| v.as_str()) {
            Some(id) if !id.is_empty() => id,
            _ => return IntentVerdict::Allow,
        };

        // plan/revw content changes use soft revert in crud.rs — skip immutability gate
        let type_prefix = doc_id.split('_').next().unwrap_or("");
        if ["plan", "revw"].contains(&type_prefix)
            && ["modify_document", "append_document"].contains(&intent.tool.as_str())
        {
            return IntentVerdict::Allow;
        }

        let refs = crate::audit::check_immutability(wd, doc_id).await;
        if refs.is_empty() {
            IntentVerdict::Allow
        } else {
            IntentVerdict::Reject {
                reason: format!(
                    "文档 {} 被下游 {} 个文档引用，不可修改: {:?}",
                    doc_id,
                    refs.len(),
                    refs
                ),
                rule_id: "IMMUTABILITY".into(),
            }
        }
    }
}

// ── Checker: ApprovalChecker ──────────────────────────────────

/// Checks that referenced documents are approved before routing to exec departments.
pub struct ApprovalChecker;

#[async_trait::async_trait]
impl IntentChecker for ApprovalChecker {
    async fn check(&self, intent: &Intent, wd: &Path) -> IntentVerdict {
        if intent.tool != "route_to" {
            return IntentVerdict::Allow;
        }
        let exec_depts = ["尚书令", "吏部", "兵部", "工部", "刑部", "礼部"];
        let to = match intent.params.get("to").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => return IntentVerdict::Allow,
        };
        if !exec_depts.contains(&to) {
            return IntentVerdict::Allow;
        }
        let subject = match intent.params.get("subject").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s,
            _ => return IntentVerdict::Allow,
        };
        match crate::tool::documents::check_doc_refs_approved_for_route(wd, subject).await {
            Ok(_) => IntentVerdict::Allow,
            Err(msg) => IntentVerdict::Reject {
                reason: msg,
                rule_id: "APPROVAL_GATE".into(),
            },
        }
    }
}

// ── Checker: RateLimiter ──────────────────────────────────────

/// Prevents rapid repeated calls to the same tool with the same key arg.
/// Partially replaces the watchdog's same-tool detection by rejecting
/// before execution instead of post-hoc hint injection.
pub struct RateLimiter {
    recent: Arc<Mutex<VecDeque<(String, String, Instant)>>>,
    max_repeat: usize,
    window_secs: u64,
}

impl RateLimiter {
    pub fn new(max_repeat: usize, window_secs: u64) -> Self {
        Self {
            recent: Arc::new(Mutex::new(VecDeque::new())),
            max_repeat,
            window_secs,
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new(30, 30)
    }
}

#[async_trait::async_trait]
impl IntentChecker for RateLimiter {
    async fn check(&self, intent: &Intent, _wd: &Path) -> IntentVerdict {
        let key_arg = intent
            .params
            .get("path")
            .or_else(|| intent.params.get("id"))
            .or_else(|| intent.params.get("command"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let key = format!("{}:{}:{}", intent.agent, intent.tool, key_arg);

        let mut recent = self.recent.lock().unwrap();
        let now = Instant::now();

        while recent
            .front()
            .map(|(_, _, t)| t.elapsed().as_secs() > self.window_secs)
            .unwrap_or(false)
        {
            recent.pop_front();
        }

        let count = recent.iter().filter(|(k, _, _)| k == &key).count();
        recent.push_back((key, intent.intent_id.clone(), now));

        if count >= self.max_repeat {
            return IntentVerdict::Reject {
                reason: format!(
                    "工具 {} 在 {} 秒内重复调用超过 {} 次（参数: {}）",
                    intent.tool, self.window_secs, self.max_repeat, key_arg
                ),
                rule_id: "RATE_LIMIT".into(),
            };
        }

        IntentVerdict::Allow
    }
}

// ── Helper: wrap tool execution with intent checking ──────────

/// Run the intent checkers against the tool call, then execute via dispatch if allowed.
/// Replaces `crate::tool::execute_named_tool(name, working_dir, args, dept)` in agent code.
pub async fn check_and_execute(
    name: &str,
    args: &serde_json::Value,
    working_dir: &Path,
    dept: &str,
    checkers: &[Box<dyn IntentChecker>],
    full_intent_log: bool,
) -> String {
    let intent = Intent {
        intent_id: uuid::Uuid::new_v4().to_string(),
        agent: dept.to_string(),
        tool: name.to_string(),
        params: args.clone(),
        session_id: dept.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    for checker in checkers {
        match checker.check(&intent, working_dir).await {
            IntentVerdict::Allow => continue,
            IntentVerdict::Reject { reason, rule_id } => {
                crate::audit::append(
                    working_dir,
                    "intent_rejected",
                    dept,
                    "",
                    &format!("{}: {}", rule_id, reason),
                )
                .await;
                return crate::tool::ToolOutput::error(name, "", &rule_id, &reason);
            }
            IntentVerdict::RequireApproval { reason } => {
                return crate::tool::ToolOutput::error(name, "", "requires_approval", &reason);
            }
        }
    }

    let result = crate::tool::execute_named_tool(name, working_dir, args, dept).await;

    if full_intent_log {
        let ok = result.contains("\"ok\":true");
        crate::audit::append(
            working_dir,
            "intent_executed",
            dept,
            "",
            &format!("{} → ok:{}", name, ok),
        )
        .await;
    }

    result
}

/// Factory function to create the standard checker chain.
/// ContractBoundaryChecker is always included (built-in defaults + optional YAML).
pub fn build_default_checkers(
    esaa_enabled: bool,
    working_dir: &Path,
) -> Arc<Vec<Box<dyn IntentChecker>>> {
    let shuji_dir = working_dir.join(".shuji");
    let checkers: Vec<Box<dyn IntentChecker>> = vec![
        Box::new(crate::config::esaa_contract::ContractBoundaryChecker::new(
            &shuji_dir,
        )),
        Box::new(ImmutabilityChecker),
        Box::new(ApprovalChecker),
        Box::new(if esaa_enabled {
            RateLimiter::default()
        } else {
            RateLimiter::new(50, 30)
        }),
    ];
    Arc::new(checkers)
}
