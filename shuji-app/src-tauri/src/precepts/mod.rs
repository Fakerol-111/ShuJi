//! Precepts: structured engineering standards that are language-aware.
//!
//! Precepts are `.toml` rule files organized by project type.
//! They are loaded by `PreceptLoader` and consumed by:
//! - `validate::lint` for automated checks
//! - `tool::audit_tools::init_checklist` for 礼部 checklist generation
//! - `tool::lint_ops` for the `run_lint` tool

use std::path::Path;

use serde::Deserialize;

/// A detected precept file on disk.
#[derive(Debug, Clone)]
pub struct PreceptFile {
    pub file_name: String,
    pub path: String,
}

/// A single precept rule.
#[derive(Debug, Clone, Deserialize)]
pub struct PreceptRule {
    pub id: String,
    pub category: String,
    pub description: String,
    pub severity: String,
    pub lint_tool: String,
    #[serde(default)]
    pub lint_args: Vec<String>,
}

/// Container for rules parsed from a precept TOML file.
#[derive(Debug, Clone, Deserialize)]
pub struct PreceptRules {
    #[serde(default)]
    pub rules: Vec<PreceptRule>,
}

/// Detect which precept files apply to the project at `working_dir`.
/// Checks for key files (Cargo.toml, package.json, etc.) and maps to precept lists.
pub fn detect_precept_files(working_dir: &Path) -> Vec<PreceptFile> {
    let index_json = include_str!("../../assets/precepts/index.json");
    let index: serde_json::Value = serde_json::from_str(index_json).unwrap_or_default();

    let detectors = index["detectors"].as_array();
    if detectors.is_none() {
        return vec![];
    }

    let mut result = Vec::new();

    // 已在上方检查 detectors.is_none()，此处安全 unwrap 或改用 if let
    let detectors = match detectors {
        Some(d) => d,
        None => return vec![],
    };

    for detector in detectors {
        let files = detector["files"].as_array();
        let precepts = detector["precepts"].as_array();
        if files.is_none() || precepts.is_none() {
            continue;
        }

        let matched = files
            .map(|fs| {
                fs.iter().any(|f| {
                    let fname = f.as_str().unwrap_or("");
                    working_dir.join(fname).exists()
                })
            })
            .unwrap_or(false);

        if matched {
            if let Some(ps) = precepts {
                for p in ps {
                    let name = p.as_str().unwrap_or("");
                    if !name.is_empty() {
                        result.push(PreceptFile {
                            file_name: name.to_string(),
                            path: format!("assets/precepts/{}", name),
                        });
                    }
                }
            }
        }
    }

    result
}

/// Load rules from the given precept file references.
/// Uses `include_str!` to read bundled assets.
pub fn load_rules(precept_files: &[PreceptFile]) -> Vec<PreceptRule> {
    let mut all_rules = Vec::new();

    for pf in precept_files {
        let content = match pf.file_name.as_str() {
            "rust.toml" => include_str!("../../assets/precepts/rust.toml"),
            "python.toml" => include_str!("../../assets/precepts/python.toml"),
            "typescript.toml" => include_str!("../../assets/precepts/typescript.toml"),
            "universal.toml" => include_str!("../../assets/precepts/universal.toml"),
            _ => continue,
        };

        if let Ok(rules) = toml::from_str::<PreceptRules>(content) {
            all_rules.extend(rules.rules);
        }
    }

    all_rules
}

/// Initialize precepts for a project by copying asset files to `.shuji/precepts/`.
/// Creates the directory if it doesn't exist.
pub async fn init_project_precepts(working_dir: &Path) -> Result<(), String> {
    let target_dir = working_dir.join(".shuji").join("precepts");
    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|e| format!("创建 precepts 目录失败: {}", e))?;

    let files = [
        (
            "index.json",
            include_str!("../../assets/precepts/index.json"),
        ),
        ("rust.toml", include_str!("../../assets/precepts/rust.toml")),
        (
            "python.toml",
            include_str!("../../assets/precepts/python.toml"),
        ),
        (
            "typescript.toml",
            include_str!("../../assets/precepts/typescript.toml"),
        ),
        (
            "universal.toml",
            include_str!("../../assets/precepts/universal.toml"),
        ),
    ];

    for (name, content) in &files {
        let target = target_dir.join(name);
        if !target.exists() {
            tokio::fs::write(&target, content)
                .await
                .map_err(|e| format!("写入 {} 失败: {}", name, e))?;
        }
    }

    Ok(())
}

