use std::path::Path;

use tauri::State;

use crate::commands::project::AppState;
use crate::pricing::PricingConfig;

#[tauri::command]
pub async fn get_pricing(state: State<'_, AppState>) -> Result<PricingConfig, String> {
    let dir = match state.current_dir.lock().await.as_ref() {
        Some(d) => d.clone(),
        None => return Err("没有加载项目".to_string()),
    };
    let config = crate::pricing::load_or_init(Path::new(&dir));
    Ok(config)
}

#[tauri::command]
pub async fn save_pricing(state: State<'_, AppState>, config: PricingConfig) -> Result<(), String> {
    let dir = match state.current_dir.lock().await.as_ref() {
        Some(d) => d.clone(),
        None => return Err("没有加载项目".to_string()),
    };
    crate::pricing::save_to_file(Path::new(&dir), &config)?;
    crate::pricing::invalidate_cache();
    Ok(())
}

#[tauri::command]
pub async fn refresh_pricing(state: State<'_, AppState>) -> Result<PricingConfig, String> {
    let dir = match state.current_dir.lock().await.as_ref() {
        Some(d) => d.clone(),
        None => return Err("没有加载项目".to_string()),
    };
    let config = crate::pricing::refresh_deepseek(Path::new(&dir)).await?;
    // Recalculate all existing token records with the new prices
    crate::token_tracker::recalculate_all(Path::new(&dir))
        .map_err(|e| format!("重算费用失败: {}", e))?;
    Ok(config)
}
