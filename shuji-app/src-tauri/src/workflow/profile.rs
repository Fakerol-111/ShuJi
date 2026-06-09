//! WorkflowProfile definitions with YAML externalization (Phase B).
//!
//! Built-in profiles are defined as YAML files under `profiles/*.yaml` and
//! governance overlays under `profiles/governance/*.yaml`, embedded at
//! compile time via `include_str!`. The `build_active()` function loads
//! the base profile, then merges the governance overlay on top.
//!
//! Falls back to compiled-in Rust static profiles when YAML parsing fails.

use std::path::Path;

use super::Governance;
use crate::workflow::stage::{StageTracker, WorkflowStage};

/// Gate rules: what tools and route destinations are forbidden.
#[derive(Debug, Clone, Default)]
pub struct GateRules {
    pub forbid_tools: Vec<String>,
    pub forbid_route_to: Vec<String>,
}

impl GateRules {
    /// Merge another GateRules into self, adding any missing entries.
    fn merge(&mut self, overlay: &GateRules) {
        for t in &overlay.forbid_tools {
            if !self.forbid_tools.iter().any(|x| x == t) {
                self.forbid_tools.push(t.clone());
            }
        }
        for r in &overlay.forbid_route_to {
            if !self.forbid_route_to.iter().any(|x| x == r) {
                self.forbid_route_to.push(r.clone());
            }
        }
    }
}

/// A compiled profile combining the base profile with governance overlays.
#[derive(Debug, Clone)]
pub struct ActiveProfile {
    pub profile_id: String,
    pub cabinet_skill: String,
    pub execution_chain_id: String,
    pub gates: GateRules,
    pub governance: Governance,
    pub stage_tracker: crate::workflow::stage::StageTracker,
}

impl ActiveProfile {
    /// Apply governance overlay (loaded from YAML) on base gates.
    fn with_governance(
        profile_id: String,
        cabinet_skill: String,
        execution_chain_id: String,
        mut gates: GateRules,
        governance: Governance,
        stage_tracker: crate::workflow::stage::StageTracker,
    ) -> Self {
        // Load governance overlay from YAML and merge
        if let Some(overlay) = load_governance_yaml(governance) {
            gates.merge(&overlay);
        }

        Self {
            profile_id,
            cabinet_skill,
            execution_chain_id,
            gates,
            governance,
            stage_tracker,
        }
    }
}

// ── YAML profile schema ──────────────────────────────────────────

