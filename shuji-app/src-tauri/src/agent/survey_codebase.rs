//! survey_codebase sub-agent.
//!
//! Scans a project's codebase (read-only) and produces a structured
//! analysis document at `.shuji/analysis/`. Used by brownfield workflows
//! to understand existing code before making changes.

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::api::client::AnthropicClient;
use crate::models::message::Message;

const PROMPT: &str = include_str!("survey_codebase_prompt.md");

/// Run the codebase survey sub-agent.
/// `cancel` is the caller's cancel flag — when set, the sub-agent stops promptly.
/// Returns the analysis document ID on success, or an error description.
pub async fn run(
    task_description: &str,
    working_dir: &Path,
    client: &Arc<AnthropicClient>,
    model: &str,
    cancel: &AtomicBool,
) -> Result<String, String> {
    let tools = {
        let mut t = crate::tool::registry::code_inspect_tools();
        t.push(crate::tool::list_dir_tool_def());
        // Documents for analysis reports
        t.push(crate::tool::documents::create_document_tool_def());
        t.push(crate::tool::documents::append_document_tool_def());
        // File write for project_profile.md (only tool allowed to write)
        t.push(crate::tool::create_file_tool_def(
            "创建/更新 project_profile.md",
        ));
        t
    };

    let prompt = format!(
        "对以下项目进行代码库勘察：\n\n项目目录: {}\n\n任务背景: {}",
        working_dir.display(),
        task_description
    );
    let msgs = vec![Message::user(&prompt)];

    let config = Arc::new(crate::config::RuntimeConfig::default());

    let mut session =
        crate::api::session::Session::new(PROMPT, &msgs, model, &tools, client, &config)
            .with_role("survey_agent")
            .with_debug_dir(working_dir.to_path_buf());

    let wd = working_dir.to_path_buf();
    let exec = move |name: &str, args: &serde_json::Value| -> crate::api::control::ToolFuture {
        let name = name.to_owned();
        let args = args.clone();
        let wd = wd.clone();
        Box::pin(
            async move { crate::tool::execute_named_tool(&name, &wd, &args, "survey_agent").await },
        )
    };

    let mut controller = crate::api::control::AgentController::new();

    let run_result = controller
        .run(&mut session, &exec, cancel, &tools, None, &config, None)
        .await
        .map_err(|e| format!("勘察失败: {}", e))?;
    let result = run_result.into_text();

    Ok(result.trim().to_string())
}
