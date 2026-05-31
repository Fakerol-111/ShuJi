use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::commands::friendly_error::friendly_error;

#[derive(Debug, Clone, Serialize)]
pub struct ShujiEntry {
    pub name: String,
    pub path: String,
    pub type_label: String,
    pub is_dir: bool,
    pub children: Vec<ShujiEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShujiDoc {
    pub content: String,
    pub path: String,
}

#[tauri::command]
pub async fn list_shuji_tree(project_dir: String) -> Result<Vec<ShujiEntry>, String> {
    let project = PathBuf::from(project_dir);
    tokio::task::spawn_blocking(move || build_project_tree(&project))
        .await
        .map_err(friendly_error)?
}

#[tauri::command]
pub async fn read_shuji_doc(project_dir: String, path: String) -> Result<ShujiDoc, String> {
    let rel = safe_project_path(&path)?;
    let root = PathBuf::from(project_dir);
    let target = root.join(&rel);

    let root_canon = tokio::fs::canonicalize(&root)
        .await
        .map_err(friendly_error)?;
    let target_canon = tokio::fs::canonicalize(&target)
        .await
        .map_err(friendly_error)?;
    if !target_canon.starts_with(&root_canon) {
        return Err(friendly_error("路径越界"));
    }
    if target_canon.is_dir() {
        return Err(friendly_error("不能读取目录"));
    }

    let content = tokio::fs::read_to_string(&target_canon)
        .await
        .map_err(friendly_error)?;
    Ok(ShujiDoc {
        content,
        path: rel.to_string_lossy().replace('\\', "/"),
    })
}

fn safe_project_path(path: &str) -> Result<PathBuf, String> {
    let rel = Path::new(path);
    if rel.is_absolute() {
        return Err("非法路径".to_string());
    }
    if rel.components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err("非法路径".to_string());
    }
    Ok(rel.components().collect())
}

fn build_project_tree(project_dir: &Path) -> Result<Vec<ShujiEntry>, String> {
    if !project_dir.exists() {
        return Ok(vec![]);
    }
    let mut entries = collect_entries(project_dir, project_dir, 0)?;
    sort_entries(&mut entries);
    Ok(entries)
}

fn collect_entries(root: &Path, dir: &Path, depth: usize) -> Result<Vec<ShujiEntry>, String> {
    if depth > 8 {
        return Ok(vec![]);
    }

    let mut entries = Vec::new();
    let read_dir = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    for entry in read_dir.filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if should_skip(&name, &path) {
            continue;
        }

        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if path.is_dir() {
            let mut children = collect_entries(root, &path, depth + 1)?;
            sort_entries(&mut children);
            entries.push(ShujiEntry {
                name,
                path: rel,
                type_label: "目录".to_string(),
                is_dir: true,
                children,
            });
        } else if should_include_file(&path) {
            entries.push(ShujiEntry {
                name,
                path: rel.clone(),
                type_label: infer_label(&path, &rel),
                is_dir: false,
                children: vec![],
            });
        }
    }
    Ok(entries)
}

fn should_skip(name: &str, path: &Path) -> bool {
    let skipped_dirs = [
        ".git",
        "node_modules",
        "target",
        "dist",
        "build",
        ".next",
        ".vite",
        "coverage",
        ".idea",
        ".vscode",
        "__pycache__",
    ];
    if path.is_dir() && skipped_dirs.contains(&name) {
        return true;
    }
    if name == "logs"
        && path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            == Some(".shuji")
    {
        return true;
    }
    false
}

fn should_include_file(path: &Path) -> bool {
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return false,
    };
    if metadata.len() > 512 * 1024 {
        return false;
    }
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "md" | "txt"
            | "json"
            | "jsonl"
            | "toml"
            | "yaml"
            | "yml"
            | "rs"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "css"
            | "html"
            | "xml"
            | "svg"
            | "py"
            | "sh"
            | "ps1"
            | "env"
            | "gitignore"
    ) || path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|name| name.starts_with(".") && name.contains("env"))
}

fn infer_label(path: &Path, rel: &str) -> String {
    if rel.starts_with(".shuji/") {
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let prefix = name.split('_').next().unwrap_or("");
        return match prefix {
            "dsgn" => "方案设计",
            "plan" => "阶段规划",
            "pdsg" => "阶段设计",
            "ddtl" => "详细设计",
            "revw" => "审查",
            "ctrt" => "契约",
            "rprt" => "报告",
            "task" => "任务",
            "reqs" => "需求",
            "anls" => "分析",
            _ => "枢机文档",
        }
        .to_string();
    }

    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "md" => "Markdown",
        "rs" => "Rust",
        "ts" | "tsx" => "TypeScript",
        "js" | "jsx" => "JavaScript",
        "json" | "jsonl" => "JSON",
        "toml" => "TOML",
        "yaml" | "yml" => "YAML",
        "css" => "CSS",
        "html" => "HTML",
        "py" => "Python",
        "svg" => "SVG",
        "env" => "Env",
        _ => "文本",
    }
    .to_string()
}

fn sort_entries(entries: &mut [ShujiEntry]) {
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => {
            let a_shuji = a.path.starts_with(".shuji");
            let b_shuji = b.path.starts_with(".shuji");
            b_shuji
                .cmp(&a_shuji)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        }
    });
}
