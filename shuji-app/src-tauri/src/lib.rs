// 存量 clippy warning 允许项 — 逐文件消解（已全部消解完毕）

use tauri::Manager;

// ============================================================================
// 宏定义
// ============================================================================

/// 控制台日志宏 — 通过专用 writer 任务输出日志。
///
/// 日志行通过 mpsc 通道发送到单个后台任务，由该任务顺序写入 stderr，
/// 从而防止多线程环境下日志行交错输出的问题。
///
/// 使用方式：
/// ```ignore
/// log_console!("[{}] 任务完成，耗时 {}ms", role_name, elapsed);
/// ```
#[macro_export]
macro_rules! log_console {
    ($($arg:tt)*) => {{
        $crate::logging::logger::console_send(format!($($arg)*));
    }};
}

// ============================================================================
// 模块声明
// ============================================================================

// --- 核心业务模块 (pub — 外部可访问) ---
pub mod actor; // Actor 系统：角色邮箱、消息路由、FastMessage/FastChannel
pub mod agent; // Agent trait + 通用执行框架 (runner) + 9 部门 + 2 子 agent
pub mod api; // LLM API 层：双格式 HTTP client、会话管理、AgentController、上下文压缩
pub mod audit; // 审计系统：事件日志、引用索引、文档谱系、diff 追踪、合规检查单
pub mod config; // 配置系统：RuntimeConfig (TOML)、优先级合并、阈值解析
pub mod events; // Tauri 事件名称常量与类型化 emit 辅助函数
pub mod learning; // 角色化学习记忆：soul 读写、注入、结构化索引
pub mod metrics; // 运行指标收集与查询
pub mod models; // 数据模型：Role、ChatMessage、Project 等
pub mod pipeline; // Pipeline 引擎：阶段编排、验证、恢复、死锁检测
pub mod playbook; // Playbook 剧本定义：本项目中预定义的协作流程模板
pub mod precepts; // 戒律/规范模块：项目中定义的规则和约束
pub mod pricing; // API 定价计算与统计
pub mod scenario; // 场景定义与回放测试框架
pub mod storage; // 存储层：.shuji/ 目录管理、checkpoint 持久化
pub mod tool; // 工具系统：工具注册表、分发、文件操作、文档操作、审计工具
pub mod util;
pub mod validate; // 交付验证模块：端到端验证流程与验证器
pub mod workflow; // 工作流系统：WorkflowProfile、Resolver、Gate、Chain 组合器 // 通用工具：锁辅助等

// --- 内部模块 (非 pub — 仅供 crate 内部使用) ---
mod commands; // Tauri 命令处理器：project、workflow、settings、checkpoint 等
mod logging; // 日志系统：部门作用域的 JSONL 日志、控制台输出
mod round_metrics; // 轮次指标：当前角色/skill 的实时 token 消耗追踪
mod runtime_notify;
mod token_tracker; // Token 追踪器：持久化记录、按时间窗口聚合统计
mod usage_notify; // 度支/文脉面板刷新通知（usage-update 事件） //  cockpit 实时状态（runtime-update 事件）

// ============================================================================
// 类型别名
// ============================================================================

use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;

/// 取消标志映射表 — 共享类型别名，用于 agent 和 tool 之间传递取消信号。
///
/// 结构：`Arc<HashMap<Role, Arc<AtomicBool>>>`
/// - 初始化后只读（key 集合固定为 9 个角色），flag 通过 AtomicBool 无锁修改
/// - 内阁通过 `cancel_agent` 工具将一个部门的 flag 置为 true 来中断其执行
/// - 各部门在 `AgentController.run()` 的每次迭代开始时检查自己的 flag
pub type CancelMap = Arc<HashMap<crate::models::role::Role, Arc<AtomicBool>>>;

