use std::path::Path;

use crate::tool::{resolve_scoped_path, ToolOutput};

use super::approval::add_pending_approval;
use super::parse::{
    build_doc, dept_to_author, find_rprt_path, next_id, now_iso, parse_doc, resolve_doc_path,
    rprt_rel_path, type_to_dir, DocMeta,
};

/// ── create_document ────────────────────────────────────────────────
pub async fn tool_create_document(
    working_dir: &Path,
    args: &serde_json::Value,
    dept: &str,
) -> String {
    let doc_type = args["type"].as_str().unwrap_or("").to_string();
    if doc_type.is_empty() {
        return ToolOutput::error("create_document", "", "empty_type", "文档类型不能为空");
    }
    let valid_types = [
        "dsgn", "plan", "pdsg", "ddtl", "task", "ctrt", "rprt", "revw", "precepts", "anls", "reqs",
    ];
    if !valid_types.contains(&doc_type.as_str()) {
        return ToolOutput::error(
            "create_document",
            "",
            "invalid_type",
            &format!(
                "无效文档类型: {}，支持的类型: {}",
                doc_type,
                valid_types.join(", ")
            ),
        );
    }

    let refs = args["refs"]
        .as_array()
        .map(|arr| {
            let nums: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_i64())
                .map(|n| n.to_string())
                .collect();
            if nums.is_empty() {
                "[-1]".to_string()
            } else {
                format!("[{}]", nums.join(", "))
            }
        })
        .unwrap_or_else(|| "[-1]".to_string());

    let id_num = match next_id(working_dir).await {
        Ok(n) => n,
        Err(e) => return ToolOutput::error("create_document", "", "counter_error", &e),
    };
    let doc_id = format!("{}_{}", doc_type, id_num);
    let rel_path = if doc_type == "rprt" {
        rprt_rel_path(dept, &doc_id)
    } else {
        let dir = type_to_dir(&doc_type);
        if dir.is_empty() {
            format!(".shuji/{}.md", doc_id)
        } else {
            format!(".shuji/{}/{}.md", dir, doc_id)
        }
    };

    let author = dept_to_author(dept);
    let ts = now_iso();
    let status = match doc_type.as_str() {
        "plan" | "revw" => "in_review".to_string(),
        _ => String::new(),
    };

    let meta = DocMeta {
        id: doc_id.clone(),
        doc_type: doc_type.clone(),
        author: author.to_string(),
        timestamp: ts,
        refs,
        status: status.clone(),
        notes: String::new(),
    };
    let content = build_doc(&meta, "");

    let full = match resolve_scoped_path(working_dir, &rel_path).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("create_document", &doc_id, "path_error", &e),
    };
    if let Some(parent) = full.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    match tokio::fs::write(&full, &content).await {
        Ok(_) => {
            if status == "in_review" {
                let _ = add_pending_approval(working_dir, &doc_id).await;
            }
            let detail = format!("type={}, refs={}", doc_type, meta.refs);
            crate::audit::append(working_dir, "create_document", dept, &doc_id, &detail).await;
            ToolOutput::success(
                "create_document",
                &doc_id,
                &format!(
                    "文档 {} 创建成功\n【重要】后续操作（append/modify/set-status/route）请使用此 ID: {}",
                    doc_id, doc_id
                ),
            )
        }
        Err(e) => ToolOutput::error("create_document", &doc_id, "write_error", &e.to_string()),
    }
}

