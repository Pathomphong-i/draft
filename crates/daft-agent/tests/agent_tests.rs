use daft_agent::{AgentLiveness, AgentManager, AgentType};
use daft_core::init::{init, InitOptions};
use daft_core::Repository;
use daft_dimension::DimensionManager;
use std::fs;
use std::sync::Arc;
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
fn test_agent_register_and_list() {
    let (_tmp, repo) = setup_repo();
    let manager = AgentManager::new(Arc::clone(&repo));

    let a1 = manager
        .register("agent-alice", AgentType::Ai, vec!["coding".into()])
        .unwrap();
    assert_eq!(a1.name, "agent-alice");
    assert_eq!(a1.agent_type, AgentType::Ai);
    assert_eq!(a1.assigned_dimension.as_deref(), Some("mainline"));

    let a2 = manager
        .register("human-bob", AgentType::Human, vec!["review".into()])
        .unwrap();
    assert_eq!(a2.name, "human-bob");
    assert_eq!(a2.agent_type, AgentType::Human);

    let list = manager.list().unwrap();
    assert_eq!(list.len(), 2);

    // Duplicate name fails
    let dup = manager.register("agent-alice", AgentType::Ai, vec![]);
    assert!(dup.is_err());
}

#[test]
fn test_agent_assign_dimension() {
    let (_tmp, repo) = setup_repo();
    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr
        .create_dimension("feature-auth", None, None)
        .unwrap();

    let manager = AgentManager::new(Arc::clone(&repo));
    manager
        .register("agent-auth", AgentType::Ai, vec![])
        .unwrap();

    // Assign to valid dimension
    let assigned = manager.assign("agent-auth", "feature-auth").unwrap();
    assert_eq!(assigned.assigned_dimension.as_deref(), Some("feature-auth"));

    // Assign to non-existent dimension fails
    let bad_assign = manager.assign("agent-auth", "non-existent-dim");
    assert!(bad_assign.is_err());
}

#[test]
fn test_agent_heartbeat_and_liveness() {
    let (_tmp, repo) = setup_repo();
    let manager = AgentManager::new(Arc::clone(&repo));
    manager
        .register("worker-node", AgentType::Ai, vec![])
        .unwrap();

    let hb = manager
        .heartbeat("worker-node", Some("mainline"), Some("ready"))
        .unwrap();
    assert_eq!(hb.agent_name, "worker-node");

    let liveness = manager.get_liveness("worker-node");
    assert_eq!(liveness, AgentLiveness::Active);

    // Simulated time jumps
    let now = hb.timestamp;
    assert_eq!(
        AgentLiveness::evaluate(Some(now), now + 30),
        AgentLiveness::Active
    );
    assert_eq!(
        AgentLiveness::evaluate(Some(now), now + 120),
        AgentLiveness::Idle
    );
    assert_eq!(
        AgentLiveness::evaluate(Some(now), now + 600),
        AgentLiveness::Offline
    );
}

#[test]
fn test_agent_mailbox_messaging() {
    let (_tmp, repo) = setup_repo();
    let manager = AgentManager::new(Arc::clone(&repo));
    manager
        .register("sender-bot", AgentType::Ai, vec![])
        .unwrap();
    manager
        .register("receiver-bot", AgentType::Ai, vec![])
        .unwrap();

    // Broadcast
    let bmsg = manager
        .broadcast("sender-bot", "sync", "Broadcast announcement")
        .unwrap();
    assert_eq!(bmsg.sender, "sender-bot");
    assert_eq!(bmsg.body, "Broadcast announcement");

    // Direct message
    let dmsg = manager
        .send_message("sender-bot", "receiver-bot", "task", "Implement feature X")
        .unwrap();
    assert_eq!(dmsg.recipient.as_deref(), Some("receiver-bot"));

    // Receiver inbox should see both direct and broadcast messages
    let inbox = manager.read_inbox(Some("receiver-bot"), false).unwrap();
    assert!(inbox.len() >= 2);
    assert!(inbox.iter().any(|m| m.body == "Broadcast announcement"));
    assert!(inbox.iter().any(|m| m.body == "Implement feature X"));

    // Mark direct message as read
    manager.mark_read(&dmsg.id, Some("receiver-bot")).unwrap();

    // Check unread only
    let unread = manager.read_inbox(Some("receiver-bot"), true).unwrap();
    assert!(!unread.iter().any(|m| m.id == dmsg.id));
}

#[test]
fn test_agent_status_aggregation() {
    let (_tmp, repo) = setup_repo();
    let manager = AgentManager::new(Arc::clone(&repo));
    manager.register("bot-1", AgentType::Ai, vec![]).unwrap();

    let status = manager.status(None).unwrap();
    assert_eq!(status.total_agents, 1);
    assert_eq!(status.agents[0].name, "bot-1");
}

#[test]
fn test_agent_status_staged_files() {
    let (_tmp, repo) = setup_repo();
    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    dim_mgr.create_dimension("dim-dev", None, None).unwrap();

    let dev_repo =
        daft_dimension::DimensionRepository::for_dimension(Arc::clone(&repo), "dim-dev").unwrap();
    let mut index = dev_repo.index().unwrap();

    // Stage a file in index
    let blob_oid = repo.cas().write_blob(b"staged content").unwrap();
    index.add_entry(
        daft_core::index::IndexEntry::new(
            "src/lib.rs",
            blob_oid,
            0o100644,
            daft_core::index::Stage::Normal,
            14,
        )
        .unwrap(),
    );
    dev_repo.write_index(&index).unwrap();

    let manager = AgentManager::new(Arc::clone(&repo));
    manager
        .register("agent-dev", AgentType::Ai, vec![])
        .unwrap();
    manager.assign("agent-dev", "dim-dev").unwrap();

    let status = manager.status(Some("agent-dev")).unwrap();
    assert_eq!(status.agents.len(), 1);
    assert_eq!(
        status.agents[0].staged_files,
        vec!["src/lib.rs".to_string()]
    );
    assert_eq!(status.agents[0].total_dirty_files, 2); // 1 staged + 1 unstaged deleted (since not yet on disk)
}
