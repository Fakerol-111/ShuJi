use std::path::Path;

use crate::tool::editor::{self, EditorConfig};

#[tauri::command]
pub async fn get_editor_config() -> Result<EditorConfig, String> {
    Ok(editor::load_editor_config())
}

#[tauri::command]
pub async fn set_editor_config(config: EditorConfig) -> Result<(), String> {
    editor::save_editor_config(&config)
}

#[tauri::command]
pub async fn check_external_editor(config: EditorConfig) -> Result<String, String> {
    editor::check_editor_available(&config)
}

#[tauri::command]
pub async fn open_in_external_editor(
    project_dir: String,
    rel_path: String,
    line: Option<u32>,
) -> Result<(), String> {
    editor::open_file_in_editor(Path::new(&project_dir), &rel_path, line).await
}

#[tauri::command]
pub async fn open_project_in_external_editor(project_dir: String) -> Result<(), String> {
    editor::open_project_in_editor(Path::new(&project_dir)).await
}
