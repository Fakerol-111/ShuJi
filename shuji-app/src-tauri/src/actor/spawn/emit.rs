//! 向前端发送皇帝消息、解析 `<options>`、部门日志。
//!
//! 三个辅助函数：
//! - `emit_to_emperor` — 将 agent 输出经选项解析后发往前端
//! - `parse_options`   — 提取 `<options>` 标签生成可点击按钮
//! - `log_dept`        — 向 DeptStatusPanel 发送状态日志

use tokio::sync::mpsc;

use crate::models::chat::ChatMessage;
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
    let role_name = role.name();
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return;
    }
    let (clean_content, options) = parse_options(trimmed);
    let mut msg = ChatMessage::new(role_name, &clean_content);
    msg.options = options;
    if let Err(e) = tx.try_send(msg) {
        log_console!("[actor] emperor_tx.try_send failed ({}): {}", role_name, e);
    }
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
/// 4. 从原始内容中移除整个 options 块
pub(super) fn parse_options(content: &str) -> (String, Vec<crate::models::chat::ChatOption>) {
    let mut options = Vec::new();
    // 找到 <options> ... </options> 块
    if let Some(start) = content.find("<options>") {
        if let Some(end) = content.find("</options>") {
            let block = &content[start..end + "</options>".len()];
            // 逐项提取 <option key="X" label="Y" description="Z" />
            let mut pos = 0;
            while let Some(opt_start) = block[pos..].find("<option ") {
                let abs_start = pos + opt_start;
                if let Some(opt_end) = block[abs_start..].find("/>") {
                    let tag = &block[abs_start..abs_start + opt_end + 2];
                    let key = extract_attr(tag, "key").unwrap_or_default();
                    let label = extract_attr(tag, "label").unwrap_or_default();
                    let desc = extract_attr(tag, "description").unwrap_or_default();
                    if !key.is_empty() {
                        options.push(crate::models::chat::ChatOption {
                            key,
                            label,
                            description: desc,
                        });
                    }
                    pos = abs_start + opt_end + 2;
                } else {
                    break;
                }
            }
            // 从内容中移除整个 options 块
            let clean = format!(
                "{}{}",
                content[..start].trim(),
                content[end + "</options>".len()..].trim()
            );
            return (clean, options);
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
/// 查找模式 `{attr}="` → 定位值起点 → 查找下一个 `"` 定位终点 → 返回中间字符串。
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    if let Some(val_start) = tag.find(&pattern) {
        let val_begin = val_start + pattern.len();
        if let Some(val_end) = tag[val_begin..].find('\"') {
            return Some(tag[val_begin..val_begin + val_end].to_string());
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
