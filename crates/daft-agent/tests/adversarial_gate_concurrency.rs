//! Adversarial concurrency stress harness for Daft Agent messaging system.
//! Verifies zero duplicate message IDs and safe Maildir delivery under heavy multithreaded contention.

use daft_agent::{AgentManager, AgentMessage, AgentType};
use daft_core::init::{init, InitOptions};
use daft_core::Repository;
use daft_dimension::DimensionManager;
use std::collections::HashSet;
use std::fs;
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::TempDir;

fn setup_test_repo() -> (TempDir, Arc<Repository>) {
    let tmp = TempDir::new().expect("create tempdir");
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).expect("create repo_root");

    init(&repo_root, &InitOptions::default()).expect("init repo");
    let repo = Arc::new(Repository::open(&repo_root.join(".dft")).expect("open repo"));
    let manager = DimensionManager::new(repo.clone());
    let _ = manager.init();
    (tmp, repo)
}

#[test]
fn test_adversarial_heavy_multithreaded_message_id_uniqueness() {
    // 20 concurrent threads generating 200 messages each = 4,000 messages
    let num_threads = 20;
    let msgs_per_thread = 200;
    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = Vec::new();

    for t in 0..num_threads {
        let b = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            b.wait();
            let mut ids = Vec::with_capacity(msgs_per_thread);
            for i in 0..msgs_per_thread {
                let msg = AgentMessage::new_broadcast(
                    &format!("agent-{}", t),
                    &format!("subject-{}", i),
                    "Empirical stress testing payload",
                );
                ids.push(msg.id);
            }
            ids
        }));
    }

    let mut all_ids = HashSet::with_capacity(num_threads * msgs_per_thread);
    let mut duplicates = Vec::new();

    for h in handles {
        let thread_ids = h.join().expect("thread join");
        for id in thread_ids {
            if !all_ids.insert(id.clone()) {
                duplicates.push(id);
            }
        }
    }

    assert!(
        duplicates.is_empty(),
        "CRITICAL: Found {} duplicate message IDs under concurrent load! Duplicates: {:?}",
        duplicates.len(),
        duplicates
    );
    assert_eq!(
        all_ids.len(),
        num_threads * msgs_per_thread,
        "Total unique message IDs must match total messages generated"
    );
}

#[test]
fn test_adversarial_concurrent_mailbox_delivery_integrity() {
    let (_tmp, repo) = setup_test_repo();
    let manager = Arc::new(AgentManager::new(Arc::clone(&repo)));

    // Register receiver
    manager
        .register("target-agent", AgentType::Ai, vec![])
        .expect("register target");

    let num_senders = 10;
    let msgs_per_sender = 50;
    let barrier = Arc::new(Barrier::new(num_senders));
    let mut handles = Vec::new();

    for s in 0..num_senders {
        let mgr = Arc::clone(&manager);
        let bar = Arc::clone(&barrier);
        let sender_name = format!("sender-{}", s);
        mgr.register(&sender_name, AgentType::Ai, vec![])
            .expect("register sender");

        handles.push(thread::spawn(move || {
            bar.wait();
            for m in 0..msgs_per_sender {
                let res = mgr.send_message(
                    &sender_name,
                    "target-agent",
                    &format!("Msg-{}", m),
                    "Payload",
                );
                assert!(res.is_ok(), "Failed to send message: {:?}", res.err());
            }
        }));
    }

    for h in handles {
        h.join().expect("sender thread join");
    }

    let inbox = manager
        .read_inbox(Some("target-agent"), false)
        .expect("read inbox");
    assert_eq!(
        inbox.len(),
        num_senders * msgs_per_sender,
        "Inbox message count mismatch: expected {}, got {}",
        num_senders * msgs_per_sender,
        inbox.len()
    );

    // Verify all IDs in inbox are unique
    let mut inbox_ids = HashSet::new();
    for msg in &inbox {
        assert!(
            inbox_ids.insert(msg.id.clone()),
            "Duplicate ID in Maildir: {}",
            msg.id
        );
    }
}
