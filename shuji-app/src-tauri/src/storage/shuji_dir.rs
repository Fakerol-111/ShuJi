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

    /// Initialize .shuji directory structure for a new project.
    pub async fn init(&self) -> anyhow::Result<()> {
        let dirs = [
            self.root.join("context"),
            self.root.join("designs"),
            self.root.join("designs/detail"),
            self.root.join("reviews"),
            self.root.join("tasks"),
            self.root.join("contracts"),
            self.root.join("reports"),
            self.root.join("logs"),
            self.root.join("execution"),
            self.root.join("skills"),
            self.root.join("soul"),
            self.root.join("audit"),
            self.root.join("audit/diffs"),
        ];
        for dir in &dirs {
            fs::create_dir_all(dir).await?;
        }
        // Init document ID counter if not present
        let counter_path = self.root.join("_counter");
        if !counter_path.exists() {
            fs::write(&counter_path, "1").await?;
        }
        // Copy default zuxun if not already present — user can edit it later
        self.init_zuxun().await?;
        // Init isolated git repo for checkpoint system
        self.init_git_repo().await?;
        Ok(())
    }

    /// Initialize an isolated git repository at `.shuji/.git/` for checkpoint use.
    /// This is completely separate from any user git repo at the project root.
    async fn init_git_repo(&self) -> anyhow::Result<()> {
        let git_dir = self.root.join(".git");
        if fs::try_exists(&git_dir).await? {
            return Ok(()); // already initialized
        }
        let git_dir_str = git_dir.to_string_lossy().to_string();
        let root = self
            .root
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        // git --git-dir=.shuji/.git --work-tree=. init
        // Creates an isolated repo at .shuji/.git/ with worktree = project root
        let init = tokio::process::Command::new("git")
            .args(["--git-dir", &git_dir_str, "--work-tree", &root, "init"])
            .output()
            .await?;
        if !init.status.success() {
            anyhow::bail!("git init failed: {}", String::from_utf8_lossy(&init.stderr));
        }

        // Set local user config so commits work without global git config
        let set_name = tokio::process::Command::new("git")
            .args(["--git-dir", &git_dir_str, "config", "user.name", "ShuJi"])
            .output()
            .await?;
        let set_email = tokio::process::Command::new("git")
            .args([
                "--git-dir",
                &git_dir_str,
                "config",
                "user.email",
                "shuji@local",
            ])
            .output()
            .await?;
        if !set_name.status.success() || !set_email.status.success() {
            anyhow::bail!("setting git user config failed");
        }

        // Initial commit so HEAD exists (required by git diff-index --cached --quiet HEAD)
        let initial = tokio::process::Command::new("git")
            .args([
                "--git-dir",
                &git_dir_str,
                "--work-tree",
                &root,
                "commit",
                "--allow-empty",
                "-m",
                "shuji: init",
            ])
            .output()
            .await?;
        if !initial.status.success() {
            anyhow::bail!(
                "git initial commit failed: {}",
                String::from_utf8_lossy(&initial.stderr)
            );
        }

        Ok(())
    }

    /// Ensure a zuxun.md exists in .shuji/ — copy from bundled default if needed.
    async fn init_zuxun(&self) -> anyhow::Result<()> {
        let path = self.root.join("zuxun.md");
        if fs::try_exists(&path).await? {
            return Ok(()); // already exists, user may have customised it
        }
        let default = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../assets/defaults/zuxun.md"
        ));
        fs::write(&path, default).await?;
        Ok(())
    }

    pub async fn load_project(&self) -> anyhow::Result<Option<Project>> {
        let path = self.root.join("state.json");
        if !fs::try_exists(&path).await? {
            return Ok(None);
        }
        let data = fs::read_to_string(&path).await?;
        let mut project: Project = serde_json::from_str(&data)?;

        // Reconstruct missing context from file system (for old projects
        // that predate summary/talk/task fields)
        if project.goal.is_empty() || project.summary.is_empty() {
            self.reconstruct_state(&mut project).await?;
        }

        Ok(Some(project))
    }

    /// Reconstruct project context from file system when loading an old project
    /// that doesn't have summary/talk/task fields populated.
    async fn reconstruct_state(&self, project: &mut Project) -> anyhow::Result<()> {
        let designs = self.list_documents("designs").await.unwrap_or_default();
        let reviews = self.list_documents("reviews").await.unwrap_or_default();

        // 1. Reconstruct goal from overall design document's first heading
        if project.goal.is_empty() {
            if let Ok(Some(content)) = self.read_document("designs", "overall_design.md").await {
                if let Some(line) = content.lines().find(|l| l.starts_with("# ")) {
                    project.goal = line.trim_start_matches("# ").trim().to_string();
                }
            }
        }

        // 2. Reconstruct summary from existing files
        let has_overall = designs.iter().any(|d| d.contains("overall_design"));
        let has_phase1 = designs.iter().any(|d| d.contains("phase_1_design"));
        let has_approved_review = reviews
            .iter()
            .any(|r| r.contains("_agree") || r.contains("_pass"));
        let has_rejected_review = reviews.iter().any(|r| r.contains("_reject"));

        if has_overall && has_phase1 && has_approved_review {
            project.summary =
                "Overall plan approved, Phase 1 design complete, awaiting execution".into();
        } else if has_overall && has_approved_review {
            project.summary = "Overall plan reviewed and passed, pending emperor approval".into();
        } else if has_overall && has_rejected_review {
            project.summary = "Overall plan review found issues, returning for revision".into();
        } else if has_overall {
            project.summary = "Overall plan being designed".into();
        } else {
            project.summary = "Project opened, not yet started".into();
        }

        // 3. Reconstruct task milestones from review files
        for review in &reviews {
            let milestone = if review.contains("overall_design") {
                if review.contains("_agree") {
                    "[Restored] Overall plan review passed".to_string()
                } else if review.contains("_reject") {
                    "[Restored] Overall plan review found issues".to_string()
                } else {
                    format!("[Restored] Review report: {}", review)
                }
            } else if review.contains("phase_1") {
                if review.contains("_agree") {
                    "[Restored] Phase 1 review passed".to_string()
                } else {
                    format!("[Restored] Phase 1 review: {}", review)
                }
            } else {
                continue;
            };
            if project.task.is_empty() {
                project.task = milestone;
            } else {
                project.task.push_str(&format!("\n{}", milestone));
            }
        }

        // 4. Set resume context for execution state
        if !project.resume.is_empty() {
            // Keep existing resume (was saved before close)
        } else if has_phase1 && has_approved_review {
            project.resume = "Phase design complete, awaiting or in execution".into();
        }

        // 5. Add a talk entry noting this is a restored session
        project.append_talk("System > Project restored, following entries rebuilt from filesystem");

        Ok(())
    }

    pub async fn save_project(&self, project: &Project) -> anyhow::Result<()> {
        let path = self.root.join("state.json");
        let data = serde_json::to_string_pretty(project)?;
        fs::write(&path, &data).await?;
        Ok(())
    }

    pub async fn read_document(
        &self,
        subdir: &str,
        filename: &str,
    ) -> anyhow::Result<Option<String>> {
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
        Ok(data
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect())
    }
}
