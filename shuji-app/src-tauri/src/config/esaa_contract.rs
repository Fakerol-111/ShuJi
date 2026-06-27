use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// Loaded from `.shuji/esaa/AGENT_CONTRACT.yaml` — defines tool/route/path
/// permissions for each role. Replaces the hardcoded BoundaryChecker in Phase 3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContracts {
    pub roles: HashMap<String, RoleContract>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleContract {
    pub allowed_tools: Option<Vec<String>>,
    pub forbidden_tools: Option<Vec<String>>,
    pub allowed_paths: Option<Vec<String>>,
    pub forbidden_routes: Option<Vec<String>>,
    pub max_create_file_size: Option<usize>,
    pub max_tool_calls_per_round: Option<usize>,
}

impl AgentContracts {
    /// Load contracts from `.shuji/esaa/AGENT_CONTRACT.yaml`.
    /// Returns empty contracts if the file doesn't exist or is unparseable.
    pub fn load(shuji_dir: &Path) -> Self {
        let path = shuji_dir.join("esaa").join("AGENT_CONTRACT.yaml");
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => {
                return Self {
                    roles: HashMap::new(),
                }
            }
        };
        serde_yaml::from_str(&content).unwrap_or(Self {
            roles: HashMap::new(),
        })
    }

    /// Resolve role name to contract key (supports Role::from_name aliases + sub-agents).
    pub fn role_key(role: &str) -> Option<&'static str> {
        if let Some(r) = crate::models::role::Role::from_name(role) {
            return Some(match r {
                crate::models::role::Role::Neige => "neige",
                crate::models::role::Role::Zhongshuling => "zhongshuling",
                crate::models::role::Role::MenxiaShizhong => "menxiashizhong",
                crate::models::role::Role::Shangshuling => "shangshuling",
                crate::models::role::Role::LiBuShangshu => "libushangshu",
                crate::models::role::Role::BingbuShangshu => "bingbushangshu",
                crate::models::role::Role::GongbuShangshu => "gongbushangshu",
                crate::models::role::Role::XingbuShangshu => "xingbushangshu",
                crate::models::role::Role::LiBuRShangshu => "liburshangshu",
            });
        }
        match role.to_lowercase().as_str() {
            "requirements_agent" => Some("requirements_agent"),
            "survey_agent" => Some("survey_agent"),
            _ => None,
        }
    }

    /// Get YAML override for a role, if present.
    pub fn for_role(&self, role: &str) -> Option<&RoleContract> {
        let role_key = Self::role_key(role)?;
        self.roles.get(role_key)
    }

    /// Effective contract = built-in defaults merged with optional YAML override.
    pub fn effective_for_role(&self, role: &str) -> Option<RoleContract> {
        let key = Self::role_key(role)?;
        let base = builtin_contract_for(key);
        match self.roles.get(key) {
            Some(overlay) => Some(merge_contracts(base, overlay)),
            None => Some(base),
        }
    }
}

/// Built-in contracts used when ESAA is off or YAML has no entry for a role.
pub fn builtin_agent_contracts() -> AgentContracts {
    let mut roles = HashMap::new();
    for key in [
        "neige",
        "zhongshuling",
        "menxiashizhong",
        "shangshuling",
        "libushangshu",
        "bingbushangshu",
        "gongbushangshu",
        "xingbushangshu",
        "liburshangshu",
        "requirements_agent",
        "survey_agent",
    ] {
        roles.insert(key.to_string(), builtin_contract_for(key));
    }
    AgentContracts { roles }
}

fn merge_contracts(base: RoleContract, overlay: &RoleContract) -> RoleContract {
    RoleContract {
        allowed_tools: overlay.allowed_tools.clone().or(base.allowed_tools),
        forbidden_tools: merge_string_lists(base.forbidden_tools, overlay.forbidden_tools.clone()),
        allowed_paths: overlay.allowed_paths.clone().or(base.allowed_paths),
        forbidden_routes: merge_string_lists(
            base.forbidden_routes,
            overlay.forbidden_routes.clone(),
        ),
        max_create_file_size: overlay.max_create_file_size.or(base.max_create_file_size),
        max_tool_calls_per_round: overlay
            .max_tool_calls_per_round
            .or(base.max_tool_calls_per_round),
    }
}

fn merge_string_lists(a: Option<Vec<String>>, b: Option<Vec<String>>) -> Option<Vec<String>> {
    match (a, b) {
        (None, None) => None,
        (Some(x), None) => Some(x),
        (None, Some(y)) => Some(y),
        (Some(mut x), Some(y)) => {
            for item in y {
                if !x.contains(&item) {
                    x.push(item);
                }
            }
            Some(x)
        }
    }
}

