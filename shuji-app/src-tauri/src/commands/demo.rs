use tauri::State;

use crate::commands::friendly_error::friendly_error;
use crate::commands::project::AppState;
use crate::models::project::Project;

/// Demo project files embedded at compile time.
const CALC_PY: &str = include_str!("../../assets/demo-project/calc.py");
const TEST_CALC_PY: &str = include_str!("../../assets/demo-project/test_calc.py");

/// Generate a demo project in a temporary directory.
/// Contains a Python calculator with known bugs + tests that fail.
/// New users can immediately try the workflow: fix bugs, run tests.
#[tauri::command]
pub async fn create_demo_project(
    state: State<'_, AppState>,
) -> Result<Project, String> {
    let base = std::env::temp_dir().join("shuji-demo");
    let working_dir = base.join("calc_demo");
    tokio::fs::create_dir_all(&working_dir)
        .await
        .map_err(friendly_error)?;

    tokio::fs::write(working_dir.join("calc.py"), CALC_PY)
        .await
        .map_err(friendly_error)?;
    tokio::fs::write(working_dir.join("test_calc.py"), TEST_CALC_PY)
        .await
        .map_err(friendly_error)?;

    let name = "calc_demo".to_string();
    let goal = "修复 calculator 模块中的 bug（power 和 factorial 函数），确保所有测试通过。".to_string();
    let wd_str = working_dir.to_string_lossy().to_string();

    crate::commands::project::create_project(state, name, goal, wd_str).await
}
