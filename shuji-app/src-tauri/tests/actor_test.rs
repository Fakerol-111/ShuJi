//! Actor消息传递测试 - 测试Task/Replace/Interrupt消息处理
//!
//! 运行: cargo test --test actor_test -- --nocapture

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use shuji_app_lib::actor::ActorMessage;
use shuji_app_lib::api::control::RouteMsgType;

// ── ActorMessage 构造测试 ─────────────────────────────────

#[test]
fn test_actor_message_task_creation() {
    let msg = ActorMessage::new("Test task", RouteMsgType::Task);
    assert_eq!(msg.msg_type as isize, RouteMsgType::Task as isize);
    assert_eq!(msg.subject, "Test task");
    assert!(msg.payload.is_none());
}

#[test]
fn test_actor_message_replace_creation() {
    let msg = ActorMessage::new("Replacement task", RouteMsgType::Replace);
    assert_eq!(msg.msg_type as isize, RouteMsgType::Replace as isize);
    assert_eq!(msg.subject, "Replacement task");
}

#[test]
fn test_actor_message_interrupt_creation() {
    let msg = ActorMessage::interrupt();
    assert_eq!(msg.msg_type as isize, RouteMsgType::Interrupt as isize);
    assert!(msg.subject.is_empty());
}

// ── Cancel flag 测试 ──────────────────────────────────────────

#[test]
fn test_cancel_flag_initial_state() {
    let cancel = Arc::new(AtomicBool::new(false));
    assert_eq!(cancel.load(Ordering::SeqCst), false);
}

#[test]
fn test_cancel_flag_set() {
    let cancel = Arc::new(AtomicBool::new(false));
    cancel.store(true, Ordering::SeqCst);
    assert_eq!(cancel.load(Ordering::SeqCst), true);
}

#[test]
fn test_cancel_flag_reset() {
    let cancel = Arc::new(AtomicBool::new(true));
    cancel.store(false, Ordering::SeqCst);
    assert_eq!(cancel.load(Ordering::SeqCst), false);
}

#[test]
fn test_cancel_flag_shared_across_threads() {
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = cancel.clone();
    
    let handle = std::thread::spawn(move || {
        cancel_clone.store(true, Ordering::SeqCst);
    });
    
    handle.join().unwrap();
    assert_eq!(cancel.load(Ordering::SeqCst), true);
}

// ── DeptLogEntry 测试 ─────────────────────────────────────────

#[test]
fn test_dept_log_entry_creation() {
    let entry = shuji_app_lib::actor::DeptLogEntry::new("工部尚书", "开始编码");
    
    assert_eq!(entry.dept, "工部尚书");
    assert_eq!(entry.action, "开始编码");
    assert!(entry.detail.is_none());
    assert!(!entry.ts.is_empty());
}

#[test]
fn test_dept_log_entry_with_detail() {
    let entry = shuji_app_lib::actor::DeptLogEntry::with_detail(
        "工部尚书",
        "创建文件",
        "src/main.rs"
    );
    
    assert_eq!(entry.dept, "工部尚书");
    assert_eq!(entry.action, "创建文件");
    assert_eq!(entry.detail, Some("src/main.rs".to_string()));
}

#[test]
fn test_dept_log_entry_timestamp_format() {
    let entry = shuji_app_lib::actor::DeptLogEntry::new("测试", "动作");
    
    // 时间戳应该是 HH:MM:SS 格式
    assert!(entry.ts.contains(":"));
    let parts: Vec<&str> = entry.ts.split(':').collect();
    assert_eq!(parts.len(), 3);
}

#[test]
fn test_dept_log_entry_serialization() {
    let entry = shuji_app_lib::actor::DeptLogEntry::with_detail(
        "内阁",
        "路由",
        "中书令"
    );
    
    let json = serde_json::to_string(&entry).unwrap();
    let deserialized: shuji_app_lib::actor::DeptLogEntry = 
        serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.dept, entry.dept);
    assert_eq!(deserialized.action, entry.action);
    assert_eq!(deserialized.detail, entry.detail);
}

// ── 消息队列行为测试 ──────────────────────────────────────────