const FILE_WRITE_TOOLS: &[&str] = &[
    "create_file",
    "edit_file",
    "apply_patch",
    "modify_file",
    "append_file",
    "delete_file",
    "rename_file",
];
const EXEC_TOOLS: &[&str] = &["execute_command", "run_tests", "run_lint", "setup_test_env"];
const APPROVAL_TOOLS: &[&str] = &["set_document_status"];
const PIPELINE_PLAN_TOOLS: &[&str] = &["submit_pipeline_plan", "update_pipeline_plan"];
const GONGBU_PLAN_TOOLS: &[&str] = &["submit_plan", "complete_task"];
const DESIGN_FORBIDDEN: &[&str] = &[
    "assign_task",
    "cancel_agent",
    "create_skill",
    "update_soul",
    "expand_requirements",
    "survey_codebase",
];

fn forbidden(names: &[&str]) -> Option<Vec<String>> {
    Some(names.iter().map(|s| (*s).to_string()).collect())
}

fn concat_forbidden(slices: &[&[&str]]) -> Option<Vec<String>> {
    let mut out = Vec::new();
    for slice in slices {
        for name in *slice {
            if !out.iter().any(|x: &String| x == name) {
                out.push((*name).to_string());
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

pub fn builtin_contract_for(role_key: &str) -> RoleContract {
    let design_doc = || RoleContract {
        allowed_tools: None,
        forbidden_tools: concat_forbidden(&[
            FILE_WRITE_TOOLS,
            EXEC_TOOLS,
            APPROVAL_TOOLS,
            PIPELINE_PLAN_TOOLS,
            GONGBU_PLAN_TOOLS,
            DESIGN_FORBIDDEN,
        ]),
        allowed_paths: None,
        forbidden_routes: Some(vec!["内阁".into()]),
        max_create_file_size: None,
        max_tool_calls_per_round: None,
    };

    match role_key {
        "neige" => RoleContract {
            forbidden_tools: concat_forbidden(&[
                FILE_WRITE_TOOLS,
                EXEC_TOOLS,
                APPROVAL_TOOLS,
                GONGBU_PLAN_TOOLS,
            ]),
            forbidden_routes: Some(vec!["内阁".into()]),
            ..RoleContract::default_empty()
        },
        "zhongshuling" | "menxiashizhong" | "libushangshu" => design_doc(),
        "bingbushangshu" => design_doc(),
        "shangshuling" => RoleContract {
            forbidden_tools: concat_forbidden(&[
                FILE_WRITE_TOOLS,
                EXEC_TOOLS,
                APPROVAL_TOOLS,
                PIPELINE_PLAN_TOOLS,
                GONGBU_PLAN_TOOLS,
            ]),
            ..RoleContract::default_empty()
        },
        "liburshangshu" => RoleContract {
            forbidden_tools: concat_forbidden(&[
                FILE_WRITE_TOOLS,
                &["execute_command", "run_tests", "setup_test_env"],
                APPROVAL_TOOLS,
                PIPELINE_PLAN_TOOLS,
                GONGBU_PLAN_TOOLS,
                &["assign_task", "cancel_agent"],
            ]),
            ..RoleContract::default_empty()
        },
        "gongbushangshu" => RoleContract {
            forbidden_tools: concat_forbidden(&[
                &["execute_command"],
                APPROVAL_TOOLS,
                PIPELINE_PLAN_TOOLS,
                &["assign_task", "cancel_agent", "route_to"],
            ]),
            ..RoleContract::default_empty()
        },
        "xingbushangshu" => RoleContract {
            forbidden_tools: concat_forbidden(&[
                &["execute_command"],
                APPROVAL_TOOLS,
                PIPELINE_PLAN_TOOLS,
                GONGBU_PLAN_TOOLS,
                &["assign_task", "cancel_agent", "route_to"],
            ]),
            ..RoleContract::default_empty()
        },
        "requirements_agent" | "survey_agent" => RoleContract {
            forbidden_tools: forbidden(&[
                "execute_command",
                "run_tests",
                "set_document_status",
                "route_to",
                "submit_pipeline_plan",
            ]),
            ..RoleContract::default_empty()
        },
        _ => RoleContract::default_empty(),
    }
}

impl RoleContract {
    fn default_empty() -> Self {
        Self {
            allowed_tools: None,
            forbidden_tools: None,
            allowed_paths: None,
            forbidden_routes: None,
            max_create_file_size: None,
            max_tool_calls_per_round: None,
        }
    }
}

/// Minimum dispatch-layer gate (always on, even when ESAA is disabled).
pub fn check_dispatch_tool_gate(dept: &str, tool: &str) -> Result<(), String> {
    let contracts = builtin_agent_contracts();
    let Some(contract) = contracts.effective_for_role(dept) else {
        return Ok(());
    };
    if contract.is_tool_allowed(tool) {
        Ok(())
    } else {
        Err(format_tool_denial(dept, tool))
    }
}

pub fn format_tool_denial(role: &str, tool: &str) -> String {
    let hint = match tool {
        "create_file" | "edit_file" | "apply_patch" | "modify_file" | "append_file"
        | "delete_file" | "rename_file" => {
            "设计/审查部门不应直接改代码。请产出文档或 route/assign 给工部/兵部执行。"
        }
        "execute_command" | "run_tests" | "run_lint" | "setup_test_env" => {
            "该部门不负责执行命令或跑测试。工部用 run_tests，刑部负责集成测试。"
        }
        "set_document_status" => "朱批仅由皇帝在 UI 中准奏，agent 不可调用 set_document_status。",
        "submit_pipeline_plan" | "update_pipeline_plan" => "仅内阁可提交/修改 pipeline 计划。",
        "submit_plan" | "complete_task" => "批次计划工具仅工部尚书可用。",
        "assign_task" => "assign_task 仅尚书令可用。",
        "route_to" => "执行部门应通过尚书令调度，不要自行 route_to。",
        _ => "请改用本部门职责范围内的工具，或 route/assign 给对应部门。",
    };
    format!("角色「{role}」无权调用 {tool}。{hint}")
}

impl RoleContract {
    /// Check if the tool is allowed. Respects forbidden_tools first, then allowed_tools.
    pub fn is_tool_allowed(&self, tool: &str) -> bool {
        if let Some(forbidden) = &self.forbidden_tools {
            if forbidden.iter().any(|t| t == tool) {
                return false;
            }
        }
        if let Some(allowed) = &self.allowed_tools {
            allowed.iter().any(|t| t == tool)
        } else {
            true
        }
    }

    /// Check if the path is within allowed paths.
    pub fn is_path_allowed(&self, path: &str) -> bool {
        match &self.allowed_paths {
            Some(patterns) => patterns.iter().any(|p| simple_glob_match(p, path)),
            None => true,
        }
    }

    /// Check if routing to the target is allowed.
    pub fn is_route_allowed(&self, target: &str) -> bool {
        match &self.forbidden_routes {
            Some(forbidden) => !forbidden.iter().any(|r| r == target),
            None => true,
        }
    }
}

/// Simple glob matching supporting `**` and `*` wildcards.
fn simple_glob_match(pattern: &str, path: &str) -> bool {
    if pattern == "**" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path.starts_with(prefix) || path == prefix;
    }
    if let Some(prefix) = pattern.strip_suffix("/*") {
        let prefix = prefix.trim_end_matches('/');
        if let Some(rest) = path.strip_prefix(prefix) {
            let rest = rest.trim_start_matches('/');
            return !rest.contains('/');
        }
        return false;
    }
    if pattern.contains('*') {
        let re_pattern = format!("^{}$", regex_like(pattern));
        regex_like_match(&re_pattern, path)
    } else {
        path == pattern
    }
}

fn regex_like(pattern: &str) -> String {
    let mut re = String::new();
    for ch in pattern.chars() {
        match ch {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            '.' => re.push_str("\\."),
            '/' => re.push('/'),
            other => re.push(other),
        }
    }
    re
}

fn regex_like_match(pattern: &str, text: &str) -> bool {
    // Simple regex-like matching without regex crate dependency
    // Uses a basic recursive approach for small patterns
    let p = pattern.as_bytes();
    let s = text.as_bytes();
    let plen = p.len();
    let slen = s.len();
    let mut dp = vec![vec![false; slen + 1]; plen + 1];
    dp[0][0] = true;
    for i in 1..=plen {
        if p[i - 1] == b'*' {
            dp[i][0] = dp[i - 1][0];
        }
    }
    for i in 1..=plen {
        for j in 1..=slen {
            if p[i - 1] == b'*' {
                dp[i][j] = dp[i - 1][j] || dp[i][j - 1];
            } else if p[i - 1] == b'.' {
                dp[i][j] = dp[i - 1][j - 1];
            } else {
                dp[i][j] = dp[i - 1][j - 1] && p[i - 1] == s[j - 1];
            }
        }
    }
    dp[plen][slen]
}

/// Checker that reads from AGENT_CONTRACT.yaml.
/// Supports hot-reload — checks file mtime every 30 seconds.
pub struct ContractBoundaryChecker {
    contracts: Mutex<AgentContracts>,
    last_load: Mutex<Instant>,
    shuji_dir: std::path::PathBuf,
    reload_interval: std::time::Duration,
}

impl ContractBoundaryChecker {
    pub fn new(shuji_dir: &Path) -> Self {
        Self {
            contracts: Mutex::new(AgentContracts::load(shuji_dir)),
            last_load: Mutex::new(Instant::now()),
            shuji_dir: shuji_dir.to_path_buf(),
            reload_interval: std::time::Duration::from_secs(30),
        }
    }

    fn maybe_reload(&self) {
        let mut last = self.last_load.lock().unwrap();
        if last.elapsed() >= self.reload_interval {
            let mut contracts = self.contracts.lock().unwrap();
            *contracts = AgentContracts::load(&self.shuji_dir);
            *last = Instant::now();
        }
    }

    pub fn check_tool(&self, role: &str, tool: &str) -> Result<(), String> {
        self.maybe_reload();
        let contracts = self.contracts.lock().unwrap();
        let Some(contract) = contracts.effective_for_role(role) else {
            return Ok(());
        };
        if !contract.is_tool_allowed(tool) {
            return Err(format_tool_denial(role, tool));
        }
        Ok(())
    }

    pub fn check_route(&self, role: &str, target: &str) -> Result<(), String> {
        self.maybe_reload();
        let contracts = self.contracts.lock().unwrap();
        let Some(contract) = contracts.effective_for_role(role) else {
            return Ok(());
        };
        if !contract.is_route_allowed(target) {
            return Err(format!(
                "角色「{}」禁止路由到 {}。请 route/assign 给职责范围内的下游部门。",
                role, target
            ));
        }
        Ok(())
    }

    pub fn check_path(&self, role: &str, path: &str) -> Result<(), String> {
        self.maybe_reload();
        let contracts = self.contracts.lock().unwrap();
        let Some(contract) = contracts.effective_for_role(role) else {
            return Ok(());
        };
        if !contract.is_path_allowed(path) {
            return Err(format!(
                "路径 {} 不在角色「{}」允许范围内。请检查 AGENT_CONTRACT.yaml 中的 allowed_paths。",
                path, role
            ));
        }
        Ok(())
    }

    pub fn check_file_size(&self, role: &str, content: &str) -> Result<(), String> {
        self.maybe_reload();
        let contracts = self.contracts.lock().unwrap();
        let Some(contract) = contracts.effective_for_role(role) else {
            return Ok(());
        };
        if let Some(max) = contract.max_create_file_size {
            if content.len() > max {
                return Err(format!(
                    "content 长度 {} 超过上限 {} 字符",
                    content.len(),
                    max
                ));
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl crate::api::intent::IntentChecker for ContractBoundaryChecker {
    async fn check(
        &self,
        intent: &crate::api::intent::Intent,
        _working_dir: &std::path::Path,
    ) -> crate::api::intent::IntentVerdict {
        use crate::api::intent::IntentVerdict;

        if let Err(msg) = self.check_tool(&intent.agent, &intent.tool) {
            return IntentVerdict::Reject {
                reason: msg,
                rule_id: "CONTRACT_TOOL".into(),
            };
        }

        if intent.tool == "route_to" {
            if let Some(to) = intent.params.get("to").and_then(|v| v.as_str()) {
                if let Err(msg) = self.check_route(&intent.agent, to) {
                    return IntentVerdict::Reject {
                        reason: msg,
                        rule_id: "CONTRACT_ROUTE".into(),
                    };
                }
            }
        }

        if let Some(path) = intent.params.get("path").and_then(|v| v.as_str()) {
            if let Err(msg) = self.check_path(&intent.agent, path) {
                return IntentVerdict::Reject {
                    reason: msg,
                    rule_id: "CONTRACT_PATH".into(),
                };
            }
        }

        if intent.tool == "create_file" {
            if let Some(content) = intent.params.get("content").and_then(|v| v.as_str()) {
                if let Err(msg) = self.check_file_size(&intent.agent, content) {
                    return IntentVerdict::Reject {
                        reason: msg,
                        rule_id: "CONTRACT_FILESIZE".into(),
                    };
                }
            }
        }

        IntentVerdict::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_contract_yaml(dir: &std::path::Path, yaml: &str) {
        let esaa_dir = dir.join(".shuji").join("esaa");
        std::fs::create_dir_all(&esaa_dir).unwrap();
        let mut f = std::fs::File::create(esaa_dir.join("AGENT_CONTRACT.yaml")).unwrap();
        f.write_all(yaml.as_bytes()).unwrap();
    }

    #[test]
    fn test_load_contract() {
        let dir = tempfile::tempdir().unwrap();
        create_contract_yaml(
            dir.path(),
            r#"
roles:
  gongbushangshu:
    allowed_tools:
      - create_file
      - read_file
    forbidden_tools:
      - delete_file
    allowed_paths:
      - "src/**"
    forbidden_routes:
      - "内阁"
    max_create_file_size: 8000
"#,
        );
        let contracts = AgentContracts::load(&dir.path().join(".shuji"));
        let contract = contracts.effective_for_role("gongbushangshu").unwrap();
        assert!(contract.is_tool_allowed("create_file"));
        assert!(contract.is_tool_allowed("read_file"));
        assert!(!contract.is_tool_allowed("delete_file"));
        assert!(!contract.is_tool_allowed("modify_file"));
        assert!(contract.is_path_allowed("src/main.rs"));
        assert!(!contract.is_path_allowed("config.toml"));
        assert!(!contract.is_route_allowed("内阁"));
        assert!(contract.is_route_allowed("礼部"));
    }

    #[test]
    fn test_for_role_pascal_case() {
        let dir = tempfile::tempdir().unwrap();
        create_contract_yaml(
            dir.path(),
            r#"
roles:
  gongbushangshu:
    allowed_tools:
      - create_file
"#,
        );
        let contracts = AgentContracts::load(&dir.path().join(".shuji"));
        assert!(contracts.for_role("Gongbushangshu").is_some());
        let effective = contracts.effective_for_role("GONGBUSHANGSHU").unwrap();
        assert!(effective.is_tool_allowed("create_file"));
    }

    #[test]
    fn test_no_contract_fallback_uses_builtin() {
        let contracts = AgentContracts {
            roles: HashMap::new(),
        };
        assert!(contracts.for_role("nonexistent").is_none());
        let effective = contracts.effective_for_role("中书令").unwrap();
        assert!(!effective.is_tool_allowed("create_file"));
        assert!(effective.is_tool_allowed("read_document"));
    }

    #[test]
    fn test_builtin_dispatch_gate_blocks_designer_code_write() {
        let err = check_dispatch_tool_gate("中书令", "create_file").unwrap_err();
        assert!(err.contains("create_file"));
        assert!(check_dispatch_tool_gate("工部尚书", "create_file").is_ok());
    }

    #[test]
    fn test_builtin_blocks_set_document_status() {
        assert!(check_dispatch_tool_gate("工部尚书", "set_document_status").is_err());
    }

    #[test]
    fn test_tool_allow_deny() {
        let contract = RoleContract {
            allowed_tools: Some(vec!["read_file".into(), "create_file".into()]),
            forbidden_tools: Some(vec!["delete_file".into()]),
            allowed_paths: None,
            forbidden_routes: None,
            max_create_file_size: None,
            max_tool_calls_per_round: None,
        };
        assert!(contract.is_tool_allowed("read_file"));
        assert!(contract.is_tool_allowed("create_file"));
        assert!(!contract.is_tool_allowed("delete_file"));
        assert!(!contract.is_tool_allowed("modify_file"));
    }

    #[test]
    fn test_path_allow() {
        let contract = RoleContract {
            allowed_tools: None,
            forbidden_tools: None,
            allowed_paths: Some(vec!["src/**".into(), "tests/**".into()]),
            forbidden_routes: None,
            max_create_file_size: None,
            max_tool_calls_per_round: None,
        };
        assert!(contract.is_path_allowed("src/main.rs"));
        assert!(contract.is_path_allowed("tests/test.rs"));
        assert!(!contract.is_path_allowed("config.toml"));
    }

    #[test]
    fn test_route_allow() {
        let contract = RoleContract {
            allowed_tools: None,
            forbidden_tools: None,
            allowed_paths: None,
            forbidden_routes: Some(vec!["内阁".into()]),
            max_create_file_size: None,
            max_tool_calls_per_round: None,
        };
        assert!(!contract.is_route_allowed("内阁"));
        assert!(contract.is_route_allowed("工部"));
    }

    #[test]
    fn test_simple_glob_match() {
        assert!(simple_glob_match("src/**", "src/main.rs"));
        assert!(simple_glob_match("src/**", "src/lib/mod.rs"));
        assert!(!simple_glob_match("src/**", "config.toml"));
        assert!(simple_glob_match("tests/**", "tests/test.rs"));
        assert!(!simple_glob_match("tests/*", "tests/sub/test.rs"));
    }
}
