use serde::Deserialize;
use tauri::State;

use crate::commands::friendly_error::friendly_error;
use crate::commands::project::AppState;
use crate::models::chat::ChatMessage;
use crate::models::project::Project;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ScenarioStep {
    agent: String,
    response: String,
    #[serde(default)]
    route: Option<String>,
    #[serde(default)]
    actions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Scenario {
    name: String,
    description: String,
    steps: Vec<ScenarioStep>,
}

/// Demo project files embedded at compile time.
const CALC_PY: &str = include_str!("../../assets/demo-project/calc.py");
const TEST_CALC_PY: &str = include_str!("../../assets/demo-project/test_calc.py");

/// Generate a demo project in a fresh `.quickstart` directory.
/// Contains a Python calculator with known bugs + tests that fail.
/// New users can immediately try the workflow: fix bugs, run tests.
///
/// The directory is always freshly created — any existing `.quickstart`
/// is removed first, guaranteeing a clean initial state for each run.
#[tauri::command]
pub async fn create_demo_project(state: State<'_, AppState>) -> Result<Project, String> {
    let base = std::env::temp_dir().join(".quickstart");

    // Always start fresh — wipe any leftover state from a previous run
    if base.exists() {
        tokio::fs::remove_dir_all(&base)
            .await
            .map_err(friendly_error)?;
    }

    tokio::fs::create_dir_all(&base)
        .await
        .map_err(friendly_error)?;

    tokio::fs::write(base.join("calc.py"), CALC_PY)
        .await
        .map_err(friendly_error)?;
    tokio::fs::write(base.join("test_calc.py"), TEST_CALC_PY)
        .await
        .map_err(friendly_error)?;

    let name = "calc_demo".to_string();
    let goal =
        "修复 calculator 模块中的 bug（power 和 factorial 函数），确保所有测试通过。".to_string();
    let wd_str = base.to_string_lossy().to_string();

    crate::commands::project::create_project(state, name, goal, wd_str).await
}

/// Reset the demo project: wipe and re-create. Same as create_demo_project
/// but does not require a fresh session. Returns the new project.
#[tauri::command]
pub async fn reset_demo_project(state: State<'_, AppState>) -> Result<Project, String> {
    create_demo_project(state).await
}

/// Run a mock workflow without calling LLM API.
/// Returns pre-recorded ChatMessage responses from the chosen scenario.
#[tauri::command]
#[allow(dead_code)]
pub async fn run_mock_workflow(
    _state: State<'_, AppState>,
    project_dir: String,
    scenario: String,
) -> Result<Vec<ChatMessage>, String> {
    // Load scenario file
    let scenario: Scenario = match scenario.as_str() {
        "todo-cli" => {
            let bytes = include_bytes!("scenarios/todo-cli.json");
            serde_json::from_slice(bytes).map_err(|e| format!("解析场景失败: {}", e))?
        }
        "markdown-parser" => {
            let bytes = include_bytes!("scenarios/markdown-parser.json");
            serde_json::from_slice(bytes).map_err(|e| format!("解析场景失败: {}", e))?
        }
        "simple-api" => {
            let bytes = include_bytes!("scenarios/simple-api.json");
            serde_json::from_slice(bytes).map_err(|e| format!("解析场景失败: {}", e))?
        }
        _ => return Err(format!("未知场景: {}", scenario)),
    };

    // Create demo project directory (idempotent)
    let base = std::path::Path::new(&project_dir);
    if !base.exists() {
        tokio::fs::create_dir_all(base)
            .await
            .map_err(friendly_error)?;
    }

    let mut messages = Vec::new();

    // Intro message from 皇帝
    messages.push(ChatMessage::new(
        "user",
        &format!("运行演示场景: {}", scenario.name),
    ));

    // Generate mock messages for each step
    for (i, step) in scenario.steps.iter().enumerate() {
        let content = if i == scenario.steps.len() - 1 {
            format!("✅ 演示完成!\n\n{}", step.response)
        } else if let Some(route) = &step.route {
            format!("{} [路由至 {}]", step.response, route)
        } else {
            step.response.clone()
        };

        let mut msg = ChatMessage::new(&step.agent, &content);

        // Add continue option for non-last steps
        if i < scenario.steps.len() - 1 {
            msg.options.push(crate::models::chat::ChatOption {
                key: "continue_mock".to_string(),
                label: "继续".to_string(),
                description: format!("下一步: {}", scenario.steps[i + 1].agent),
            });
        }

        messages.push(msg);
    }

    // Summary message
    messages.push(ChatMessage::new(
        "system",
        &format!(
            "演示场景「{}」已完成，共 {} 步。您可以在此项目上开始真实工作流。",
            scenario.name,
            scenario.steps.len()
        ),
    ));

    Ok(messages)
}
