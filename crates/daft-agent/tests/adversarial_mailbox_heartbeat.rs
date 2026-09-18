use daft_agent::{AgentLiveness, AgentManager, AgentType};
use daft_core::init::{init, InitOptions};
use daft_core::Repository;
use daft_dimension::DimensionManager;
use std::collections::HashSet;
use std::fs;
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::TempDir;

fn setup_repo() -> (TempDir, Arc<Repository>) {
    let tmp = TempDir::new().unwrap();
    let repo_root = tmp.path().join("repo");
    fs::create_dir_all(&repo_root).unwrap();

    init(&repo_root, &InitOptions::default()).unwrap();
    let repo = Arc::new(Repository::open(&repo_root.join(".dft")).unwrap());
    let manager = DimensionManager::new(repo.clone());
    let _ = manager.init();
    (tmp, repo)
}

#[test]
fn test_concurrent_id_collision() {
    let barrier = Arc::new(Barrier::new(8));
    let mut handles = Vec::new();
    for _ in 0..8 {
        let b = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            b.wait();
            let mut ids = Vec::new();
            for _ in 0..50 {
                let msg = daft_agent::AgentMessage::new_broadcast("s", "sub", "body");
                ids.push(msg.id);
            }
            ids
        }));
    }

    let mut all_ids = HashSet::new();
    let mut duplicate_count = 0;
    for h in handles {
        let ids = h.join().unwrap();
        for id in ids {
            if !all_ids.insert(id) {
                duplicate_count += 1;
            }
        }
    }
    println!(
        "Duplicate message IDs found across concurrent threads: {}",
        duplicate_count
    );
    assert_eq!(
        duplicate_count, 0,
        "Concurrent threads generated duplicate message IDs!"
    );
}

#[test]
fn test_heartbeat_boundary_transitions() {
    let now = 1_000_000u64;

    // Active boundaries: [0, 60)
    assert_eq!(
        AgentLiveness::evaluate(Some(now), now),
        AgentLiveness::Active,
        "delta 0 should be Active"
    );
    assert_eq!(
        AgentLiveness::evaluate(Some(now - 1), now),
        AgentLiveness::Active,
        "delta 1 should be Active"
    );
    assert_eq!(
        AgentLiveness::evaluate(Some(now - 59), now),
        AgentLiveness::Active,
        "delta 59 should be Active"
    );

    // Idle boundaries: [60, 300]
    assert_eq!(
        AgentLiveness::evaluate(Some(now - 60), now),
        AgentLiveness::Idle,
        "delta 60 should transition to Idle"
    );
    assert_eq!(
        AgentLiveness::evaluate(Some(now - 150), now),
        AgentLiveness::Idle,
        "delta 150 should be Idle"
    );
    assert_eq!(
        AgentLiveness::evaluate(Some(now - 300), now),
        AgentLiveness::Idle,
        "delta 300 should be Idle"
    );

    // Offline boundaries: > 300
    assert_eq!(
        AgentLiveness::evaluate(Some(now - 301), now),
        AgentLiveness::Offline,
        "delta 301 should transition to Offline"
    );
    assert_eq!(
        AgentLiveness::evaluate(Some(now - 86400), now),
        AgentLiveness::Offline,
        "delta 86400 should be Offline"
    );

    // Missing heartbeat
    assert_eq!(
        AgentLiveness::evaluate(None, now),
        AgentLiveness::Offline,
        "None should be Offline"
    );

    // Future timestamp / clock skew: now < ts
    assert_eq!(
        AgentLiveness::evaluate(Some(now + 100), now),
        AgentLiveness::Offline,
        "Future timestamp should be Offline"
    );
}