/// 快速消息发送器映射表 — 用于内阁向特定部门发送即时控制消息。
///
/// 结构：`Map<Role, mpsc::Sender<FastMessage>>`
/// - `FastMessage` 携带更高优先级的中断/控制指令
/// - 通过专用的 mpsc channel 直接送达目标 agent 的 mailbox
pub type FastTxMap =
    Arc<HashMap<crate::models::role::Role, tokio::sync::mpsc::Sender<crate::actor::FastMessage>>>;

// ============================================================================
// 应用入口
// ============================================================================

use commands::project::AppState;
use config::RuntimeConfig;

/// Tauri 应用主入口函数。
///
/// 负责：
/// 1. 加载 `config.toml` 运行时配置（失败则使用默认值）
/// 2. 注册 Tauri 插件（shell、dialog）
/// 3. 初始化全局 `AppState` 并注入 Tauri 状态管理
/// 4. 注册所有前端可调用的 Tauri 命令
///
/// 全局状态字段说明 (AppState):
/// - `current_project`: 当前打开的项目路径
/// - `current_dir`: 当前工作目录
/// - `cancel_flag`: 全局取消标志（用户点击"停止"时置 true）
/// - `actor_system`: Actor 系统句柄（None 表示未启动）
/// - `chat_history`: 当前会话的聊天记录缓存
/// - `dept_log_history`: 部门日志历史缓存
/// - `runtime_config`: 运行时配置（线程安全共享）
/// - `compacting_roles`: 正在执行上下文压缩的角色集合（防重复点击）
/// - `discuss_cancel`: 讨论模式取消标志
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 加载运行时配置，文件缺失或解析失败时回退到默认值
    let runtime_config = RuntimeConfig::load_or_default("config.toml");

    tauri::Builder::default()
        // 注册 Tauri 插件：shell（命令执行）、dialog（文件对话框）
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let (usage_tx, mut usage_rx) =
                tokio::sync::mpsc::unbounded_channel::<usage_notify::UsageUpdate>();
            usage_notify::set_sender(usage_tx);
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Some(update) = usage_rx.recv().await {
                    let _ = crate::events::emit_usage_update(&handle, &update);
                }
            });

            let (runtime_tx, mut runtime_rx) =
                tokio::sync::mpsc::unbounded_channel::<runtime_notify::RuntimeUpdate>();
            runtime_notify::set_sender(runtime_tx);
            let runtime_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Some(update) = runtime_rx.recv().await {
                    let _ = crate::events::emit_runtime_update(&runtime_handle, &update);
                }
            });
            Ok(())
        })
        // 注入全局应用状态 — 所有字段均为 Arc 包装，支持跨线程安全共享
        .manage(AppState {
            current_project: Arc::new(Mutex::new(None)),
            current_dir: Arc::new(Mutex::new(None)),
            cancel_flag: Arc::new(AtomicBool::new(false)),
            actor_system: Arc::new(tokio::sync::Mutex::new(None)),
            chat_history: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            dept_log_history: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            runtime_config: Arc::new(RwLock::new(runtime_config)),
            compacting_roles: Arc::new(Mutex::new(HashSet::new())),
            discuss_cancel: Arc::new(AtomicBool::new(false)),
            pipeline_supervisor: Arc::new(crate::pipeline::supervisor::PipelineSupervisor::new()),
        })
        // 注册所有 Tauri 命令 — 前端通过 invoke() 调用这些函数
        .invoke_handler(tauri::generate_handler![
            // --- 项目管理 ---
            commands::project::create_project,
            commands::project::load_project,
            commands::project::get_project,
            commands::project::list_projects,
            commands::workflow::send_message,
            commands::workflow::discuss_with_cabinet,
            commands::workflow::discuss_stream,
            commands::workflow::get_snapshot,
            commands::workflow::read_document,
            commands::workflow::list_documents,
            commands::workflow::list_log_files,
            commands::workflow::read_log_file,
            commands::workflow::get_recent_dirs,
            commands::workflow::get_token_stats,
            commands::workflow::get_context_stats,
            commands::workflow::compact_context,
            commands::workflow::cancel_discuss,
            commands::workflow::cancel_processing,
            commands::workflow::get_chat_history,
            commands::workflow::get_dept_logs,
            commands::workflow::get_round_metrics,
            commands::workflow::get_active_roles,
            commands::workflow::get_pending_approvals,
            commands::workflow::set_document_status,
            commands::demo::create_demo_project,
            commands::demo::reset_demo_project,
            commands::settings::api_config::get_config,
            commands::settings::api_config::save_config,
            commands::settings::api_config::set_dotenv_key,
            commands::settings::context::get_context_config,
            commands::settings::context::save_context_config,
            commands::settings::context::reset_context_config,
            commands::settings::connection::check_api_connection,
            commands::settings::workflow_preset::get_workflow_preset,
            commands::settings::workflow_preset::set_workflow_preset,
            commands::settings::soul::get_soul_content,
            commands::settings::soul::clear_soul,
            commands::settings::soul::list_soul_roles,
            commands::settings::learning::list_global_learning_candidates,
            commands::settings::learning::approve_global_learning,
            commands::settings::learning::reject_global_learning,
            commands::settings::learning::get_learning_config,
            commands::settings::learning::set_learning_global_enabled,
            commands::settings::model_preset::get_model_preset,
            commands::settings::model_preset::set_model_preset,
            commands::settings::approval::get_approval_config,
            commands::settings::approval::set_approval_config,
            commands::settings::reasoning::get_reasoning_config,
            commands::settings::reasoning::set_reasoning_config,
            commands::settings::diagnostics::export_effective_config,
            commands::shuji_docs::list_shuji_tree,
            commands::shuji_docs::read_shuji_doc,
            commands::shuji_docs::get_document_diff,
            commands::checkpoint::list_checkpoints,
            commands::checkpoint::restore_checkpoint,
            commands::workflow::get_document_lineage,
            commands::workflow::get_document_line_run,
            commands::workflow::get_document_line_for_doc,
            commands::workflow::list_document_line_runs,
            commands::workflow::analyze_document_impact,
            commands::workflow::get_audit_timeline,
            commands::workflow::generate_delivery_report,
            commands::workflow::get_document_diffs,
            commands::workflow::read_document_diff,
            commands::workflow::trace_document,
            commands::workflow::query_documents,
            commands::pricing::get_pricing,
            commands::pricing::save_pricing,
            commands::pricing::refresh_pricing,
            commands::workflow::verify_audit_trail,
            commands::workflow::export_diagnostics,
            commands::workflow::get_pipeline_status,
            commands::workflow::get_tool_logs,
            commands::workflow::get_workflow_graph,
            commands::workflow::list_workflow_archives,
            commands::workflow::load_workflow_archive,
            commands::validate::validate_delivery_cmd,
            commands::metrics::get_latest_run_metrics,
            commands::metrics::list_run_metrics,
            commands::editor::get_editor_config,
            commands::editor::set_editor_config,
            commands::editor::check_external_editor,
            commands::editor::open_in_external_editor,
            commands::editor::open_project_in_external_editor,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                log_console!("[app] exit requested, shutting down actors...");
                if let Some(state) = app_handle.try_state::<AppState>() {
                    // 1. 设置全局取消标志
                    state
                        .cancel_flag
                        .store(true, std::sync::atomic::Ordering::SeqCst);

                    // 2. 通过 tokio runtime 执行异步清理
                    let rt = tauri::async_runtime::handle();
                    rt.block_on(async {
                        if let Some(sys) = state.actor_system.lock().await.as_ref() {
                            // 协作式取消：设置所有 per-actor flag + 发送 Interrupt
                            for flag in sys.cancel_map.values() {
                                flag.store(true, std::sync::atomic::Ordering::SeqCst);
                            }
                            for tx in sys.fast_txs.values() {
                                let _ = tx.try_send(crate::actor::FastMessage::Interrupt);
                            }
                            // 等待短暂时间让 actor 完成当前操作
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                            // 强制 abort
                            sys.abort_all_actors();
                        }
                    });
                }
                log_console!("[app] shutdown complete");
            }
        });
}
