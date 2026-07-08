use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub id: String,
    pub description: String,
    pub category: String,
    pub status: String, // pending | pass | fail | na
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checklist {
    pub items: Vec<ChecklistItem>,
}

/// Load the audit checklist from `.shuji/audit/checklist.json`.
pub async fn load_checklist(working_dir: &Path) -> Checklist {
    let path = working_dir
        .join(".shuji")
        .join("audit")
        .join("checklist.json");
    tokio::fs::read_to_string(&path)
        .await
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or(Checklist { items: vec![] })
}

/// Save the audit checklist to `.shuji/audit/checklist.json`.
pub async fn save_checklist(working_dir: &Path, checklist: &Checklist) {
    let path = working_dir
        .join(".shuji")
        .join("audit")
        .join("checklist.json");
    let _ = match path.parent() {
        Some(parent) => tokio::fs::create_dir_all(parent).await,
        None => return,
    };
    if let Ok(json) = serde_json::to_string_pretty(checklist) {
        let _ = tokio::fs::write(&path, &json).await;
    }
}

/// Initialize a checklist with standard items for a given audit category.
pub async fn init_checklist(working_dir: &Path, category: &str) -> String {
    let items = match category {
        "spec" => vec![
            ChecklistItem {
                id: "spec-001".into(),
                description: "All public functions have doc comments".into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            },
            ChecklistItem {
                id: "spec-002".into(),
                description: "Naming follows Rust conventions (snake_case / CamelCase)".into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            },
            ChecklistItem {
                id: "spec-003".into(),
                description: "No unused imports or variables".into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            },
            ChecklistItem {
                id: "spec-004".into(),
                description: "Error handling complete (no unwrap/expect abuse)".into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            },
        ],
        "test" => vec![
            ChecklistItem {
                id: "test-001".into(),
                description: "All public functions have corresponding tests".into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            },
            ChecklistItem {
                id: "test-002".into(),
                description: "Tests cover edge cases".into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            },
            ChecklistItem {
                id: "test-003".into(),
                description: "Tests can run independently (no shared mutable state)".into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            },
        ],
        _ => {
            // General audit - include role boundary checks
            let mut items = vec![ChecklistItem {
                id: "gen-001".into(),
                description: format!("Audit category: {}", category),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            }];
            // Role boundary enforcement checks (always included)
            items.push(ChecklistItem {
                id: "ROLE_BOUNDARY_001".into(),
                description: "Test departments (兵部/刑部) must not deliver production implementation in reports".into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            });
            items.push(ChecklistItem {
                id: "ROLE_BOUNDARY_002".into(),
                description: "Validation departments (刑部) must not modify src/ production code"
                    .into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            });
            items.push(ChecklistItem {
                id: "ROLE_BOUNDARY_003".into(),
                description: "Implementation departments (工部) must not modify approved test contracts (ctrt)".into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            });
            items.push(ChecklistItem {
                id: "RUST_UNSAFE_001".into(),
                description: "Unsafe blocks must have invariant documentation and audit trail"
                    .into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            });
            items.push(ChecklistItem {
                id: "TEST_EVIDENCE_001".into(),
                description: "Delivery report must contain the last test command and result".into(),
                category: category.into(),
                status: "pending".into(),
                note: String::new(),
            });
            items
        }
    };
    let count = items.len();
    let checklist = Checklist { items };
    save_checklist(working_dir, &checklist).await;
    format!("Created {} checklist items", count)
}

/// Update a single checklist item's status and note.
pub async fn update_checklist_item(
    working_dir: &Path,
    id: &str,
    status: &str,
    note: &str,
) -> Result<String, String> {
    let mut checklist = load_checklist(working_dir).await;
    if let Some(item) = checklist.items.iter_mut().find(|i| i.id == id) {
        item.status = status.to_string();
        if !note.is_empty() {
            item.note = note.to_string();
        }
        save_checklist(working_dir, &checklist).await;
        Ok(format!("Checklist item {} marked as {}", id, status))
    } else {
        Err(format!("Checklist item {} not found", id))
    }
}
