use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

use crate::models::role::Role;

pub struct Logger {
    logs_dir: PathBuf,
    counters: Mutex<HashMap<String, u64>>,
}

impl Logger {
    pub fn new(shuji_root: &PathBuf) -> Self {
        let mut counters = HashMap::new();
        for prefix in &["序", "内阁", "中书省", "门下省", "尚书省", "制司", "皇帝",
                        "吏部", "户部", "礼部", "兵部", "刑部", "工部"] {
            counters.insert(prefix.to_string(), 1u64);
        }
        Self {
            logs_dir: shuji_root.join("logs"),
            counters: Mutex::new(counters),
        }
    }

    /// Log an event from a specific source with department-prefixed ID.
    pub async fn log(&self, source: &str, source_prefix: &str, event_type: &str, summary: &str, details: &str) {
        let id = {
            let mut map = self.counters.lock().unwrap();
            if !map.contains_key(source_prefix) {
                map.insert(source_prefix.to_string(), 1);
            }
            let counter = map.get_mut(source_prefix).unwrap();
            let current = *counter;
            *counter += 1;
            current
        };

        let entry = serde_json::json!({
            "id": format!("{}-{:04}", source_prefix, id),
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "source": source,
            "type": event_type,
            "summary": summary,
            "details": details,
        });

        let filename = format!("{}.jsonl", source_prefix);
        let log_path = self.logs_dir.join(&filename);

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .await
        {
            let _ = file.write_all(entry.to_string().as_bytes()).await;
            let _ = file.write_all(b"\n").await;
        }
    }

    /// Log a state transition (by 编排器).
    pub async fn log_transition(&self, summary: &str, details: &str) {
        self.log("编排器", "序", "状态转移", summary, details).await;
    }

    /// Log an agent execution.
    pub async fn log_agent(&self, role: Role, action: &str, summary: &str, details: &str) {
        let (source, prefix) = role_log_info(role);
        self.log(source, prefix, action, summary, details).await;
    }
}

fn role_log_info(role: Role) -> (&'static str, &'static str) {
    match role {
        Role::Zhongshu => ("中书省", "中书省"),
        Role::Menxia => ("门下省", "门下省"),
        Role::Neige => ("内阁", "内阁"),
        Role::Shangshu => ("尚书省", "尚书省"),
        Role::LiBuP => ("吏部", "吏部"),
        Role::Hubu => ("户部", "户部"),
        Role::LiBuR => ("礼部", "礼部"),
        Role::Bingbu => ("兵部", "兵部"),
        Role::Xingbu => ("刑部", "刑部"),
        Role::Gongbu => ("工部", "工部"),
        Role::Zhisi => ("制司", "制司"),
    }
}
