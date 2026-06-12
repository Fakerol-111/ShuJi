//! 手动测试需求展开 sub-agent（需要 .env 中的 API 配置）。
//! CI 通过 --skip expand_requirements 跳过此文件。
//!
//! 运行全部: cd src-tauri && cargo test --test expand_requirements_test -- --nocapture
//! 运行单个: cargo test --test expand_requirements_test expand_requirements_1 -- --nocapture

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn load_env() {
    for path in &[".env", "../.env"] {
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                std::env::set_var(k.trim(), v.trim().trim_matches('"').trim_matches('\''));
            }
        }
    }
}

fn project_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap()
        .parent()
        .unwrap()
        .join("test_workspace")
}

fn setup_client() -> (Arc<shuji_app_lib::api::client::AnthropicClient>, String) {
    let api_key = std::env::var("NEIGE_API_KEY")
        .or_else(|_| std::env::var("DEFAULT_API_KEY"))
        .expect("缺少 API key");
    let api_url = std::env::var("DEFAULT_API_URL")
        .unwrap_or_else(|_| "https://api.deepseek.com/chat/completions".into());
    let model = std::env::var("DEFAULT_MODEL").unwrap_or_else(|_| "deepseek-v4-flash".into());
    (
        Arc::new(shuji_app_lib::api::client::AnthropicClient::new(
            api_key, api_url,
        )),
        model,
    )
}

fn setup_task(task_id: &str, counter: u64, content: &str) -> PathBuf {
    let project_dir = project_dir();
    let shuji_dir = project_dir.join(".shuji");
    std::fs::create_dir_all(&shuji_dir).unwrap();
    let task_content = format!("---\nid: {}\nrefs: [-1]\n---\n\n{}", task_id, content);
    let task_path = shuji_dir.join("tasks").join(format!("{}.md", task_id));
    std::fs::create_dir_all(task_path.parent().unwrap()).unwrap();
    std::fs::write(&task_path, task_content).unwrap();
    std::fs::write(shuji_dir.join("_counter"), counter.to_string()).unwrap();
    task_path
}

fn print_result(doc_id: &str) {
    let requirements_dir = project_dir().join(".shuji").join("requirements");
    if let Ok(entries) = std::fs::read_dir(&requirements_dir) {
        for entry in entries.flatten() {
            if let Ok(content) = std::fs::read_to_string(entry.path()) {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.contains(doc_id.split('_').next_back().unwrap_or("")) {
                    println!("\n[{}]", entry.path().display());
                    println!("{}", content);
                }
            }
        }
    }
}

// ── 测试 1: 极模糊需求 ──────────────────────────────────────────

#[tokio::test]
async fn expand_requirements_1_vague() {
    load_env();
    let (client, model) = setup_client();
    setup_task(
        "task_test1",
        100,
        "皇帝需求：做一个简单的待办事项应用。用户能添加、完成、删除任务。",
    );

    println!("\n=== 测试1: 极模糊需求（待办事项） ===");
    let cancel = AtomicBool::new(false);
    let result = shuji_app_lib::agent::expand_requirements::run(
        "task_test1",
        &project_dir(),
        &client,
        &model,
        &cancel,
    )
    .await;

    match &result {
        Ok(id) => {
            println!("\n文档 ID: {}", id);
            print_result(id);
        }
        Err(e) => println!("\n失败: {}", e),
    }
    assert!(result.is_ok());
}

// ── 测试 2: 较明确需求 ──────────────────────────────────────────

