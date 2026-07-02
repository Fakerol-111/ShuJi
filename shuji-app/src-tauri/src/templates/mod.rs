//! Template rendering for document templates (contracts, etc.).
//!
//! Templates use `{{placeholder}}` syntax for variable substitution.
//! All templates are bundled via `include_str!` at compile time.

use std::collections::HashMap;

/// Information about an available template.
#[derive(Debug, Clone)]
pub struct TemplateInfo {
    pub name: String,
    pub description: String,
}

/// List available contract templates.
pub fn list_contract_templates() -> Vec<TemplateInfo> {
    vec![
        TemplateInfo {
            name: "module_api".into(),
            description: "单模块 API 契约模板".into(),
        },
        TemplateInfo {
            name: "integration_scenarios".into(),
            description: "集成测试场景契约模板".into(),
        },
        TemplateInfo {
            name: "rest_endpoint".into(),
            description: "REST 端点契约模板".into(),
        },
    ]
}

/// Render a contract template by name with the given variables.
/// Replaces `{{key}}` placeholders with values from `vars`.
///
/// Returns `None` if the template name is unknown.
pub fn render_contract_template(name: &str, vars: &HashMap<String, String>) -> Option<String> {
    let content = match name {
        "module_api" => include_str!("../../assets/templates/contracts/module_api.md.template"),
        "integration_scenarios" => {
            include_str!("../../assets/templates/contracts/integration_scenarios.md.template")
        }
        "rest_endpoint" => {
            include_str!("../../assets/templates/contracts/rest_endpoint.md.template")
        }
        _ => return None,
    };

    let mut result = content.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("{{{{{}}}}}", key), value);
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_templates() {
        let templates = list_contract_templates();
        assert_eq!(templates.len(), 3);
    }

    #[test]
    fn test_render_module_api() -> anyhow::Result<()> {
        let mut vars = HashMap::new();
        vars.insert("module_name".into(), "user_service".into());
        vars.insert("design_ref".into(), "ddtl_001".into());
        vars.insert(
            "functions_table".into(),
            "| create_user | name, email | User | 创建用户 |".into(),
        );

        let result = render_contract_template("module_api", &vars)?;
        assert!(result.contains("user_service"));
        assert!(result.contains("ddtl_001"));
        assert!(result.contains("create_user"));
        Ok(())
    }

    #[test]
    fn test_render_integration_scenarios() -> anyhow::Result<()> {
        let mut vars = HashMap::new();
        vars.insert("scenario_name".into(), "user_login".into());
        vars.insert("precondition".into(), "用户已注册".into());
        vars.insert("step_1".into(), "调用登录接口".into());
        vars.insert("expected_result".into(), "返回 token".into());

        let result = render_contract_template("integration_scenarios", &vars)?;
        assert!(result.contains("user_login"));
        Ok(())
    }

    #[test]
    fn test_unknown_template() {
        let result = render_contract_template("nonexistent", &HashMap::new());
        assert!(result.is_none());
    }
}