#[test]
fn test_heartbeat_corrupt_file_graceful_fallback() {
    let (_tmp, repo) = setup_repo();
    let manager = AgentManager::new(Arc::clone(&repo));
    manager
        .register("agent-glitch", AgentType::Ai, vec![])
        .unwrap();

    // 1. Initial heartbeat
    manager
        .heartbeat("agent-glitch", Some("mainline"), Some("ok"))
        .unwrap();
    assert_eq!(manager.get_liveness("agent-glitch"), AgentLiveness::Active);

    // 2. Corrupt heartbeat.json with arbitrary garbage
    let hb_path = repo.dft_dir().join("agents/agent-glitch/heartbeat.json");
    fs::write(&hb_path, b"{{INVALID_JSON_CORRUPTED_BYTES%%#@!").unwrap();

    // Must not panic, must fall back to Offline (or legacy fallback)
    let _liveness = manager.get_liveness("agent-glitch");
    // Since dual_update updated agent-glitch.json, if we also corrupt the legacy file:
    let legacy_path = repo.dft_dir().join("agents/agent-glitch.json");
    if legacy_path.exists() {
        fs::write(&legacy_path, b"GARBAGE").unwrap();
    }
    let liveness_after_full_corruption = manager.get_liveness("agent-glitch");
    assert_eq!(
        liveness_after_full_corruption,
        AgentLiveness::Offline,
        "Corrupted heartbeat files must fall back gracefully to Offline"
    );
}

