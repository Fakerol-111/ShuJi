//! WorkflowResolver: merges intent, governance, and routing heuristics
//! into an ActiveProfile that drives the rest of the system.

use serde::{Deserialize, Serialize};
use std::path::Path;

use super::config::WorkflowConfig;
use super::profile::{build_active, ActiveProfile};
use crate::agent::neige::routing::{self, Confidence, RoutingSuggestion};

/// Intent: what kind of task this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Intent {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "greenfield_standard")]
    GreenfieldStandard,
    #[serde(rename = "brownfield_optimize")]
    BrownfieldOptimize,
    #[serde(rename = "bugfix")]
    Bugfix,
    #[serde(rename = "demo")]
    Demo,
}

impl Intent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Intent::Auto => "auto",
            Intent::GreenfieldStandard => "greenfield_standard",
            Intent::BrownfieldOptimize => "brownfield_optimize",
            Intent::Bugfix => "bugfix",
            Intent::Demo => "demo",
        }
    }

    /// Map a routing skill (suggested by heuristic) to a profile id.
    pub fn from_routing_skill(skill: &str) -> Option<&'static str> {
        match skill {
            "workflow_standard" | "workflow_simple" | "workflow_complex" => {
                Some("greenfield_standard")
            }
            "workflow_optimize" => Some("brownfield_optimize"),
            "workflow_bugfix" => Some("bugfix"),
            "workflow_demo" => Some("demo"),
            _ => None,
        }
    }
}

impl Default for Intent {
    fn default() -> Self {
        Intent::Auto
    }
}

/// Governance: how thorough the process should be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Governance {
    #[serde(rename = "full")]
    Full,
    #[serde(rename = "standard")]
    Standard,
    #[serde(rename = "fast")]
    Fast,
    #[serde(rename = "audit")]
    Audit,
}

impl Governance {
    pub fn as_str(&self) -> &'static str {
        match self {
            Governance::Full => "full",
            Governance::Standard => "standard",
            Governance::Fast => "fast",
            Governance::Audit => "audit",
        }
    }
}

impl Default for Governance {
    fn default() -> Self {
        Governance::Standard
    }
}

/// Result of resolving a workflow intent into a profile.
#[derive(Debug)]
pub struct ResolveResult {
    pub profile: ActiveProfile,
    /// Hint to inject into the 内阁 session after resolver resolution.
    /// Used for auto mode with Low confidence — tells LLM to use `<options>`.
    pub hint: Option<String>,
    /// Whether the resolver has hard-locked the profile (intent != auto)
    /// vs. leaving it as a suggestion.
    pub locked: bool,
}

/// Maps routing confidence to a resolver action.
impl ResolveResult {
    fn locked(profile: ActiveProfile) -> Self {
        Self {
            profile,
            hint: None,
            locked: true,
        }
    }

    fn suggested(profile: ActiveProfile, hint: String) -> Self {
        Self {
            profile,
            hint: Some(hint),
            locked: false,
        }
    }
}

/// WorkflowResolver: main entry point.
pub struct WorkflowResolver;

impl WorkflowResolver {
    /// Resolve a config + task description into an ActiveProfile + hints.
    ///
    /// **Hard mode** (intent != Auto): force-inject the matching profile's
    /// cabinet skill; skip routing.rs hints.
    ///
    /// **Auto mode**: use routing.rs heuristic, then map the suggestion
    /// to a profile. High → lock; Medium → suggest; Low → force `<options>`.
    pub async fn resolve(
        config: &WorkflowConfig,
        project_dir: &Path,
        task_description: &str,
    ) -> ResolveResult {
        let effective = config.effective_intent();
        let governance = config.governance;

        match effective {
            // ── Hard mode: intent explicitly set ──
            Intent::GreenfieldStandard => {
                let ap = build_active("greenfield_standard", governance)
                    .expect("greenfield_standard profile should exist");
                ResolveResult::locked(ap)
            }
            Intent::BrownfieldOptimize => {
                let ap = build_active("brownfield_optimize", governance)
                    .expect("brownfield_optimize profile should exist");
                ResolveResult::locked(ap)
            }
            Intent::Bugfix => {
                let ap = build_active("bugfix", governance).expect("bugfix profile should exist");
                ResolveResult::locked(ap)
            }
            Intent::Demo => {
                let ap = build_active("demo", governance).expect("demo profile should exist");
                ResolveResult::locked(ap)
            }
            // ── Auto mode: use routing heuristic ──
            Intent::Auto => Self::resolve_auto(governance, task_description, project_dir).await,
        }
    }