#[derive(Debug, Clone, serde::Deserialize)]
struct YamlProfile {
    id: String,
    #[allow(dead_code)]
    version: Option<u32>,
    #[allow(dead_code)]
    label: Option<String>,
    cabinet_skill: String,
    execution_chain: String,
    gates: YamlGates,
    #[allow(dead_code)]
    escalations: Option<Vec<YamlEscalation>>,
    stages: Option<Vec<YamlStage>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct YamlStage {
    id: String,
    actor: String,
    skill: Option<String>,
    #[serde(rename = "output_doc")]
    output_doc: Option<String>,
    #[serde(default)]
    requires_approval: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct YamlGates {
    forbid_tools: Option<Vec<String>>,
    forbid_route_to: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct YamlEscalation {
    when_keyword: Vec<String>,
    to_intent: String,
}

// ── Governance overlay YAML schema ────────────────────────────

#[derive(Debug, Clone, serde::Deserialize)]
struct YamlGovernance {
    #[allow(dead_code)]
    id: String,
    forbid_tools: Option<Vec<String>>,
    forbid_route_to: Option<Vec<String>>,
}

const EMBEDDED_GOVERNANCE: &[(&str, &str)] = &[
    ("fast", include_str!("profiles/governance/fast.yaml")),
    ("full", include_str!("profiles/governance/full.yaml")),
    (
        "standard",
        include_str!("profiles/governance/standard.yaml"),
    ),
    ("audit", include_str!("profiles/governance/audit.yaml")),
];

/// Load a governance overlay from embedded YAML.
fn load_governance_yaml(governance: Governance) -> Option<GateRules> {
    let key = governance.as_str();
    let yaml_text = EMBEDDED_GOVERNANCE
        .iter()
        .find(|(name, _)| *name == key)
        .map(|(_, content)| *content)?;
    let gov: YamlGovernance = serde_yaml::from_str(yaml_text).ok()?;
    Some(GateRules {
        forbid_tools: gov.forbid_tools.unwrap_or_default(),
        forbid_route_to: gov.forbid_route_to.unwrap_or_default(),
    })
}

const EMBEDDED_PROFILES: &[(&str, &str)] = &[
    (
        "greenfield_standard",
        include_str!("profiles/greenfield_standard.yaml"),
    ),
    (
        "brownfield_optimize",
        include_str!("profiles/brownfield_optimize.yaml"),
    ),
    ("bugfix", include_str!("profiles/bugfix.yaml")),
    ("demo", include_str!("profiles/demo.yaml")),
    (
        "refactor",
        include_str!("profiles/refactor.yaml"),
    ),
    ("audit", include_str!("profiles/audit.yaml")),
];

/// Load a profile from embedded YAML. Returns `None` on parse error.
fn load_yaml_profile(id: &str) -> Option<ActiveProfile> {
    let yaml_text = EMBEDDED_PROFILES
        .iter()
        .find(|(name, _)| *name == id)
        .map(|(_, content)| *content)?;

    let yaml: YamlProfile = serde_yaml::from_str(yaml_text).ok()?;
    let gates = GateRules {
        forbid_tools: yaml.gates.forbid_tools.unwrap_or_default(),
        forbid_route_to: yaml.gates.forbid_route_to.unwrap_or_default(),
    };

    let stage_tracker = parse_stages(yaml.stages.as_ref(), &yaml.id);

    Some(ActiveProfile {
        profile_id: yaml.id,
        cabinet_skill: yaml.cabinet_skill,
        execution_chain_id: yaml.execution_chain,
        gates,
        governance: Governance::Standard, // placeholder, overwritten by build_active
        stage_tracker,
    })
}

/// Parse YAML stages into StageTracker, or generate defaults.
fn parse_stages(yaml_stages: Option<&Vec<YamlStage>>, profile_id: &str) -> StageTracker {
    let stages = match yaml_stages {
        Some(list) => list
            .iter()
            .map(|ys| WorkflowStage {
                id: ys.id.clone(),
                actor: ys.actor.clone(),
                skill: ys.skill.clone(),
                output_doc: ys.output_doc.clone(),
                requires_approval: ys.requires_approval,
                description: ys.id.clone(), // use id as description fallback
            })
            .collect(),
        None => return StageTracker::default_for_profile(profile_id),
    };
    StageTracker::new(stages)
}

// ── Fallback built-in profiles (compile-time Rust, used when YAML fails) ────

struct StaticProfile {
    id: &'static str,
    cabinet_skill: &'static str,
    execution_chain_id: &'static str,
    forbid_tools: &'static [&'static str],
    forbid_route_to: &'static [&'static str],
}

const STATIC_PROFILES: &[StaticProfile] = &[
    StaticProfile {
        id: "greenfield_standard",
        cabinet_skill: "workflow_standard",
        execution_chain_id: "greenfield_full",
        forbid_tools: &[],
        forbid_route_to: &[],
    },
    StaticProfile {
        id: "brownfield_optimize",
        cabinet_skill: "workflow_optimize",
        execution_chain_id: "brownfield_patch",
        forbid_tools: &["expand_requirements"],
        forbid_route_to: &["门下侍中"],
    },
    StaticProfile {
        id: "bugfix",
        cabinet_skill: "workflow_bugfix",
        execution_chain_id: "brownfield_patch",
        forbid_tools: &["expand_requirements"],
        forbid_route_to: &["中书令", "门下侍中"],
    },
    StaticProfile {
        id: "demo",
        cabinet_skill: "workflow_demo",
        execution_chain_id: "brownfield_patch",
        forbid_tools: &["expand_requirements"],
        forbid_route_to: &["中书令", "门下侍中"],
    },
];

fn load_static_profile(id: &str) -> Option<ActiveProfile> {
    let s = STATIC_PROFILES.iter().find(|p| p.id == id)?;
    Some(ActiveProfile {
        profile_id: s.id.to_string(),
        cabinet_skill: s.cabinet_skill.to_string(),
        execution_chain_id: s.execution_chain_id.to_string(),
        gates: GateRules {
            forbid_tools: s.forbid_tools.iter().map(|&s| s.to_string()).collect(),
            forbid_route_to: s.forbid_route_to.iter().map(|&s| s.to_string()).collect(),
        },
        governance: Governance::Standard,
        stage_tracker: StageTracker::default_for_profile(s.id),
    })
}

// ── Public API ───────────────────────────────────────────────────

/// Build an ActiveProfile from a profile id + governance overlay.
/// Tries YAML first; falls back to compile-time Rust static.
pub fn build_active(profile_id: &str, governance: Governance) -> Option<ActiveProfile> {
    let base = load_yaml_profile(profile_id).or_else(|| load_static_profile(profile_id))?;
    Some(ActiveProfile::with_governance(
        base.profile_id,
        base.cabinet_skill,
        base.execution_chain_id,
        base.gates,
        governance,
        base.stage_tracker,
    ))
}

/// Load a profile as a YAML file from disk (for custom/user profiles).
/// Used in Phase C for `.shuji/workflows/custom.yaml`.
pub async fn load_profile_from_disk(path: &Path) -> Option<ActiveProfile> {
    let content = tokio::fs::read_to_string(path).await.ok()?;
    let yaml: YamlProfile = serde_yaml::from_str(&content).ok()?;
    let gates = GateRules {
        forbid_tools: yaml.gates.forbid_tools.unwrap_or_default(),
        forbid_route_to: yaml.gates.forbid_route_to.unwrap_or_default(),
    };
    let stage_tracker = parse_stages(yaml.stages.as_ref(), &yaml.id);
    Some(ActiveProfile {
        profile_id: yaml.id,
        cabinet_skill: yaml.cabinet_skill,
        execution_chain_id: yaml.execution_chain,
        gates,
        governance: Governance::Standard,
        stage_tracker,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_yaml_greenfield() {
        let p = load_yaml_profile("greenfield_standard").unwrap();
        assert_eq!(p.profile_id, "greenfield_standard");
        assert_eq!(p.cabinet_skill, "workflow_standard");
        assert_eq!(p.execution_chain_id, "greenfield_full");
        assert!(p.gates.forbid_tools.is_empty());
        assert!(p.gates.forbid_route_to.is_empty());
    }

    #[test]
    fn test_load_yaml_brownfield() {
        let p = load_yaml_profile("brownfield_optimize").unwrap();
        assert_eq!(p.cabinet_skill, "workflow_optimize");
        assert!(p
            .gates
            .forbid_tools
            .iter()
            .any(|t| t == "expand_requirements"));
        assert!(p.gates.forbid_route_to.iter().any(|r| r == "门下侍中"));
    }

    #[test]
    fn test_load_yaml_bugfix() {
        let p = load_yaml_profile("bugfix").unwrap();
        assert!(p.gates.forbid_route_to.iter().any(|r| r == "中书令"));
    }

    #[test]
    fn test_load_yaml_demo() {
        let p = load_yaml_profile("demo").unwrap();
        assert_eq!(p.cabinet_skill, "workflow_demo");
    }

    #[test]
    fn test_yaml_unknown() {
        assert!(load_yaml_profile("nonexistent").is_none());
    }

    #[test]
    fn test_static_fallback() {
        let p = load_static_profile("greenfield_standard").unwrap();
        assert_eq!(p.profile_id, "greenfield_standard");
    }

    #[test]
    fn test_build_active_from_yaml() {
        let a = build_active("greenfield_standard", Governance::Standard).unwrap();
        assert_eq!(a.profile_id, "greenfield_standard");
        assert_eq!(a.governance, Governance::Standard);
    }

    #[test]
    fn test_build_active_fast_overlay() {
        let a = build_active("greenfield_standard", Governance::Fast).unwrap();
        assert!(a
            .gates
            .forbid_tools
            .iter()
            .any(|t| t == "expand_requirements"));
        assert!(a.gates.forbid_route_to.iter().any(|r| r == "门下侍中"));
    }

    #[test]
    fn test_build_active_brownfield_fast() {
        // Brownfield already forbids expand + 门下 — fast overlay should not duplicate
        let a = build_active("brownfield_optimize", Governance::Fast).unwrap();
        let expand_count = a
            .gates
            .forbid_tools
            .iter()
            .filter(|t| *t == "expand_requirements")
            .count();
        assert_eq!(expand_count, 1, "should not duplicate forbid entries");
    }

    #[test]
    fn test_build_active_unknown() {
        assert!(build_active("nonexistent", Governance::Standard).is_none());
    }
}
