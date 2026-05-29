use std::path::Path;
use std::sync::Mutex;

static LOG_LOCK: Mutex<()> = Mutex::new(());

/// Log a tool call to `.shuji/logs/tool-calls/{dept}.jsonl`.
/// Each line: `{"ts":"...","tool":"...","args":{...}}`
pub async fn log_tool_call(dept: &str, tool_name: &str, args: &serde_json::Value, working_dir: &Path) {
    let log_dir = working_dir.join(".shuji").join("logs").join("tool-calls");
    let _ = tokio::fs::create_dir_all(&log_dir).await;

    let log_path = log_dir.join(format!("{}.jsonl", dept));

    let entry = serde_json::json!({
        "ts": chrono::Local::now().to_rfc3339(),
        "tool": tool_name,
        "args": args,
    });

    let entry = format!("{}\n", entry);

    let _ = tokio::task::spawn_blocking(move || {
        let _lock = LOG_LOCK.lock().ok()?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .ok()?;
        use std::io::Write;
        file.write_all(entry.as_bytes()).ok()
    }).await;
}
