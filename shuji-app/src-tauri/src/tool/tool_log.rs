use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

static LOG_LOCK: Mutex<()> = Mutex::new(());

/// Log a tool call to `.shuji/logs/tool-calls/{dept}.jsonl`.
/// Each line: `{"ts":"...","tool":"...","args":{...}}`
pub fn log_tool_call(dept: &str, tool_name: &str, args: &serde_json::Value, working_dir: &Path) {
    let log_dir = working_dir.join(".shuji").join("logs").join("tool-calls");
    let _ = std::fs::create_dir_all(&log_dir);

    let log_path = log_dir.join(format!("{}.jsonl", dept));

    let entry = serde_json::json!({
        "ts": chrono::Local::now().to_rfc3339(),
        "tool": tool_name,
        "args": args,
    });

    let _lock = LOG_LOCK.lock();
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let _ = file.write_all(entry.to_string().as_bytes());
        let _ = file.write_all(b"\n");
    }
}