/// ── update_document ────────────────────────────────────────────────
pub async fn tool_modify_document(
    working_dir: &Path,
    args: &serde_json::Value,
    dept: &str,
) -> String {
    let id = args["id"].as_str().unwrap_or("");
    if id.is_empty() {
        return ToolOutput::error("modify_document", "", "empty_id", "文档 ID 不能为空");
    }

    if let Some(old_text) = args["old_text"].as_str() {
        if old_text.len() > 300 {
            return ToolOutput::error(
                "modify_document",
                id,
                "content_too_long",
                &format!(
                    "old_text 过长（{} 字符），最大 300。请缩小匹配范围。",
                    old_text.len()
                ),
            );
        }
    }
    if let Some(new_text) = args["new_text"].as_str() {
        if new_text.len() > 300 {
            return ToolOutput::error(
                "modify_document",
                id,
                "content_too_long",
                &format!(
                    "new_text 过长（{} 字符），最大 300。请分批修改或使用 append_document。",
                    new_text.len()
                ),
            );
        }
    }

    let type_prefix = id.split('_').next().unwrap_or("");
    let full = if type_prefix == "rprt" {
        match find_rprt_path(working_dir, id).await {
            Some(p) => p,
            None => {
                return ToolOutput::error(
                    "modify_document",
                    id,
                    "not_found",
                    &format!("文档 {} 不存在", id),
                )
            }
        }
    } else {
        let dir = type_to_dir(type_prefix);
        let rel_path = if dir.is_empty() {
            format!(".shuji/{}.md", id)
        } else {
            format!(".shuji/{}/{}.md", dir, id)
        };
        match resolve_scoped_path(working_dir, &rel_path).await {
            Ok(p) => p,
            Err(e) => return ToolOutput::error("modify_document", id, "path_error", &e),
        }
    };
    if !full.exists() {
        return ToolOutput::error(
            "modify_document",
            id,
            "not_found",
            &format!("文档 {} 不存在", id),
        );
    }

    let content = match tokio::fs::read_to_string(&full).await {
        Ok(c) => c,
        Err(e) => return ToolOutput::error("modify_document", id, "read_error", &e.to_string()),
    };

    let (mut meta, body) = match parse_doc(&content) {
        Ok(m) => m,
        Err(e) => return ToolOutput::error("modify_document", id, "parse_error", &e),
    };

    let new_body = if let Some(old_text) = args["old_text"].as_str() {
        let new_text = args["new_text"].as_str().unwrap_or("");
        if old_text.is_empty() {
            return ToolOutput::error("modify_document", id, "empty_old_text", "old_text 不能为空");
        }
        if !body.contains(old_text) {
            return ToolOutput::error(
                "modify_document",
                id,
                "not_found",
                "未在文档正文中找到匹配的文本。请先 read_document 确认内容。",
            );
        }
        body.replacen(old_text, new_text, 1)
    } else {
        body.to_string()
    };

    meta.timestamp = now_iso();
    let new_content = build_doc(&meta, &new_body);

    match tokio::fs::write(&full, &new_content).await {
        Ok(_) => {
            let detail = format!(
                "old_text_len={}, new_text_len={}",
                args["old_text"].as_str().unwrap_or("").len(),
                args["new_text"].as_str().unwrap_or("").len()
            );
            crate::audit::append(working_dir, "modify_document", dept, id, &detail).await;
            crate::audit::save_diff(working_dir, id, "modify_document", body, &new_body).await;
            ToolOutput::success("modify_document", id, "修改成功")
        }
        Err(e) => ToolOutput::error("modify_document", id, "write_error", &e.to_string()),
    }
}