/// Load rules for the project at working_dir using auto-detection.
pub fn load_project_rules(working_dir: &Path) -> Vec<PreceptRule> {
    let files = detect_precept_files(working_dir);
    load_rules(&files)
}

/// Convert precept rules to audit ChecklistItems.
pub fn rules_to_checklist_items(rules: &[PreceptRule]) -> Vec<crate::audit::ChecklistItem> {
    rules
        .iter()
        .map(|r| crate::audit::ChecklistItem {
            id: r.id.clone(),
            description: format!("[{}] {} — {}", r.severity, r.category, r.description),
            category: r.category.clone(),
            status: "pending".to_string(),
            note: String::new(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_precept_files_rust() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "").unwrap();

        let files = detect_precept_files(tmp.path());
        assert!(
            files.iter().any(|f| f.file_name == "rust.toml"),
            "should detect rust precepts"
        );
        assert!(
            files.iter().any(|f| f.file_name == "universal.toml"),
            "should always include universal"
        );
    }

    #[test]
    fn test_detect_precept_files_node() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("package.json"), "{}").unwrap();

        let files = detect_precept_files(tmp.path());
        assert!(files.iter().any(|f| f.file_name == "typescript.toml"));
        assert!(files.iter().any(|f| f.file_name == "universal.toml"));
    }

    #[test]
    fn test_detect_precept_files_unknown() {
        let tmp = tempfile::TempDir::new().unwrap();
        // No Cargo.toml, no package.json — should only detect universal
        // Actually, with no matching detector, nothing returns
        let files = detect_precept_files(tmp.path());
        assert!(files.is_empty(), "no files should match: {:?}", files);
    }

    #[test]
    fn test_load_rules_rust() {
        let files = vec![
            PreceptFile {
                file_name: "rust.toml".into(),
                path: "assets/precepts/rust.toml".into(),
            },
            PreceptFile {
                file_name: "universal.toml".into(),
                path: "assets/precepts/universal.toml".into(),
            },
        ];
        let rules = load_rules(&files);
        assert!(!rules.is_empty(), "should load rules");
        assert!(rules.iter().any(|r| r.id.starts_with("RUST_")));
        assert!(rules.iter().any(|r| r.id.starts_with("UNIVERSAL_")));
    }

    #[test]
    fn test_rules_to_checklist_items() {
        let rules = vec![PreceptRule {
            id: "RUST_NO_UNWRAP".into(),
            category: "error_handling".into(),
            description: "禁止 unwrap".into(),
            severity: "error".into(),
            lint_tool: "clippy".into(),
            lint_args: vec![],
        }];
        let items = rules_to_checklist_items(&rules);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "RUST_NO_UNWRAP");
        assert_eq!(items[0].status, "pending");
    }

    #[tokio::test]
    async fn test_init_project_precepts() {
        let tmp = tempfile::TempDir::new().unwrap();
        init_project_precepts(tmp.path()).await.unwrap();

        let precepts_dir = tmp.path().join(".shuji").join("precepts");
        assert!(precepts_dir.join("index.json").exists());
        assert!(precepts_dir.join("rust.toml").exists());
        assert!(precepts_dir.join("universal.toml").exists());
    }

    #[test]
    fn test_load_project_rules_integration() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "").unwrap();

        let rules = load_project_rules(tmp.path());
        assert!(
            !rules.is_empty(),
            "should auto-detect and load rust+universal rules"
        );
    }
}
