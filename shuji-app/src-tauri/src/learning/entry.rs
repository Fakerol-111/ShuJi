use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningScope {
    Project,
    GlobalCandidate,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearningKind {
    Experience,
    Lesson,
    Preference,
    ProjectFact,
    Command,
    ReviewRule,
}

impl LearningKind {
    pub fn markdown_heading(&self) -> &'static str {
        match self {
            Self::Experience => "## Experience",
            Self::Lesson => "## Lessons",
            Self::Preference => "## Preferences",
            Self::ProjectFact => "## Project Facts",
            Self::Command => "## Commands",
            Self::ReviewRule => "## Review Rules",
        }
    }

    pub fn from_section_or_kind(section: Option<&str>, kind: Option<&str>) -> Option<Self> {
        if let Some(k) = kind {
            return Self::parse(k);
        }
        section.and_then(Self::from_section)
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "experience" | "经验" => Some(Self::Experience),
            "lesson" | "lessons" | "教训" => Some(Self::Lesson),
            "preference" | "preferences" | "偏好" | "emperor preferences" => {
                Some(Self::Preference)
            }
            "project_fact" | "project facts" | "项目事实" => Some(Self::ProjectFact),
            "command" | "commands" | "命令" => Some(Self::Command),
            "review_rule" | "review rules" | "审查规则" => Some(Self::ReviewRule),
            _ => None,
        }
    }

    fn from_section(section: &str) -> Option<Self> {
        Self::parse(section)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningEntry {
    pub id: String,
    pub role: String,
    pub scope: LearningScope,
    pub kind: LearningKind,
    pub content: String,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    pub created_at: String,
    pub last_seen: String,
}

fn default_confidence() -> f32 {
    0.7
}

impl LearningEntry {
    pub fn new(
        role: &str,
        scope: LearningScope,
        kind: LearningKind,
        content: &str,
        evidence: Vec<String>,
        tags: Vec<String>,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        let raw = uuid::Uuid::new_v4().to_string();
        let id = format!("le_{}_{}", role.to_lowercase(), &raw[..8]);
        Self {
            id,
            role: role.to_string(),
            scope,
            kind,
            content: content.to_string(),
            evidence,
            tags,
            confidence: 0.7,
            created_at: now.clone(),
            last_seen: now,
        }
    }

    pub fn markdown_line(&self) -> String {
        format!("- {}\n", self.content)
    }
}