/// Append content to an existing document's body.
pub async fn tool_append_document(
    working_dir: &Path,
    args: &serde_json::Value,
    dept: &str,
) -> String {
    let id = args["id"].as_str().unwrap_or("");
    if id.is_empty() {
        return ToolOutput::error("append_document", "", "empty_id", "文档 ID 不能为空");
    }

    let append_parts: Vec<String> = if let Some(parts) = args["contents"].as_array() {
        if parts.is_empty() {
            return ToolOutput::error(
                "append_document",
                id,
                "empty_contents",
                "contents 数组不能为空",
            );
        }
        if parts.len() > 5 {
            return ToolOutput::error(
                "append_document",
                id,
                "too_many_contents",
                "contents 最多 5 项，请分批调用",
            );
        }
        for (i, part) in parts.iter().enumerate() {
            let text = part.as_str().unwrap_or("");
            if text.is_empty() {
                return ToolOutput::error(
                    "append_document",
                    id,
                    "empty_content_item",
                    &format!("contents[{}] 不能为空", i),
                );
            }
            if text.len() > 2000 {
                return ToolOutput::error(
                    "append_document",
                    id,
                    "content_too_long",
                    &format!(
                        "contents[{}] 过长（{} 字符），最大 2000 字符。",
                        i,
                        text.len()
                    ),
                );
            }
        }
        parts
            .iter()
            .filter_map(|p| p.as_str().map(|s| s.to_string()))
            .collect()
    } else {
        let single = args["content"].as_str().unwrap_or("").to_string();
        if single.is_empty() {
            return ToolOutput::error(
                "append_document",
                id,
                "empty_content",
                "请传入 content（单段）或 contents（批量）参数",
            );
        }
        if single.len() > 2000 {
            return ToolOutput::error(
                "append_document",
                id,
                "content_too_long",
                &format!(
                    "追加内容过长（{} 字符），最大 2000 字符。请分批追加或使用 contents 数组。",
                    single.len()
                ),
            );
        }
        vec![single]
    };

    let type_prefix = id.split('_').next().unwrap_or("");
    let full = if type_prefix == "rprt" {
        match find_rprt_path(working_dir, id).await {
            Some(p) => p,
            None => {
                return ToolOutput::error(
                    "append_document",
                    id,
                    "not_found",
                    &format!("文档 {} 不存在", id),
                )
            }
        }
    } else {
        let dir = type_to_dir(type_prefix);
        let rel_path = if dir.is_empty() {
            format!(".shuji/{}.md", id)
        } else {
            format!(".shuji/{}/{}.md", dir, id)
        };
        match resolve_scoped_path(working_dir, &rel_path).await {
            Ok(p) => p,
            Err(e) => return ToolOutput::error("append_document", id, "path_error", &e),
        }
    };
    if !full.exists() {
        return ToolOutput::error(
            "append_document",
            id,
            "not_found",
            &format!("文档 {} 不存在", id),
        );
    }

    let content = match tokio::fs::read_to_string(&full).await {
        Ok(c) => c,
        Err(e) => return ToolOutput::error("append_document", id, "read_error", &e.to_string()),
    };

    let (mut meta, body) = match parse_doc(&content) {
        Ok(m) => m,
        Err(e) => return ToolOutput::error("append_document", id, "parse_error", &e),
    };

    let new_body = if body.is_empty() {
        append_parts.join("\n")
    } else {
        let mut b = body.to_string();
        for part in &append_parts {
            b.push_str("\n");
            b.push_str(part);
        }
        b
    };

    meta.timestamp = now_iso();
    let new_content = build_doc(&meta, &new_body);

    match tokio::fs::write(&full, &new_content).await {
        Ok(_) => {
            let total: usize = append_parts.iter().map(|s| s.len()).sum();
            let detail = format!("append_parts={}, total_chars={}", append_parts.len(), total);
            crate::audit::append(working_dir, "append_document", dept, id, &detail).await;
            crate::audit::save_diff(working_dir, id, "append_document", body, &new_body).await;
            ToolOutput::success("append_document", id, "追加成功")
        }
        Err(e) => ToolOutput::error("append_document", id, "write_error", &e.to_string()),
    }
}

/// ── Tool definitions ──────────────────────────────────────────────
pub fn create_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "create_document".into(),
            description: "创建新文档，系统自动分配 ID、生成 YAML 头部，返回文档 ID。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "type": {
                        "type": "string",
                        "enum": ["dsgn", "plan", "pdsg", "ddtl", "task", "ctrt", "rprt", "revw", "anls", "reqs"],
                        "description": "dsgn/plan/pdsg/ddtl/revw/task/ctrt/rprt/anls/reqs"
                    },
                    "refs": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "引用的文档ID列表（整数，不带类型前缀）。无引用传 []"
                    }
                },
                "required": ["type", "refs"]
            }),
        },
    }
}

