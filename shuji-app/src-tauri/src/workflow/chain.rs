//! ChainEngine: execution chain resolution and injection.
//!
//! When 尚书令 receives a task, ChainEngine injects the execution steps
//! into the session so the LLM knows which departments to route to and
//! in what order.

/// A single step in an execution chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainStep {
    /// Chinese role name to route to, e.g. "吏部", "兵部".
    pub role: &'static str,
}

/// An execution chain — the ordered list of departments 尚书令 routes through.
#[derive(Debug, Clone)]
pub struct ExecutionChain {
    pub id: &'static str,
    pub steps: &'static [ChainStep],
}

/// Built-in execution chains.
const BUILTIN_CHAINS: &[ExecutionChain] = &[
    // greenfield_full: full pipeline — design → contract → code → test → standards
    ExecutionChain {
        id: "greenfield_full",
        steps: &[
            ChainStep { role: "吏部" },
            ChainStep { role: "兵部" },
            ChainStep { role: "工部" },
            ChainStep { role: "刑部" },
            ChainStep { role: "礼部" },
        ],
    },
    // brownfield_patch: minimal — code → test
    ExecutionChain {
        id: "brownfield_patch",
        steps: &[ChainStep { role: "工部" }, ChainStep { role: "刑部" }],
    },
];

/// ChainEngine: lookup and inject execution chains.
pub struct ChainEngine;

impl ChainEngine {
    /// Look up a chain by ID.
    pub fn get_chain(id: &str) -> Option<&'static ExecutionChain> {
        BUILTIN_CHAINS.iter().find(|c| c.id == id)
    }

    /// Build a human-readable injection string for the 尚书令 session.
    /// This is designed to be injected AFTER the base prompt.
    pub fn build_injection(chain_id: &str) -> Option<String> {
        let chain = Self::get_chain(chain_id)?;
        let steps: Vec<String> = chain
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{}. {}", i + 1, s.role))
            .collect();
        Some(format!(
            "[Execution Chain: {}]\n\
             执行步骤:\n{}\n\
             按顺序路由到每个部门。每步通过后才能进入下一步。",
            chain.id,
            steps.join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greenfield_full_chain() {
        let chain = ChainEngine::get_chain("greenfield_full").unwrap();
        assert_eq!(chain.steps.len(), 5);
        assert_eq!(chain.steps[0].role, "吏部");
        assert_eq!(chain.steps[1].role, "兵部");
        assert_eq!(chain.steps[2].role, "工部");
        assert_eq!(chain.steps[3].role, "刑部");
        assert_eq!(chain.steps[4].role, "礼部");
    }

    #[test]
    fn test_brownfield_patch_chain() {
        let chain = ChainEngine::get_chain("brownfield_patch").unwrap();
        assert_eq!(chain.steps.len(), 2);
        assert_eq!(chain.steps[0].role, "工部");
        assert_eq!(chain.steps[1].role, "刑部");
    }

    #[test]
    fn test_unknown_chain() {
        assert!(ChainEngine::get_chain("nonexistent").is_none());
    }

    #[test]
    fn test_build_injection_greenfield() {
        let inj = ChainEngine::build_injection("greenfield_full").unwrap();
        assert!(inj.contains("Execution Chain"));
        assert!(inj.contains("吏部"));
        assert!(inj.contains("礼部"));
        assert!(inj.contains("5."));
    }

    #[test]
    fn test_build_injection_brownfield() {
        let inj = ChainEngine::build_injection("brownfield_patch").unwrap();
        assert!(inj.contains("Execution Chain"));
        assert!(inj.contains("工部"));
        assert!(inj.contains("刑部"));
        assert!(!inj.contains("吏部"));
    }
}
