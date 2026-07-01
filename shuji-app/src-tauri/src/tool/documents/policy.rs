//! allowed_doc_types — department-to-document-type whitelist.

/// 返回某部门允许创建的文档类型白名单。
/// 返回 None 表示「不限制」（对未知部门保守放行，避免误伤）。
pub(crate) fn allowed_doc_types(dept: &str) -> Option<&'static [&'static str]> {
    use crate::models::role::Role;
    match dept.to_lowercase().as_str() {
        "requirements_agent" => return Some(&["reqs", "task"]),
        "survey_agent" => return Some(&["anls"]),
        _ => {}
    }
    let role = Role::from_name(dept)?;
    Some(match role {
        Role::Neige => &["task"],
        Role::Zhongshuling => &["dsgn", "plan", "pdsg", "anls", "precepts"],
        Role::MenxiaShizhong => &["revw"],
        Role::Shangshuling => &["task", "rprt"],
        Role::LiBuShangshu => &["ddtl", "pdsg"],
        Role::BingbuShangshu => &["ctrt", "rprt"],
        Role::GongbuShangshu => &["rprt"],
        Role::XingbuShangshu => &["rprt"],
        Role::LiBuRShangshu => &["rprt"],
    })
}
