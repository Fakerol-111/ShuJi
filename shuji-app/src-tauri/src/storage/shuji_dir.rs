use std::path::PathBuf;
use tokio::fs;

use crate::models::project::Project;

pub struct ShujiDir {
    root: PathBuf,
}

impl ShujiDir {
    pub fn new(working_dir: &str) -> Self {
        Self {
            root: PathBuf::from(working_dir).join(".shuji"),
        }
    }

    pub fn root(&self) -> PathBuf {
        self.root.clone()
    }

    /// Initialize .shuji directory structure for a new project.
    pub async fn init(&self) -> anyhow::Result<()> {
        let dirs = [
            self.root.join("contexts"),
            self.root.join("designs"),
            self.root.join("reviews"),
            self.root.join("reports"),
            self.root.join("logs"),
            self.root.join("execution"),
        ];
        for dir in &dirs {
            fs::create_dir_all(dir).await?;
        }
        Ok(())
    }

    pub async fn project_exists(&self) -> bool {
        fs::try_exists(self.root.join("state.json")).await.unwrap_or(false)
    }

    pub async fn load_project(&self) -> anyhow::Result<Option<Project>> {
        let path = self.root.join("state.json");
        if !fs::try_exists(&path).await? {
            return Ok(None);
        }
        let data = fs::read_to_string(&path).await?;
        let project: Project = serde_json::from_str(&data)?;
        Ok(Some(project))
    }

    pub async fn save_project(&self, project: &Project) -> anyhow::Result<()> {
        let path = self.root.join("state.json");
        let data = serde_json::to_string_pretty(project)?;
        fs::write(&path, &data).await?;
        Ok(())
    }

    pub async fn write_document(&self, subdir: &str, filename: &str, content: &str) -> anyhow::Result<String> {
        let dir = self.root.join(subdir);
        fs::create_dir_all(&dir).await?;
        let path = dir.join(filename);
        fs::write(&path, content).await?;
        Ok(path.to_string_lossy().to_string())
    }

    pub async fn read_document(&self, subdir: &str, filename: &str) -> anyhow::Result<Option<String>> {
        let path = self.root.join(subdir).join(filename);
        if !fs::try_exists(&path).await? {
            return Ok(None);
        }
        let content = fs::read_to_string(&path).await?;
        Ok(Some(content))
    }

    pub async fn list_documents(&self, subdir: &str) -> anyhow::Result<Vec<String>> {
        let dir = self.root.join(subdir);
        if !fs::try_exists(&dir).await? {
            return Ok(vec![]);
        }
        let mut entries = fs::read_dir(&dir).await?;
        let mut files = vec![];
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                files.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        files.sort();
        Ok(files)
    }

    pub async fn list_log_files(&self) -> anyhow::Result<Vec<String>> {
        let dir = self.root.join("logs");
        if !fs::try_exists(&dir).await? {
            return Ok(vec![]);
        }
        let mut entries = fs::read_dir(&dir).await?;
        let mut files = vec![];
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                files.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        files.sort();
        Ok(files)
    }

    pub async fn read_log_file(&self, filename: &str) -> anyhow::Result<Vec<String>> {
        let path = self.root.join("logs").join(filename);
        if !fs::try_exists(&path).await? {
            return Ok(vec![]);
        }
        let data = fs::read_to_string(&path).await?;
        Ok(data.lines().filter(|l| !l.is_empty()).map(|l| l.to_string()).collect())
    }
}
