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

/// Parse the bundled precepts index.json.
/// Logs a warning on parse failure (shouldn't happen in production — it's a compile-time asset).
fn parse_precepts_index() -> serde_json::Value {
    let index_json = include_str!("../../assets/precepts/index.json");
    let result: serde_json::Value = serde_json::from_str(index_json).unwrap_or_else(|e| {
        log_console!("[precepts] bundled index.json parse failed: {}", e);
        serde_json::Value::Null
    });
    result
}

/// Detect which precept files apply to the project at `working_dir`.
/// Checks for key files (Cargo.toml, package.json, etc.) and maps to precept lists.
pub fn detect_precept_files(working_dir: &Path) -> Vec<PreceptFile> {
    let index = parse_precepts_index();

    let Some(detectors) = index["detectors"].as_array() else {
        return vec![];
    };

    let mut result = Vec::new();

    for detector in detectors {
        let Some(files) = detector["files"].as_array() else {
            continue;
        };
        let Some(precepts) = detector["precepts"].as_array() else {
            continue;
        };

        let matched = files.iter().any(|f| {
            let fname = match f.as_str() {
                Some(name) => name,
                None => return false,
            };
            working_dir.join(fname).exists()
        });

        if matched {
            for p in precepts {
                let name = match p.as_str() {
                    Some(n) if !n.is_empty() => n,
                    _ => continue,
                };
                result.push(PreceptFile {
                    file_name: name.to_string(),
                    path: format!("assets/precepts/{}", name),
                });
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
    fn test_detect_precept_files_rust() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;
        std::fs::write(tmp.path().join("Cargo.toml"), "")?;

        let files = detect_precept_files(tmp.path());
        assert!(
            files.iter().any(|f| f.file_name == "rust.toml"),
            "should detect rust precepts"
        );
        assert!(
            files.iter().any(|f| f.file_name == "universal.toml"),
            "should always include universal"
        );
        Ok(())
    }

    #[test]
    fn test_detect_precept_files_node() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;
        std::fs::write(tmp.path().join("package.json"), "{}")?;

        let files = detect_precept_files(tmp.path());
        assert!(files.iter().any(|f| f.file_name == "typescript.toml"));
        assert!(files.iter().any(|f| f.file_name == "universal.toml"));
        Ok(())
    }

    #[test]
    fn test_detect_precept_files_unknown() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;
        let files = detect_precept_files(tmp.path());
        assert!(files.is_empty(), "no files should match: {:?}", files);
        Ok(())
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
    async fn test_init_project_precepts() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;
        init_project_precepts(tmp.path()).await.unwrap();

        let precepts_dir = tmp.path().join(".shuji").join("precepts");
        assert!(precepts_dir.join("index.json").exists());
        assert!(precepts_dir.join("rust.toml").exists());
        assert!(precepts_dir.join("universal.toml").exists());
        Ok(())
    }

    #[test]
    fn test_load_project_rules_integration() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;
        std::fs::write(tmp.path().join("Cargo.toml"), "")?;

        let rules = load_project_rules(tmp.path());
        assert!(
            !rules.is_empty(),
            "should auto-detect and load rust+universal rules"
        );
        Ok(())
    }
}