#[tokio::test]
async fn expand_requirements_2_clear() {
    load_env();
    let (client, model) = setup_client();
    setup_task(
        "task_test2",
        200,
        "皇帝需求：给博客系统增加文章评论功能。\
        用户能从文章页看到评论区，已登录用户可发表评论，\
        评论支持 Markdown 格式，作者可删除自己文章的任意评论，\
        评论者可在 5 分钟内编辑自己的评论。\
        评论按时间正序显示，超过 50 条时分页加载。\
        未登录用户只能看评论不能发。",
    );

    println!("\n=== 测试2: 较明确需求（博客评论） ===");
    let cancel = AtomicBool::new(false);
    let result = shuji_app_lib::agent::expand_requirements::run(
        "task_test2",
        &project_dir(),
        &client,
        &model,
        &cancel,
    )
    .await;

    match &result {
        Ok(id) => {
            println!("\n文档 ID: {}", id);
            print_result(id);
        }
        Err(e) => println!("\n失败: {}", e),
    }
    assert!(result.is_ok());
}

// ── 测试 3: 极宽需求 ────────────────────────────────────────────

#[tokio::test]
async fn expand_requirements_3_broad() {
    load_env();
    let (client, model) = setup_client();
    setup_task("task_test3", 300, "皇帝需求：做一个电商系统。");

    println!("\n=== 测试3: 极宽需求（电商系统） ===");
    let cancel = AtomicBool::new(false);
    let result = shuji_app_lib::agent::expand_requirements::run(
        "task_test3",
        &project_dir(),
        &client,
        &model,
        &cancel,
    )
    .await;

    match &result {
        Ok(id) => {
            println!("\n文档 ID: {}", id);
            print_result(id);
        }
        Err(e) => println!("\n失败: {}", e),
    }
    assert!(result.is_ok());
}

// ── 测试 4: 非功能需求 ──────────────────────────────────────────

#[tokio::test]
async fn expand_requirements_4_non_functional() {
    load_env();
    let (client, model) = setup_client();
    setup_task(
        "task_test4",
        400,
        "皇帝需求：让应用加载更快。用户反馈首页打开要3秒，太慢了。",
    );

    println!("\n=== 测试4: 非功能需求（性能优化） ===");
    let cancel = AtomicBool::new(false);
    let result = shuji_app_lib::agent::expand_requirements::run(
        "task_test4",
        &project_dir(),
        &client,
        &model,
        &cancel,
    )
    .await;

    match &result {
        Ok(id) => {
            println!("\n文档 ID: {}", id);
            print_result(id);
        }
        Err(e) => println!("\n失败: {}", e),
    }
    assert!(result.is_ok());
}

// ── 测试 5: 极窄需求 ────────────────────────────────────────────

#[tokio::test]
async fn expand_requirements_5_narrow() {
    load_env();
    let (client, model) = setup_client();
    setup_task(
        "task_test5",
        500,
        "皇帝需求：给删除按钮加个 loading 状态，点击后显示加载动画，请求完成后再消失。",
    );

    println!("\n=== 测试5: 极窄需求（按钮 loading） ===");
    let cancel = AtomicBool::new(false);
    let result = shuji_app_lib::agent::expand_requirements::run(
        "task_test5",
        &project_dir(),
        &client,
        &model,
        &cancel,
    )
    .await;

    match &result {
        Ok(id) => {
            println!("\n文档 ID: {}", id);
            print_result(id);
        }
        Err(e) => println!("\n失败: {}", e),
    }
    assert!(result.is_ok());
}

// ── 测试 6: 开发者需求 ──────────────────────────────────────────

#[tokio::test]
async fn expand_requirements_6_dev_internal() {
    load_env();
    let (client, model) = setup_client();
    setup_task(
        "task_test6",
        600,
        "皇帝需求：重构数据库连接池。当前连接池在高并发下频繁超时，需要换用更高效的池化方案。",
    );

    println!("\n=== 测试6: 开发者需求（连接池重构） ===");
    let cancel = AtomicBool::new(false);
    let result = shuji_app_lib::agent::expand_requirements::run(
        "task_test6",
        &project_dir(),
        &client,
        &model,
        &cancel,
    )
    .await;

    match &result {
        Ok(id) => {
            println!("\n文档 ID: {}", id);
            print_result(id);
        }
        Err(e) => println!("\n失败: {}", e),
    }
    assert!(result.is_ok());
}
