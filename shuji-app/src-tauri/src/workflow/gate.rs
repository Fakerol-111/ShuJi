//! GateEngine: tool/route interception based on ActiveProfile gate rules.
//!
//! The 内阁 exec closure calls `GateEngine::check_tool(...)` before executing.
//! Violations return a structured error (existing ToolOutput error format) so
//! the LLM can understand why the gate blocked the call.
//!
//! The `--override-skill-gate` escape hatch is preserved (checked in `route_to`
//! subject field, but also respected here).

use super::ActiveProfile;

/// Reason why a gate violation occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateViolation {
    pub message: String,
}

impl GateViolation {
    pub fn to_error_json(&self) -> serde_json::Value {
        serde_json::json!({
            "ok": false,
            "operation": "gate_blocked",
            "path": "",
            "message": self.message,
            "error_code": "skill_short_circuit"
        })
    }
}

/// GateEngine: stateless tool/route gate checker.
pub struct GateEngine;

impl GateEngine {
    /// Check whether `tool_name` with `args` is allowed under the given profile.
    /// Returns `Ok(())` if allowed, `Err(GateViolation)` if blocked.
    pub fn check_tool(
        profile: &ActiveProfile,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> Result<(), GateViolation> {
        // ── Gates: route_to restrictions ──
        if tool_name == "route_to" {
            if let Some(to) = args.get("to").and_then(|v| v.as_str()) {
                // Check override-skill-gate escape hatch
                if let Some(subject) = args.get("subject").and_then(|v| v.as_str()) {
                    if subject.contains("--override-skill-gate") {
                        return Ok(());
                    }
                }
                if profile.gates.forbid_route_to.iter().any(|r| r == to) {
                    return Err(GateViolation {
                        message: format!(
                            "[技能短路] 当前配置「{}」禁止路由到「{}」。禁止路由目标: {:?}。如需强制路由，请在 subject 中包含 --override-skill-gate。",
                            profile.profile_id, to, profile.gates.forbid_route_to
                        ),
                    });
                }
            }
        }

        // ── Gates: tool restrictions ──
        if profile.gates.forbid_tools.iter().any(|t| t == tool_name) {
            return Err(GateViolation {
                message: format!(
                    "[工具门禁] 当前配置「{}」禁止使用工具「{}」。已被禁止的工具: {:?}",
                    profile.profile_id, tool_name, profile.gates.forbid_tools
                ),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::profile::build_active;
    use crate::workflow::Governance;

    fn make_profile(profile_id: &'static str, governance: Governance) -> ActiveProfile {
        build_active(profile_id, governance).expect("test profile should exist")
    }

    #[test]
    fn test_brownfield_forbid_expand_requirements() {
        let p = make_profile("brownfield_optimize", Governance::Standard);
        let args = serde_json::json!({});
        let result = GateEngine::check_tool(&p, "expand_requirements", &args);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("expand_requirements"));
    }

    #[test]
    fn test_brownfield_forbid_menxia() {
        let p = make_profile("brownfield_optimize", Governance::Standard);
        let args = serde_json::json!({"to": "门下侍中", "subject": "test"});
        let result = GateEngine::check_tool(&p, "route_to", &args);
        assert!(result.is_err());
    }

    #[test]
    fn test_brownfield_allow_zhongshu() {
        let p = make_profile("brownfield_optimize", Governance::Standard);
        let args = serde_json::json!({"to": "中书令", "subject": "test"});
        let result = GateEngine::check_tool(&p, "route_to", &args);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bugfix_forbid_zhongshu_and_menxia() {
        let p = make_profile("bugfix", Governance::Standard);
        let args = serde_json::json!({"to": "中书令", "subject": "test"});
        assert!(GateEngine::check_tool(&p, "route_to", &args).is_err());

        let args2 = serde_json::json!({"to": "门下侍中", "subject": "test"});
        assert!(GateEngine::check_tool(&p, "route_to", &args2).is_err());
    }

    #[test]
    fn test_bugfix_allow_gongbu() {
        let p = make_profile("bugfix", Governance::Standard);
        let args = serde_json::json!({"to": "工部", "subject": "test"});
        assert!(GateEngine::check_tool(&p, "route_to", &args).is_ok());
    }

    #[test]
    fn test_override_flag_bypasses_gate() {
        let p = make_profile("bugfix", Governance::Standard);
        let args = serde_json::json!({
            "to": "中书令",
            "subject": "--override-skill-gate"
        });
        assert!(GateEngine::check_tool(&p, "route_to", &args).is_ok());
    }

    #[test]
    fn test_greenfield_standard_no_restrictions() {
        let p = make_profile("greenfield_standard", Governance::Standard);
        // All tools and routes should be allowed
        let result = GateEngine::check_tool(&p, "expand_requirements", &serde_json::json!({}));
        assert!(result.is_ok());

        let args = serde_json::json!({"to": "门下侍中", "subject": "test"});
        assert!(GateEngine::check_tool(&p, "route_to", &args).is_ok());
    }

    #[test]
    fn test_greenfield_fast_overlay() {
        let p = make_profile("greenfield_standard", Governance::Fast);
        // Fast should forbid expand_requirements and 门下侍中
        let result = GateEngine::check_tool(&p, "expand_requirements", &serde_json::json!({}));
        assert!(result.is_err());

        let args = serde_json::json!({"to": "门下侍中", "subject": "test"});
        assert!(GateEngine::check_tool(&p, "route_to", &args).is_err());
    }

    #[test]
    fn test_inspect_tool_always_allowed() {
        let p = make_profile("brownfield_optimize", Governance::Standard);
        // read_file should never be gated
        let result = GateEngine::check_tool(&p, "read_file", &serde_json::json!({}));
        assert!(result.is_ok());
    }
}
