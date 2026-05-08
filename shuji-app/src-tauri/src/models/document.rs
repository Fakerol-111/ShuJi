use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub title: String,
    pub content: String,
    pub doc_type: DocumentType,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentType {
    Design,
    Review,
    Memorial,
    Edict,
    Dispatch,
    Log,
    TaskBreakdown,
}

impl DocumentType {
    pub fn as_str(&self) -> &str {
        match self {
            DocumentType::Design => "设计文档",
            DocumentType::Review => "审查报告",
            DocumentType::Memorial => "奏折",
            DocumentType::Edict => "敕令",
            DocumentType::Dispatch => "移文",
            DocumentType::Log => "日志",
            DocumentType::TaskBreakdown => "任务清单",
        }
    }
}