pub fn modify_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "modify_document".into(),
            description: "替换文档正文中的文本 (find+replace)。≤300字符。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "文档 ID，如 dsgn_003"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "待替换文本（≤300字符）",
                        "maxLength": 300
                    },
                    "new_text": {
                        "type": "string",
                        "description": "新文本（≤300字符）",
                        "maxLength": 300
                    }
                },
                "required": ["id", "old_text", "new_text"]
            }),
        },
    }
}

pub fn append_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "append_document".into(),
            description: "追加内容到已有文档的正文末尾。每次追加一段（content ≤2000 字符），多段内容分多次调用。不要用 contents 数组——数组 JSON 在长内容下易截断。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "文档 ID，如 dsgn_003"
                    },
                    "content": {
                        "type": "string",
                        "description": "追加内容（≤2000 字符）。多段请分多次调，不要用 contents 数组。",
                        "maxLength": 2000
                    },
                    "contents": {
                        "type": "array",
                        "description": "【不推荐】数组 JSON 在长内容下易被截断导致错误。请用单段 content 分多次调用。",
                        "items": {
                            "type": "string",
                            "maxLength": 2000
                        },
                        "maxItems": 5
                    }
                },
                "anyOf": [
                    {"required": ["id", "content"]},
                    {"required": ["id", "contents"]}
                ]
            }),
        },
    }
}

/// ── read_document ─────────────────────────────────────────────────
pub async fn tool_read_document(working_dir: &Path, args: &serde_json::Value) -> String {
    let id = args["id"].as_str().unwrap_or("");
    if id.is_empty() {
        return ToolOutput::error("read_document", "", "empty_id", "文档 ID 不能为空");
    }

    let full = match resolve_doc_path(working_dir, id).await {
        Ok(p) => p,
        Err(e) => return ToolOutput::error("read_document", id, "not_found", &e),
    };
    if !full.exists() {
        return ToolOutput::error(
            "read_document",
            id,
            "not_found",
            &format!("文档 {} 不存在", id),
        );
    }

    if let Some(cached) = crate::tool::cache_lookup(working_dir, &full) {
        return cached;
    }

    let content = match tokio::fs::read_to_string(&full).await {
        Ok(c) => {
            if let Ok(meta) = tokio::fs::metadata(&full).await {
                if let Ok(mtime) = meta.modified() {
                    crate::tool::cache_insert(working_dir, full.clone(), mtime, c.clone());
                }
            }
            c
        }
        Err(e) => return ToolOutput::error("read_document", id, "read_error", &e.to_string()),
    };

    let (meta, body) = match parse_doc(&content) {
        Ok(m) => m,
        Err(e) => return ToolOutput::error("read_document", id, "parse_error", &e),
    };

    let target_section = args["section"].as_str().filter(|s| !s.is_empty());
    let extracted = if let Some(section_name) = target_section {
        match extract_section(body, section_name) {
            Ok(content) => content,
            Err(msg) => {
                return ToolOutput::error("read_document", id, "section_not_found", &msg);
            }
        }
    } else {
        body.to_string()
    };

    let max_chars = args["max_chars"].as_u64().unwrap_or(4000) as usize;
    let display_body = if max_chars > 0 && extracted.len() > max_chars {
        let cutoff = extracted.floor_char_boundary(max_chars);
        format!(
            "{}...\n\n[截断：显示前 {} 字符，共 {} 字符]",
            &extracted[..cutoff],
            cutoff,
            extracted.len()
        )
    } else {
        extracted
    };

    let rel_path = full
        .strip_prefix(working_dir)
        .unwrap_or(&full)
        .to_string_lossy();
    let meta_line = format!(
        "📄 {} | 类型: {} | 作者: {} | 时间: {} | 状态: {} | refs: {}",
        meta.id,
        meta.doc_type,
        meta.author,
        meta.timestamp,
        if meta.status.is_empty() {
            "-"
        } else {
            &meta.status
        },
        meta.refs
    );
    let result = if let Some(ref section) = target_section {
        format!(
            "{}\n─── 章节 [{}] ───\n{}",
            meta_line, section, display_body
        )
    } else {
        format!("{}\n─── 正文 ───\n{}", meta_line, display_body)
    };

    ToolOutput::read_file("read_document", &rel_path, &result)
}

