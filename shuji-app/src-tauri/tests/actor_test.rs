//! Actor消息传递测试 - 测试Task/Replace/Interrupt消息处理
//!
//! 运行: cargo test --test actor_test -- --nocapture

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// ── ActorMessage 枚举测试 ─────────────────────────────────────

#[test]
fn test_actor_message_task_creation() {
    let msg = shuji_app_lib::actor::ActorMessage::Task {
        content: "Test task".to_string()
    };
    
    match msg {
        shuji_app_lib::actor::ActorMessage::Task { content } => {
            assert_eq!(content, "Test task");
        }
        _ => panic!("Expected Task message"),
    }
}

#[test]
fn test_actor_message_replace_creation() {
    let msg = shuji_app_lib::actor::ActorMessage::Replace {
        content: "Replacement task".to_string()
    };
    
    match msg {
        shuji_app_lib::actor::ActorMessage::Replace { content } => {
            assert_eq!(content, "Replacement task");
        }
        _ => panic!("Expected Replace message"),
    }
}

#[test]
fn test_actor_message_interrupt_creation() {
    let msg = shuji_app_lib::actor::ActorMessage::Interrupt;
    
    match msg {
        shuji_app_lib::actor::ActorMessage::Interrupt => {
            // Success
        }
        _ => panic!("Expected Interrupt message"),
    }
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
    
    tx.send(shuji_app_lib::actor::ActorMessage::Task {
        content: "First".to_string()
    }).unwrap();
    
    tx.send(shuji_app_lib::actor::ActorMessage::Task {
        content: "Second".to_string()
    }).unwrap();
    
    tx.send(shuji_app_lib::actor::ActorMessage::Task {
        content: "Third".to_string()
    }).unwrap();
    
    // 消息应该按FIFO顺序接收
    let msg1 = rx.blocking_recv().unwrap();
    match msg1 {
        shuji_app_lib::actor::ActorMessage::Task { content } => {
            assert_eq!(content, "First");
        }
        _ => panic!("Expected Task message"),
    }
    
    let msg2 = rx.blocking_recv().unwrap();
    match msg2 {
        shuji_app_lib::actor::ActorMessage::Task { content } => {
            assert_eq!(content, "Second");
        }
        _ => panic!("Expected Task message"),
    }
    
    let msg3 = rx.blocking_recv().unwrap();
    match msg3 {
        shuji_app_lib::actor::ActorMessage::Task { content } => {
            assert_eq!(content, "Third");
        }
        _ => panic!("Expected Task message"),
    }
}

#[test]
fn test_message_queue_mixed_types() {
    use tokio::sync::mpsc;
    
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    tx.send(shuji_app_lib::actor::ActorMessage::Task {
        content: "Task".to_string()
    }).unwrap();
    
    tx.send(shuji_app_lib::actor::ActorMessage::Interrupt).unwrap();
    
    tx.send(shuji_app_lib::actor::ActorMessage::Replace {
        content: "Replace".to_string()
    }).unwrap();
    
    // 验证消息类型顺序
    let msg1 = rx.blocking_recv().unwrap();
    assert!(matches!(msg1, shuji_app_lib::actor::ActorMessage::Task { .. }));
    
    let msg2 = rx.blocking_recv().unwrap();
    assert!(matches!(msg2, shuji_app_lib::actor::ActorMessage::Interrupt));
    
    let msg3 = rx.blocking_recv().unwrap();
    assert!(matches!(msg3, shuji_app_lib::actor::ActorMessage::Replace { .. }));
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
            tx_clone.send(shuji_app_lib::actor::ActorMessage::Task {
                content: format!("Task {}", i)
            }).unwrap();
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    drop(tx);
    
    let mut received = vec![];
    while let Some(msg) = rx.blocking_recv() {
        match msg {
            shuji_app_lib::actor::ActorMessage::Task { content } => {
                received.push(content);
            }
            _ => {}
        }
    }
    
    assert_eq!(received.len(), 10);
}

// ── 消息内容提取测试 ──────────────────────────────────────────

#[test]
fn test_message_subject_extraction() {
    let task = shuji_app_lib::actor::ActorMessage::Task {
        content: "Task content".to_string()
    };
    
    let replace = shuji_app_lib::actor::ActorMessage::Replace {
        content: "Replace content".to_string()
    };
    
    let interrupt = shuji_app_lib::actor::ActorMessage::Interrupt;
    
    // 测试subject()方法（如果公开的话）
    // 这里我们只能通过模式匹配来验证
    match task {
        shuji_app_lib::actor::ActorMessage::Task { content } => {
            assert_eq!(content, "Task content");
        }
        _ => panic!(),
    }
    
    match replace {
        shuji_app_lib::actor::ActorMessage::Replace { content } => {
            assert_eq!(content, "Replace content");
        }
        _ => panic!(),
    }
    
    match interrupt {
        shuji_app_lib::actor::ActorMessage::Interrupt => {
            // Interrupt没有content
        }
        _ => panic!(),
    }
}

// ── 空消息内容测试 ────────────────────────────────────────────

#[test]
fn test_empty_message_content() {
    let msg = shuji_app_lib::actor::ActorMessage::Task {
        content: String::new()
    };
    
    match msg {
        shuji_app_lib::actor::ActorMessage::Task { content } => {
            assert!(content.is_empty());
        }
        _ => panic!(),
    }
}

#[test]
fn test_large_message_content() {
    let large_content = "x".repeat(10000);
    let msg = shuji_app_lib::actor::ActorMessage::Task {
        content: large_content.clone()
    };
    
    match msg {
        shuji_app_lib::actor::ActorMessage::Task { content } => {
            assert_eq!(content.len(), 10000);
            assert_eq!(content, large_content);
        }
        _ => panic!(),
    }
}

// ── Unicode消息内容测试 ───────────────────────────────────────

#[test]
fn test_unicode_message_content() {
    let msg = shuji_app_lib::actor::ActorMessage::Task {
        content: "测试中文内容 🚀".to_string()
    };
    
    match msg {
        shuji_app_lib::actor::ActorMessage::Task { content } => {
            assert!(content.contains("测试"));
            assert!(content.contains("🚀"));
        }
        _ => panic!(),
    }
}

// ── 消息克隆测试 ──────────────────────────────────────────────

#[test]
fn test_message_clone() {
    let original = shuji_app_lib::actor::ActorMessage::Task {
        content: "Original".to_string()
    };
    
    let cloned = original.clone();
    
    match (original, cloned) {
        (
            shuji_app_lib::actor::ActorMessage::Task { content: c1 },
            shuji_app_lib::actor::ActorMessage::Task { content: c2 }
        ) => {
            assert_eq!(c1, c2);
        }
        _ => panic!(),
    }
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
    
    let result = tx.send(shuji_app_lib::actor::ActorMessage::Task {
        content: "Test".to_string()
    });
    
    assert!(result.is_err());
}
