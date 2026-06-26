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

    /// Get contract for a role, supporting both Chinese and English role names.
    pub fn for_role(&self, role: &str) -> Option<&RoleContract> {
        let role_key = match role.to_lowercase().as_str() {
            "工部" | "gongbushangshu" => "gongbushangshu",
            "刑部" | "xingbushangshu" => "xingbushangshu",
            "内阁" | "neige" => "neige",
            "吏部" | "libushangshu" => "libushangshu",
            "兵部" | "bingbushangshu" => "bingbushangshu",
            "礼部" | "liburshangshu" => "liburshangshu",
            "中书令" | "zhongshuling" => "zhongshuling",
            "门下侍中" | "menxiashizhong" => "menxiashizhong",
            "尚书令" | "shangshuling" => "shangshuling",
            _ => return None,
        };
        self.roles.get(role_key)
    }
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
        let contract = match contracts.for_role(role) {
            Some(c) => c,
            None => return Ok(()),
        };
        if !contract.is_tool_allowed(tool) {
            return Err(format!("{} 被禁止使用工具 {}", role, tool));
        }
        Ok(())
    }

    pub fn check_route(&self, role: &str, target: &str) -> Result<(), String> {
        self.maybe_reload();
        let contracts = self.contracts.lock().unwrap();
        let contract = match contracts.for_role(role) {
            Some(c) => c,
            None => return Ok(()),
        };
        if !contract.is_route_allowed(target) {
            return Err(format!("{} 禁止路由到 {}", role, target));
        }
        Ok(())
    }

    pub fn check_path(&self, role: &str, path: &str) -> Result<(), String> {
        self.maybe_reload();
        let contracts = self.contracts.lock().unwrap();
        let contract = match contracts.for_role(role) {
            Some(c) => c,
            None => return Ok(()),
        };
        if !contract.is_path_allowed(path) {
            return Err(format!("路径 {} 不在 {} 的白名单中", path, role));
        }
        Ok(())
    }

    pub fn check_file_size(&self, role: &str, content: &str) -> Result<(), String> {
        self.maybe_reload();
        let contracts = self.contracts.lock().unwrap();
        let contract = match contracts.for_role(role) {
            Some(c) => c,
            None => return Ok(()),
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
        let contract = contracts.for_role("gongbushangshu").unwrap();
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
        assert!(contracts.for_role("GONGBUSHANGSHU").is_some());
    }

    #[test]
    fn test_no_contract_fallback() {
        let contracts = AgentContracts {
            roles: HashMap::new(),
        };
        assert!(contracts.for_role("nonexistent").is_none());
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
