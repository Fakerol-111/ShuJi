//! 向前端发送皇帝消息、解析 `<options>`、部门日志。
//!
//! 三个辅助函数：
//! - `emit_to_emperor` — 将 agent 输出经选项解析后发往前端
//! - `parse_options`   — 提取 `<options>` 标签生成可点击按钮
//! - `log_dept`        — 向 DeptStatusPanel 发送状态日志

use tokio::sync::mpsc;

use crate::models::chat::{ChatDocument, ChatMessage, ChatOption};
use crate::models::role::Role;

use super::super::{ActorContext, DeptLogEntry};

/// 将 agent 输出 emit 到皇帝的前端面板。
///
/// 处理步骤：
/// 1. 从 content 中解析 `<options>` 标签 → `ChatOption` 按钮列表
/// 2. 用去除 options 标签后的干净内容构造 `ChatMessage`
/// 3. 通过 `emperor_tx` try_send 发送（不阻塞）
///
/// 空内容直接返回（不 emit 空白消息）。
pub(super) fn emit_to_emperor(tx: &mpsc::Sender<ChatMessage>, role: Role, content: &str) {
    emit_to_emperor_with_options(tx, role, content, &[], &[]);
}

/// 将 agent 输出 emit 到皇帝的前端面板，优先使用 `request_decision` 工具选项。
pub(super) fn emit_to_emperor_with_options(
    tx: &mpsc::Sender<ChatMessage>,
    role: Role,
    content: &str,
    decision_options: &[String],
    documents: &[ChatDocument],
) {
    let role_name = role.name();
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return;
    }
    let (clean_content, options) = if !decision_options.is_empty() {
        (
            trimmed.to_string(),
            build_decision_options(decision_options),
        )
    } else {
        parse_options(trimmed)
    };
    let mut msg = ChatMessage::new(role_name, &clean_content);
    msg.options = options;
    msg.documents = documents.to_vec();
    if let Err(e) = tx.try_send(msg) {
        log_console!("[actor] emperor_tx.try_send failed ({}): {}", role_name, e);
    }
}

/// 将 `request_decision` 工具的选项数组转为前端按钮。
pub(super) fn build_decision_options(opts: &[String]) -> Vec<ChatOption> {
    opts.iter()
        .map(|s| ChatOption {
            key: s.clone(),
            label: s.clone(),
            description: String::new(),
        })
        .collect()
}

/// 从内容中提取 `<options>` 块，返回（干净内容, 选项列表）。
///
/// 内阁输出格式：
/// ```xml
/// <options>
/// <option key="approve" label="批准" description="批准当前设计方案"/>
/// <option key="reject" label="驳回" description="要求中书令重新设计"/>
/// </options>
/// ```
///
/// 解析逻辑：
/// 1. 找到 `<options>` / `</options>` 配对位置
/// 2. 在块内逐个查找 `<option ... />` 标签
/// 3. 提取每个标签的 key/label/description 属性
/// 4. 仅当解析出有效 option 时才从原始内容中移除 options 块
pub(super) fn parse_options(content: &str) -> (String, Vec<ChatOption>) {
    let mut options = Vec::new();
    // 找到 <options> ... </options> 块
    if let Some(start) = content.find("<options>") {
        if let Some(end) = content.find("</options>") {
            let block = &content[start..end + "</options>".len()];
            // 逐项提取 <option ... /> 或 <option ...>
            let mut pos = 0;
            while let Some(opt_start) = block[pos..].find("<option ") {
                let abs_start = pos + opt_start;
                let tag_end = block[abs_start..]
                    .find("/>")
                    .map(|i| abs_start + i + 2)
                    .or_else(|| block[abs_start..].find('>').map(|i| abs_start + i + 1));
                if let Some(end_pos) = tag_end {
                    let tag = &block[abs_start..end_pos];
                    let key = extract_attr(tag, "key").unwrap_or_default();
                    let label = extract_attr(tag, "label").unwrap_or_default();
                    let desc = extract_attr(tag, "description").unwrap_or_default();
                    if !key.is_empty() {
                        options.push(ChatOption {
                            key,
                            label,
                            description: desc,
                        });
                    }
                    pos = end_pos;
                } else {
                    break;
                }
            }
            if !options.is_empty() {
                let clean = format!(
                    "{}{}",
                    content[..start].trim(),
                    content[end + "</options>".len()..].trim()
                );
                return (clean, options);
            }
        }
    }
    (content.to_string(), options)
}

/// 从 XML 风格的标签字符串中提取属性值。
///
/// 例: `extract_attr(tag, "key")`
/// tag = `<option key="approve" label="批准" />`
/// 返回 `Some("approve")`
///
/// 查找模式 `{attr}="` 或 `{attr}='` → 定位值起点 → 查找下一个引号定位终点。
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    for quote in ['\"', '\''] {
        let pattern = format!("{attr}={quote}");
        if let Some(val_start) = tag.find(&pattern) {
            let val_begin = val_start + pattern.len();
            if let Some(val_end) = tag[val_begin..].find(quote) {
                return Some(tag[val_begin..val_begin + val_end].to_string());
            }
        }
    }
    None
}

/// 发送一条部门日志条目到前端 DeptStatusPanel（actor 模块内可见）。
///
/// `pub(in crate::actor)` 限制可见性——只有 `crate::actor` 模块内的代码可以调用。
/// `routing.rs` 中的 `log_dept` 调用就是透过这个 pub 可见的。
pub(in crate::actor) fn log_dept(ctx: &ActorContext, dept: &str, action: &str) {
    if let Err(e) = ctx.dept_log_tx.try_send(DeptLogEntry::new(dept, action)) {
        log_console!("[actor] dept_log_tx.try_send failed ({}): {}", dept, e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_decision_options_uses_full_text_as_key_and_label() {
        let opts = build_decision_options(&["选项A".into(), "选项B".into()]);
        assert_eq!(opts.len(), 2);
        assert_eq!(opts[0].key, "选项A");
        assert_eq!(opts[0].label, "选项A");
        assert_eq!(opts[1].key, "选项B");
    }

    #[test]
    fn parse_options_invalid_block_preserves_content() {
        let content = "请选择:\n<options>\n无效内容\n</options>";
        let (clean, options) = parse_options(content);
        assert!(options.is_empty());
        assert_eq!(clean, content);
    }

    #[test]
    fn parse_options_valid_xml_still_works() {
        let content = r#"摘要
<options>
<option key="approve" label="批准" description="通过"/>
<option key="reject" label="驳回" />
</options>
尾部"#;
        let (clean, options) = parse_options(content);
        assert_eq!(options.len(), 2);
        assert_eq!(options[0].key, "approve");
        assert_eq!(options[1].label, "驳回");
        assert!(!clean.contains("<options>"));
        assert!(clean.contains("摘要"));
    }

    #[test]
    fn parse_options_supports_single_quotes() {
        let content = r#"<options><option key='go' label='继续' /></options>"#;
        let (_, options) = parse_options(content);
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].key, "go");
        assert_eq!(options[0].label, "继续");
    }
}
