// 存量 clippy warning 允许项 — 逐文件消解（已全部消解完毕）

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
pub mod metrics; // 运行指标收集与查询
pub mod models; // 数据模型：Role、ChatMessage、Project、WorkflowState 等
pub mod pipeline; // Pipeline 引擎：阶段编排、验证、恢复、死锁检测
pub mod playbook; // Playbook 剧本定义：本项目中预定义的协作流程模板
pub mod precepts; // 戒律/规范模块：项目中定义的规则和约束
pub mod pricing; // API 定价计算与统计
pub mod scenario; // 场景定义与回放测试框架
pub mod storage; // 存储层：.shuji/ 目录管理、checkpoint 持久化
pub mod tool; // 工具系统：工具注册表、分发、文件操作、文档操作、审计工具
pub mod validate; // 交付验证模块：端到端验证流程与验证器
pub mod workflow; // 工作流系统：WorkflowProfile、Resolver、Gate、Chain 组合器

// --- 内部模块 (非 pub — 仅供 crate 内部使用) ---
mod commands; // Tauri 命令处理器：project、workflow、settings、checkpoint 等
mod logging; // 日志系统：部门作用域的 JSONL 日志、控制台输出
mod round_metrics; // 轮次指标：当前角色/skill 的实时 token 消耗追踪
mod token_tracker; // Token 追踪器：持久化记录、按时间窗口聚合统计
mod usage_notify; // 度支/文脉面板刷新通知（usage-update 事件）

// ============================================================================
// 类型别名
// ============================================================================

use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 取消标志映射表 — 共享类型别名，用于 agent 和 tool 之间传递取消信号。
///
/// 结构：`Map<Role, Arc<AtomicBool>>`
/// - 内阁通过 `cancel_agent` 工具将一个部门的 flag 置为 true 来中断其执行
/// - 各部门在 `AgentController.run()` 的每次迭代开始时检查自己的 flag
pub type CancelMap = Arc<std::sync::Mutex<HashMap<crate::models::role::Role, Arc<AtomicBool>>>>;

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
use tauri::Emitter;

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
                    let _ = handle.emit("usage-update", &update);
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
            runtime_config: Arc::new(runtime_config),
            compacting_roles: Arc::new(Mutex::new(HashSet::new())),
            discuss_cancel: Arc::new(AtomicBool::new(false)),
        })
        // 注册所有 Tauri 命令 — 前端通过 invoke() 调用这些函数
        .invoke_handler(tauri::generate_handler![
            // --- 项目管理 ---
            commands::project::create_project,
            commands::project::load_project,
            commands::project::get_project,
            commands::project::list_projects,
            // --- 工作流核心 ---
            commands::workflow::send_message, // 向内阁发送用户消息，启动工作流
            commands::workflow::discuss_with_cabinet, // 讨论模式（无工具、不修改项目）
            commands::workflow::get_snapshot, // 获取文档目录快照
            commands::workflow::read_document, // 读取 .shuji/ 下文档内容
            commands::workflow::list_documents, // 列出所有文档
            commands::workflow::list_log_files, // 列出日志文件
            commands::workflow::read_log_file, // 读取日志文件内容
            commands::workflow::get_recent_dirs, // 获取最近使用的目录
            // --- Token & 上下文统计 ---
            commands::workflow::get_token_stats, // Token 消耗统计（按时间窗口）
            commands::workflow::get_context_stats, // 上下文使用统计
            commands::workflow::compact_context, // 手动触发上下文压缩
            commands::workflow::get_round_metrics, // 当前轮次指标
            commands::workflow::get_active_roles, // 获取当前活跃的角色
            // --- 取消操作 ---
            commands::workflow::cancel_discuss,    // 取消讨论模式
            commands::workflow::cancel_processing, // 取消当前处理流程
            // --- 聊天 & 日志 ---
            commands::workflow::get_chat_history, // 获取聊天记录
            commands::workflow::get_dept_logs,    // 获取部门日志
            // --- 审批系统 ---
            commands::workflow::get_pending_approvals, // 获取待审批文档列表
            commands::workflow::set_document_status,   // 设置文档审批状态（朱批）
            // --- 演示模式 ---
            commands::demo::create_demo_project, // 创建演示项目
            // --- 设置 ---
            commands::settings::get_config,  // 获取 config.toml 配置
            commands::settings::save_config, // 保存 config.toml 配置
            commands::settings::set_dotenv_key, // 设置 .env 环境变量
            commands::settings::get_context_config, // 获取上下文压缩配置
            commands::settings::save_context_config, // 保存上下文压缩配置
            commands::settings::reset_context_config, // 重置上下文压缩配置
            commands::settings::check_api_connection, // 测试 API 连接
            commands::settings::get_workflow_preset, // 获取工作流预设
            commands::settings::set_workflow_preset, // 设置工作流预设
            commands::settings::get_workflow_config, // 获取工作流配置
            commands::settings::set_workflow_config, // 设置工作流配置
            commands::settings::get_soul_content, // 获取内阁 soul.md 内容
            commands::settings::clear_soul,  // 清空内阁 soul
            commands::settings::get_model_preset, // 获取模型预设
            commands::settings::set_model_preset, // 设置模型预设
            // --- 文档浏览器 ---
            commands::shuji_docs::list_shuji_tree, // 获取 .shuji/ 目录树
            commands::shuji_docs::read_shuji_doc,  // 读取 .shuji/ 文档
            commands::shuji_docs::get_document_diff, // 获取文档 diff
            // --- Checkpoint ---
            commands::checkpoint::list_checkpoints, // 列出 checkpoint 快照
            commands::checkpoint::restore_checkpoint, // 恢复到指定 checkpoint
            // --- 审计 ---
            commands::workflow::get_document_lineage, // 获取文档谱系
            commands::workflow::get_audit_timeline,   // 获取审计时间线
            commands::workflow::generate_delivery_report, // 生成交付报告
            commands::workflow::get_document_diffs,   // 获取文档 diff 列表
            commands::workflow::read_document_diff,   // 读取文档 diff
            commands::workflow::trace_document,       // 追踪文档上下游依赖
            commands::workflow::verify_audit_trail,   // 验证审计轨迹完整性
            // --- 定价 ---
            commands::pricing::get_pricing,     // 获取定价信息
            commands::pricing::save_pricing,    // 保存定价信息
            commands::pricing::refresh_pricing, // 刷新定价信息
            // --- Pipeline ---
            commands::workflow::get_pipeline_status, // 获取 pipeline 状态
            commands::workflow::get_tool_logs,       // 获取工具调用日志
            commands::workflow::get_workflow_state,  // 获取工作流状态
            commands::workflow::get_workflow_graph,  // 获取工作流图谱
            commands::workflow::list_workflow_archives, // 列出工作流归档
            commands::workflow::load_workflow_archive, // 加载工作流归档
            // --- 验证 & 指标 ---
            commands::validate::validate_delivery_cmd, // 执行交付验证
            commands::metrics::get_latest_run_metrics, // 获取最近运行指标
            commands::metrics::list_run_metrics,       // 列出历史运行指标
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