/// Extract a section (## or ### heading) from markdown body text.
fn extract_section(body: &str, section_name: &str) -> Result<String, String> {
    let heading = format!("## {}", section_name);
    let heading3 = format!("### {}", section_name);
    let lines: Vec<&str> = body.lines().collect();
    let mut start: Option<usize> = None;
    let mut end: Option<usize> = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed == heading
            || trimmed.starts_with(&heading)
            || trimmed == heading3
            || trimmed.starts_with(&heading3)
        {
            start = Some(i);
        } else if start.is_some()
            && end.is_none()
            && (line.starts_with("## ") || line.starts_with("### "))
        {
            end = Some(i);
            break;
        }
    }

    if let Some(s) = start {
        let e = end.unwrap_or(lines.len());
        Ok(lines[s..e].join("\n"))
    } else {
        let available: Vec<&str> = lines
            .iter()
            .filter(|l| l.starts_with("## ") || l.starts_with("### "))
            .map(|l| l.trim())
            .collect();
        Err(format!(
            "文档中未找到章节「{}」。\n可用章节（共 {} 个）：\n{}",
            section_name,
            available.len(),
            available.join("\n")
        ))
    }
}

pub fn read_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "read_document".into(),
            description: "首选读文档方式。按文档ID读取，返回 YAML 元信息 + 正文。默认截断 4000 字符（传 max_chars=0 禁用）。可选按 ## 章节提取。替代 find_document → read_file 两步。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "文档 ID，如 dsgn_003, rprt_32, task_5"
                    },
                    "section": {
                        "type": "string",
                        "description": "可选：按 ## 标题提取特定章节（如「签名」「数据操作」），不传则返回全文"
                    },
                    "max_chars": {
                        "type": "integer",
                        "description": "可选：最大返回字符数，超出截断（以防超长文档撑爆上下文）"
                    }
                },
                "required": ["id"]
            }),
        },
    }
}

/// ── find_document ─────────────────────────────────────────────────
pub async fn tool_find_document(working_dir: &Path, args: &serde_json::Value) -> String {
    let id = args["id"].as_str().unwrap_or("");
    if id.is_empty() {
        return ToolOutput::error("find_document", "", "empty_id", "文档 ID 不能为空");
    }

    let type_prefix = id.split('_').next().unwrap_or("");

    if type_prefix == "rprt" {
        match find_rprt_path(working_dir, id).await {
            Some(p) => {
                let rel = p.strip_prefix(working_dir).unwrap_or(&p);
                ToolOutput::success("find_document", id, &format!("{}", rel.display()))
            }
            None => ToolOutput::error(
                "find_document",
                id,
                "not_found",
                &format!("文档 {} 不存在", id),
            ),
        }
    } else {
        let dir = type_to_dir(type_prefix);
        let rel_path = if dir.is_empty() {
            format!(".shuji/{}.md", id)
        } else {
            format!(".shuji/{}/{}.md", dir, id)
        };
        match resolve_scoped_path(working_dir, &rel_path).await {
            Ok(full) if full.exists() => {
                let rel = full.strip_prefix(working_dir).unwrap_or(&full);
                ToolOutput::success("find_document", id, &format!("{}", rel.display()))
            }
            Ok(_) => ToolOutput::error(
                "find_document",
                id,
                "not_found",
                &format!("文档 {} 不存在", id),
            ),
            Err(e) => ToolOutput::error("find_document", id, "path_error", &e),
        }
    }
}

pub fn find_document_tool_def() -> crate::api::client::ToolDefinition {
    crate::api::client::ToolDefinition {
        tool_type: "function".into(),
        function: crate::api::client::ToolFunction {
            name: "find_document".into(),
            description: "⚠️ 降级建议：除非 read_document 失败，否则勿用。read_document 已合并查找+读取+章节提取，一次调用即可。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "文档 ID，如 rprt_32, dsgn_003, task_5"
                    }
                },
                "required": ["id"]
            }),
        },
    }
}