#[test]
fn test_message_queue_ordering() {
    use tokio::sync::mpsc;
    
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    tx.send(ActorMessage::new("First", RouteMsgType::Task)).unwrap();
    tx.send(ActorMessage::new("Second", RouteMsgType::Task)).unwrap();
    tx.send(ActorMessage::new("Third", RouteMsgType::Task)).unwrap();
    
    // 消息应该按FIFO顺序接收
    let msg1 = rx.blocking_recv().unwrap();
    assert_eq!(msg1.subject, "First");
    
    let msg2 = rx.blocking_recv().unwrap();
    assert_eq!(msg2.subject, "Second");
    
    let msg3 = rx.blocking_recv().unwrap();
    assert_eq!(msg3.subject, "Third");
}

#[test]
fn test_message_queue_mixed_types() {
    use tokio::sync::mpsc;
    
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    tx.send(ActorMessage::new("Task", RouteMsgType::Task)).unwrap();
    tx.send(ActorMessage::interrupt()).unwrap();
    tx.send(ActorMessage::new("Replace", RouteMsgType::Replace)).unwrap();
    
    // 验证消息类型顺序
    let msg1 = rx.blocking_recv().unwrap();
    assert_eq!(msg1.msg_type as isize, RouteMsgType::Task as isize);
    
    let msg2 = rx.blocking_recv().unwrap();
    assert_eq!(msg2.msg_type as isize, RouteMsgType::Interrupt as isize);
    
    let msg3 = rx.blocking_recv().unwrap();
    assert_eq!(msg3.msg_type as isize, RouteMsgType::Replace as isize);
}

// ── 并发消息发送测试 ──────────────────────────────────────────

#[test]
fn test_concurrent_message_sending() {
    use tokio::sync::mpsc;
    use std::thread;
    
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    let mut handles = vec![];
    
    for i in 0..10 {
        let tx_clone = tx.clone();
        let handle = thread::spawn(move || {
            tx_clone.send(ActorMessage::new(format!("Task {}", i), RouteMsgType::Task)).unwrap();
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    drop(tx);
    
    let mut received = vec![];
    while let Some(msg) = rx.blocking_recv() {
        received.push(msg.subject);
    }
    
    assert_eq!(received.len(), 10);
}

// ── 消息内容提取测试 ──────────────────────────────────────────

#[test]
fn test_message_subject_extraction() {
    let task = ActorMessage::new("Task content", RouteMsgType::Task);
    let replace = ActorMessage::new("Replace content", RouteMsgType::Replace);
    let interrupt = ActorMessage::interrupt();
    
    assert_eq!(task.subject, "Task content");
    assert_eq!(replace.subject, "Replace content");
    assert!(interrupt.subject.is_empty());
}

// ── 空消息内容测试 ────────────────────────────────────────────

#[test]
fn test_empty_message_content() {
    let msg = ActorMessage::new("", RouteMsgType::Task);
    assert!(msg.subject.is_empty());
}

#[test]
fn test_large_message_content() {
    let large_content = "x".repeat(10000);
    let msg = ActorMessage::new(large_content.clone(), RouteMsgType::Task);
    assert_eq!(msg.subject.len(), 10000);
    assert_eq!(msg.subject, large_content);
}

// ── Unicode消息内容测试 ───────────────────────────────────────

#[test]
fn test_unicode_message_content() {
    let msg = ActorMessage::new("测试中文内容 🚀", RouteMsgType::Task);
    assert!(msg.subject.contains("测试"));
    assert!(msg.subject.contains("🚀"));
}

// ── 消息克隆测试 ──────────────────────────────────────────────

#[test]
fn test_message_clone() {
    let original = ActorMessage::new("Original", RouteMsgType::Task);
    let cloned = original.clone();
    assert_eq!(original.subject, cloned.subject);
    assert_eq!(original.msg_type as isize, cloned.msg_type as isize);
}

// ── 通道关闭测试 ──────────────────────────────────────────────

#[test]
fn test_channel_closed_detection() {
    use tokio::sync::mpsc;
    
    let (tx, mut rx) = mpsc::unbounded_channel::<shuji_app_lib::actor::ActorMessage>();
    
    drop(tx);
    
    let result = rx.blocking_recv();
    assert!(result.is_none());
}

#[test]
fn test_send_after_receiver_dropped() {
    use tokio::sync::mpsc;
    
    let (tx, rx) = mpsc::unbounded_channel::<shuji_app_lib::actor::ActorMessage>();
    
    drop(rx);
    
    let result = tx.send(ActorMessage::new("Test", RouteMsgType::Task));
    
    assert!(result.is_err());
}
