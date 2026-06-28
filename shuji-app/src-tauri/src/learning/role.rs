use crate::models::role::Role;

/// Normalize external role input to canonical `Role::name()` (PascalCase).
/// Rejects unknown roles and path-like strings — never use raw input in file paths.
pub fn normalize_role_name(input: Option<&str>) -> Result<String, String> {
    let raw = input.unwrap_or("Neige").trim();
    if raw.is_empty() {
        return Err("Role cannot be empty".into());
    }
    Role::from_name(raw)
        .map(|r| r.name().to_string())
        .ok_or_else(|| format!("Unknown role: {raw}"))
}

/// Validate that `role_name` is already a canonical role name.
pub fn ensure_canonical_role(role_name: &str) -> Result<String, String> {
    let canonical = normalize_role_name(Some(role_name))?;
    if canonical != role_name {
        return Err(format!("Role must be canonical name, got: {role_name}"));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_chinese_role_name() {
        assert_eq!(normalize_role_name(Some("工部")).unwrap(), "Gongbushangshu");
    }

    #[test]
    fn reject_unknown_role() {
        assert!(normalize_role_name(Some("unknown-role")).is_err());
    }

    #[test]
    fn reject_path_like_role() {
        assert!(normalize_role_name(Some("../../escape")).is_err());
    }
}