    /// Auto mode: delegate to routing.rs, map to profile & confidence action.
    async fn resolve_auto(
        governance: Governance,
        task_description: &str,
        _project_dir: &Path,
    ) -> ResolveResult {
        match routing::suggest_workflow(task_description) {
            None
            | Some(RoutingSuggestion {
                skill: _,
                confidence: Confidence::Low,
            }) => {
                // Low confidence or no signal: force `<options>` — never silently fallback
                let ap = build_active("greenfield_standard", governance)
                    .expect("greenfield_standard profile should exist");
                ResolveResult {
                    profile: ap,
                    hint: Some(
                        "[Workflow Decision Required]\n\
                         The task type is not clear from keywords. \
                         You MUST use the <options> tag to present workflow choices to the emperor. \
                         Do NOT silently proceed with a default workflow. \
                         Available intents: greenfield_standard (新功能), \
                         brownfield_optimize (存量优化), bugfix (缺陷修复), demo (快速原型)."
                            .to_string(),
                    ),
                    locked: false,
                }
            }
            Some(RoutingSuggestion {
                skill,
                confidence: Confidence::Medium,
            }) => {
                let profile_id = Intent::from_routing_skill(skill).unwrap_or("greenfield_standard");
                let ap = build_active(profile_id, governance)
                    .unwrap_or_else(|| build_active("greenfield_standard", governance).unwrap());
                let hint = format!(
                    "[Workflow Suggestion: {}]\n\
                     This task appears to match the {} workflow. \
                     Consider activating it with the <skill> tag, \
                     or use <options> if you need clarification.",
                    skill, skill
                );
                ResolveResult::suggested(ap, hint)
            }
            Some(RoutingSuggestion {
                skill,
                confidence: Confidence::High,
            }) => {
                let profile_id = Intent::from_routing_skill(skill).unwrap_or("greenfield_standard");
                let ap = build_active(profile_id, governance)
                    .unwrap_or_else(|| build_active("greenfield_standard", governance).unwrap());
                ResolveResult::locked(ap)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(intent: Intent, governance: Governance) -> WorkflowConfig {
        WorkflowConfig {
            intent,
            governance,
            intent_override: None,
        }
    }

    #[tokio::test]
    async fn test_hard_greenfield() {
        let cfg = make_config(Intent::GreenfieldStandard, Governance::Standard);
        let tmp = tempfile::TempDir::new().unwrap();
        let result = WorkflowResolver::resolve(&cfg, tmp.path(), "实现登录功能").await;
        assert_eq!(result.profile.profile_id, "greenfield_standard");
        assert!(result.locked);
    }

    #[tokio::test]
    async fn test_hard_brownfield() {
        let cfg = make_config(Intent::BrownfieldOptimize, Governance::Standard);
        let tmp = tempfile::TempDir::new().unwrap();
        let result = WorkflowResolver::resolve(&cfg, tmp.path(), "优化性能").await;
        assert_eq!(result.profile.profile_id, "brownfield_optimize");
        assert!(result.locked);
    }

    #[tokio::test]
    async fn test_auto_optimize_task() {
        let cfg = make_config(Intent::Auto, Governance::Standard);
        let tmp = tempfile::TempDir::new().unwrap();
        let result = WorkflowResolver::resolve(&cfg, tmp.path(), "优化登录接口性能").await;
        // Auto + "优化" keyword → routing suggests optimize → Medium confidence
        assert_eq!(result.profile.profile_id, "brownfield_optimize");
        assert!(!result.locked);
        assert!(result.hint.is_some());
    }

    #[tokio::test]
    async fn test_auto_greenfield_task() {
        let cfg = make_config(Intent::Auto, Governance::Standard);
        let tmp = tempfile::TempDir::new().unwrap();
        let result = WorkflowResolver::resolve(&cfg, tmp.path(), "实现用户登录功能").await;
        // Auto + no specific keyword → Low confidence → must options
        assert_eq!(result.profile.profile_id, "greenfield_standard");
        assert!(result.hint.unwrap().contains("<options>"));
        assert!(!result.locked);
    }

    #[tokio::test]
    async fn test_auto_explicit_skill() {
        let cfg = make_config(Intent::Auto, Governance::Standard);
        let tmp = tempfile::TempDir::new().unwrap();
        let result =
            WorkflowResolver::resolve(&cfg, tmp.path(), "请用 workflow_bugfix 处理这个 bug").await;
        // Explicit workflow_bugfix → High confidence → locked
        assert_eq!(result.profile.profile_id, "bugfix");
        assert!(result.locked);
    }
}
