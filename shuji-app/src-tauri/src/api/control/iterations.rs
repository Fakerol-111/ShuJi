use crate::api::client::ToolDefinition;
use crate::config::RuntimeConfig;

/// Check if a tool name is a read-only operation (safe to parallelize).
pub(super) fn is_read_tool(name: &str) -> bool {
    matches!(
        name,
        "read_file"
            | "read_document"
            | "list_dir"
            | "list_dir_tree"
            | "find_document"
            | "search_text"
            | "summarize_logs"
    )
}

/// Iteration budget based on tool set.
pub(super) fn max_iterations_for_tools(
    tools: &[ToolDefinition],
    config: &RuntimeConfig,
    role: &str,
) -> usize {
    // 设计/审核部门需要更充裕的迭代空间来完成读代码→构思→输出的完整链路
    if role == "Zhongshuling" || role == "Menxiashizhong" {
        return config.tool_iterations.write_heavy.max(80);
    }

    let has_write_file = tools.iter().any(|t| {
        matches!(
            t.function.name.as_str(),
            "create_file" | "modify_file" | "append_file" | "delete_file" | "rename_file"
        )
    });
    let has_append_document = tools.iter().any(|t| {
        matches!(
            t.function.name.as_str(),
            "append_document" | "modify_document"
        )
    });

    if has_write_file {
        config.tool_iterations.write_heavy
    } else if has_append_document {
        config.tool_iterations.document_heavy
    } else {
        config.tool_iterations.readonly
    }
}
