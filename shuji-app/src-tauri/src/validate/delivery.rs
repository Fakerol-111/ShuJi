//! validate_delivery — main orchestration function.
//!
//! Runs configured checks sequentially, produces a ValidationReport,
//! persists to `.shuji/validate/latest.json`, and writes an audit event.

use std::path::Path;

use crate::validate::report::*;
use crate::validate::tests_runner::run_test_gate;

/// Main validation entry point. Called by Pipeline `self_execute` handler
/// or directly via Tauri command.
pub async fn validate_delivery(
    working_dir: &Path,
    opts: &DeliveryOptions,
) -> Result<ValidationReport, String> {
    // 1. Load config
    let config = load_validate_config(working_dir).await;

    if !config.enabled {
        return Ok(ValidationReport {
            ts: chrono::Local::now().to_rfc3339(),
            project_type: detect_project_type_str(working_dir).await,
            overall_pass: true,
            checks: vec![],
            ctrt_id: opts.ctrt_id.clone(),
        });
    }

    let mut checks: Vec<CheckResult> = Vec::new();

    // 2. Run tests gate
    if config.tests.required {
        let test_result = run_test_gate(working_dir, &config).await;
        checks.push(test_result);
    }

    // 3. Contract diff (optional)
    if opts.run_contract_diff {
        let ctrt_id = opts.ctrt_id.as_deref().unwrap_or("");
        if !ctrt_id.is_empty() {
            let contract_result = run_contract_diff_gate(working_dir, ctrt_id).await;
            checks.push(contract_result);
        }
    }

    // 4. Lint (optional)
    if opts.run_lint {
        let lint_result = run_lint_gate(working_dir, &config).await;
        checks.push(lint_result);
    }

    let overall_pass = checks.iter().all(|c| c.pass);

    let project_type = detect_project_type_str(working_dir).await;

    let report = ValidationReport {
        ts: chrono::Local::now().to_rfc3339(),
        project_type,
        overall_pass,
        checks,
        ctrt_id: opts.ctrt_id.clone(),
    };

    // 5. Persist report
    persist_report(working_dir, &report).await?;

    // 6. Audit event
    crate::audit::append(
        working_dir,
        "validate_delivery",
        "系统",
        &report.ts,
        &format!(
            "验证 {}: overall_pass={} ({} checks)",
            report.project_type,
            report.overall_pass,
            report.checks.len()
        ),
    )
    .await;

    Ok(report)
}

/// Load `validate_config.json` or return defaults.
async fn load_validate_config(working_dir: &Path) -> ValidateConfig {
    let path = working_dir.join(".shuji").join("validate_config.json");
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => ValidateConfig::default(),
    }
}

/// Detect project type by file presence.
async fn detect_project_type_str(working_dir: &Path) -> String {
    if working_dir.join("Cargo.toml").exists() {
        "rust".to_string()
    } else if working_dir.join("package.json").exists() {
        "node".to_string()
    } else if working_dir.join("pyproject.toml").exists() || working_dir.join("setup.py").exists() {
        "python".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Run contract diff check (stub for phase 1 — actual parsing comes later).
async fn run_contract_diff_gate(working_dir: &Path, ctrt_id: &str) -> CheckResult {
    let contract_path = working_dir
        .join(".shuji")
        .join("contracts")
        .join(format!("{}.md", ctrt_id));

    if !contract_path.exists() {
        return CheckResult {
            name: "contract_api".into(),
            pass: true,
            summary: format!("契约 {} 不存在，跳过契约检查", ctrt_id),
            details: serde_json::json!({"skipped": true, "reason": "contract_not_found"}),
        };
    }

    // Phase 1: check file exists only; detailed parsing/diff in §1.5-1.6
    CheckResult {
        name: "contract_api".into(),
        pass: true,
        summary: format!("契约 {} 存在", ctrt_id),
        details: serde_json::json!({"ctrt_id": ctrt_id, "phase1": true}),
    }
}

/// Run lint gate (stub for phase 1 — detailed lint in §1.7).
async fn run_lint_gate(working_dir: &Path, _config: &ValidateConfig) -> CheckResult {
    let project_type = detect_project_type_str(working_dir).await;

    let cmd = match project_type.as_str() {
        "rust" => "cargo clippy -- -D warnings",
        "node" => "npm run lint",
        "python" => "ruff check .",
        _ => "",
    };

    if cmd.is_empty() {
        return CheckResult {
            name: "lint".into(),
            pass: true,
            summary: format!("不支持的项目类型 {}，跳过 lint", project_type),
            details: serde_json::json!({"skipped": true}),
        };
    }

    let timeout = std::time::Duration::from_secs(300);
    let (shell, shell_args) = crate::tool::command_ops::get_shell();

    match crate::tool::command_ops::execute_with_timeout(
        shell,
        &shell_args,
        cmd,
        working_dir,
        timeout,
    )
    .await
    {
        Ok(output) => {
            let exit_code = output.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let pass = exit_code == 0;

            CheckResult {
                name: "lint".into(),
                pass,
                summary: if pass {
                    "lint 通过".to_string()
                } else {
                    format!("lint 失败 (exit={})", exit_code)
                },
                details: serde_json::json!({
                    "exit_code": exit_code,
                    "stdout": &stdout.chars().take(2000).collect::<String>(),
                    "stderr": &stderr.chars().take(2000).collect::<String>(),
                }),
            }
        }
        Err(e) => CheckResult {
            name: "lint".into(),
            pass: false,
            summary: format!("lint 执行失败: {}", e),
            details: serde_json::json!({"error": e}),
        },
    }
}

/// Persist validation report to `.shuji/validate/latest.json`.
async fn persist_report(working_dir: &Path, report: &ValidationReport) -> Result<(), String> {
    let dir = working_dir.join(".shuji").join("validate");
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("创建 validate 目录失败: {}", e))?;

    let path = dir.join("latest.json");
    let content =
        serde_json::to_string_pretty(report).map_err(|e| format!("序列化报告失败: {}", e))?;
    tokio::fs::write(&path, &content)
        .await
        .map_err(|e| format!("写入报告失败: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validate_delivery_with_passing_tests() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();

        // Minimal cargo project with passing test
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
#[cfg(test)]
mod tests {
    #[test]
    fn it_works() { assert_eq!(2 + 2, 4); }
}
"#,
        )
        .await
        .unwrap();

        let opts = DeliveryOptions::default();
        let report = validate_delivery(dir, &opts).await.unwrap();

        assert!(report.overall_pass);
        assert_eq!(report.project_type, "rust");
        assert!(report.checks.iter().any(|c| c.name == "tests" && c.pass));
    }

    #[tokio::test]
    async fn test_persist_report_creates_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();

        let report = ValidationReport {
            ts: "2026-06-13T12:00:00Z".into(),
            project_type: "rust".into(),
            overall_pass: true,
            checks: vec![],
            ctrt_id: None,
        };

        persist_report(dir, &report).await.unwrap();

        let path = dir.join(".shuji").join("validate").join("latest.json");
        assert!(path.exists(), "report should be persisted");

        let loaded: ValidationReport =
            serde_json::from_str(&tokio::fs::read_to_string(&path).await.unwrap()).unwrap();
        assert_eq!(loaded.project_type, "rust");
        assert!(loaded.overall_pass);
    }
}