#[test]
fn test_concurrent_heartbeats_multiple_threads() {
    let (_tmp, repo) = setup_repo();
    let manager = Arc::new(AgentManager::new(Arc::clone(&repo)));

    let num_threads = 8;
    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = Vec::new();

    for t in 0..num_threads {
        let mgr = Arc::clone(&manager);
        let bar = Arc::clone(&barrier);
        let agent_name = format!("worker-{}", t);
        mgr.register(&agent_name, AgentType::Ai, vec![]).unwrap();

        handles.push(thread::spawn(move || {
            bar.wait();
            for iter in 0..20 {
                let msg = format!("heartbeat iter {}", iter);
                let hb = mgr.heartbeat(&agent_name, Some("mainline"), Some(&msg));
                assert!(hb.is_ok(), "Heartbeat recording failed: {:?}", hb.err());
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Verify all agents are Active
    for t in 0..num_threads {
        let agent_name = format!("worker-{}", t);
        assert_eq!(
            manager.get_liveness(&agent_name),
            AgentLiveness::Active,
            "Agent {} should be active",
            agent_name
        );
    }
}

#[test]
fn test_concurrent_mailbox_messaging_stress() {
    let (_tmp, repo) = setup_repo();
    let manager = Arc::new(AgentManager::new(Arc::clone(&repo)));

    manager
        .register("central-receiver", AgentType::Ai, vec![])
        .unwrap();

    let num_senders = 6;
    let msgs_per_sender = 25;
    let barrier = Arc::new(Barrier::new(num_senders));
    let mut handles = Vec::new();

    for s in 0..num_senders {
        let mgr = Arc::clone(&manager);
        let bar = Arc::clone(&barrier);
        let sender_name = format!("sender-{}", s);
        mgr.register(&sender_name, AgentType::Ai, vec![]).unwrap();

        handles.push(thread::spawn(move || {
            bar.wait();
            for i in 0..msgs_per_sender {
                if i % 2 == 0 {
                    // Direct message
                    let body = format!("Direct from {} seq {}", sender_name, i);
                    let res =
                        mgr.send_message(&sender_name, "central-receiver", "direct-subject", &body);
                    assert!(res.is_ok(), "Direct send failed: {:?}", res.err());
                } else {
                    // Broadcast message
                    let body = format!("Broadcast from {} seq {}", sender_name, i);
                    let res = mgr.broadcast(&sender_name, "bcast-subject", &body);
                    assert!(res.is_ok(), "Broadcast failed: {:?}", res.err());
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Read all messages for central-receiver
    let inbox = manager.read_inbox(Some("central-receiver"), false).unwrap();
    let total_expected = num_senders * msgs_per_sender;

    println!(
        "Total inbox count for central-receiver: {} (expected: {})",
        inbox.len(),
        total_expected
    );

    // Verify Maildir tmp directory is completely drained
    let broadcast_tmp = repo.dft_dir().join("messages/broadcast/tmp");
    if broadcast_tmp.exists() {
        let tmp_entries: Vec<_> = fs::read_dir(broadcast_tmp).unwrap().flatten().collect();
        assert!(
            tmp_entries.is_empty(),
            "Maildir broadcast/tmp must be empty after writes, found: {:?}",
            tmp_entries
        );
    }

    let receiver_tmp = repo.dft_dir().join("messages/central-receiver/tmp");
    if receiver_tmp.exists() {
        let tmp_entries: Vec<_> = fs::read_dir(receiver_tmp).unwrap().flatten().collect();
        assert!(
            tmp_entries.is_empty(),
            "Maildir central-receiver/tmp must be empty after writes, found: {:?}",
            tmp_entries
        );
    }

    // Check message delivery count and uniqueness
    let mut message_ids = HashSet::new();
    let mut message_bodies = HashSet::new();
    for msg in &inbox {
        message_ids.insert(msg.id.clone());
        message_bodies.insert(msg.body.clone());
    }

    assert_eq!(
        message_bodies.len(),
        total_expected,
        "Message drop detected! Expected {} unique message bodies, got {}",
        total_expected,
        message_bodies.len()
    );
    assert_eq!(
        message_ids.len(),
        total_expected,
        "Message ID collision detected! Expected {} unique IDs, got {}",
        total_expected,
        message_ids.len()
    );
}

#[test]
fn test_maildir_atomic_transition_and_mark_read() {
    let (_tmp, repo) = setup_repo();
    let manager = AgentManager::new(Arc::clone(&repo));

    manager.register("alice", AgentType::Ai, vec![]).unwrap();
    manager.register("bob", AgentType::Ai, vec![]).unwrap();

    let mut msg_ids = Vec::new();
    for i in 0..10 {
        let msg = manager
            .send_message("alice", "bob", "ping", &format!("Ping {}", i))
            .unwrap();
        msg_ids.push(msg.id);
    }

    let bob_new_dir = repo.dft_dir().join("messages/bob/new");
    let bob_cur_dir = repo.dft_dir().join("messages/bob/cur");

    let new_count = fs::read_dir(&bob_new_dir).unwrap().flatten().count();
    let cur_count = fs::read_dir(&bob_cur_dir).unwrap().flatten().count();
    assert_eq!(
        new_count, 10,
        "All 10 messages should initially be in 'new/'"
    );
    assert_eq!(cur_count, 0, "No messages should initially be in 'cur/'");

    // Mark 4 messages as read
    for id in &msg_ids[0..4] {
        manager.mark_read(id, Some("bob")).unwrap();
    }

    let new_count_after = fs::read_dir(&bob_new_dir).unwrap().flatten().count();
    let cur_count_after = fs::read_dir(&bob_cur_dir).unwrap().flatten().count();
    assert_eq!(
        new_count_after, 6,
        "6 unread messages should remain in 'new/'"
    );
    assert_eq!(
        cur_count_after, 4,
        "4 read messages should be moved to 'cur/'"
    );

    // Query unread only
    let unread = manager.read_inbox(Some("bob"), true).unwrap();
    assert_eq!(unread.len(), 6, "read_inbox unread_only should return 6");

    // Query all
    let all = manager.read_inbox(Some("bob"), false).unwrap();
    assert_eq!(all.len(), 10, "read_inbox should return 10 total");
}

#[test]
fn test_mailbox_corrupt_message_immunity() {
    let (_tmp, repo) = setup_repo();
    let manager = AgentManager::new(Arc::clone(&repo));
    manager.register("receiver", AgentType::Ai, vec![]).unwrap();

    // Send one valid message
    manager
        .send_message("system", "receiver", "valid", "Valid message")
        .unwrap();

    // Inject corrupted JSON file directly into new/
    let corrupt_file = repo
        .dft_dir()
        .join("messages/receiver/new/corrupted_payload.json");
    fs::write(&corrupt_file, b"MALFORMED_JSON_CONTENT{{{[[[").unwrap();

    // Inject non-json file into new/
    let junk_file = repo.dft_dir().join("messages/receiver/new/junk.txt");
    fs::write(&junk_file, b"Plain text file").unwrap();

    // Reading inbox must not panic, must filter out corrupted entries
    let inbox = manager.read_inbox(Some("receiver"), false).unwrap();
    assert_eq!(inbox.len(), 1, "Should gracefully ignore corrupted files");
    assert_eq!(inbox[0].body, "Valid message");
}
