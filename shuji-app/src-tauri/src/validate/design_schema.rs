//! Design output block schema validation.
//!
//! Validates that design documents contain a properly structured `## 输出块`
//! section with required fields: conclusions, pending_issues, refs.

/// Validate the output block of a design document body.
/// Extracts YAML/JSON from `## 输出块` section and validates required fields.
/// Phase 1: basic field existence check. Phase 2: JSON Schema validation.
pub fn validate_design_output_block(body: &str) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    // Find the ## 输出块 section
    let block_start = match body.find("## 输出块") {
        Some(pos) => pos,
        None => {
            errors.push("缺少 ## 输出块 章节".to_string());
            return Err(errors);
        }
    };

    let after_header = &body[block_start + "## 输出块".len()..];

    // Find the next heading or end of document
    let block_content = if let Some(next_heading) = after_header.find("\n## ") {
        &after_header[..next_heading]
    } else {
        after_header
    };

    // Check for required sections in the block content
    if !block_content.contains("conclusions") && !block_content.contains("结论") {
        errors.push("输出块缺少 conclusions/结论 字段".to_string());
    }
    if !block_content.contains("pending_issues") && !block_content.contains("待处理") {
        errors.push("输出块缺少 pending_issues/待处理 字段".to_string());
    }
    if !block_content.contains("refs") && !block_content.contains("引用") {
        errors.push("输出块缺少 refs/引用 字段".to_string());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_design_output_block() {
        let body = r#"
# Design doc

Some content here.

## 输出块

conclusions:
  - 使用 REST API 架构
pending_issues:
  - 数据库选型待定
refs:
  - reqs_001
  - dsgn_002
"#;
        assert!(validate_design_output_block(body).is_ok());
    }

    #[test]
    fn test_missing_output_block() {
        let body = "# Design doc\n\nNo output block here.";
        let result = validate_design_output_block(body);
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].contains("缺少"));
    }

    #[test]
    fn test_missing_required_fields() {
        let body = r#"
## 输出块

some_field: value
"#;
        let result = validate_design_output_block(body);
        assert!(result.is_err());
    }

    #[test]
    fn test_chinese_field_names() {
        let body = r#"
## 输出块

结论:
  - 使用微服务
待处理:
  - API 版本管理
引用:
  - reqs_001
"#;
        assert!(validate_design_output_block(body).is_ok());
    }
}
