//! ???? sub-agent????? `expand_requirements` ?????
//! ???????????????????????????? ID?
//! ??????????????

use std::path::Path;
use std::sync::Arc;

use crate::api::client::AnthropicClient;
use crate::models::message::Message;

const PROMPT: &str = include_str!("expand_requirements_prompt.md");

/// Run the requirements expansion sub-agent.
/// `task_id` is the ID of a task document containing the emperor's request.
/// Returns the document ID on success, or an error description.
pub async fn run(
    task_id: &str,
    working_dir: &Path,
    client: &Arc<AnthropicClient>,
    model: &str,
) -> Result<String, String> {
    let tools = {
        let mut t = crate::tool::registry::inspect_tools();
        t.extend(crate::tool::registry::document_tools());
        t
    };

    let prompt = format!("??????????? task ?????? {} ?????", task_id);
    let msgs = vec![Message::user(&prompt)];

    let config = Arc::new(crate::config::RuntimeConfig::default());

    let mut session =
        crate::api::session::Session::new(PROMPT, &msgs, model, &tools, client, &[], &config)
            .with_role("requirements_agent")
            .with_debug_dir(working_dir.to_path_buf());

    let wd = working_dir.to_path_buf();
    let exec = move |name: &str, args: &serde_json::Value| -> crate::api::control::ToolFuture {
        let name = name.to_owned();
        let args = args.clone();
        let wd = wd.clone();
        Box::pin(async move {
            crate::tool::execute_named_tool(&name, &wd, &args, "requirements_agent").await
        })
    };

    let mut controller = crate::api::control::AgentController::new();
    let cancel = std::sync::atomic::AtomicBool::new(false);

    let run_result = controller
        .run(&mut session, &exec, &cancel, &tools, None, &config)
        .await
        .map_err(|e| format!("??????: {}", e))?;
    let result = run_result.into_text();

    Ok(result.trim().to_string())
}
