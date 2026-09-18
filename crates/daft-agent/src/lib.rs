//! Multi-agent identity registry, real-time activity status, heartbeats, and Maildir messaging for Daft VCS.

pub mod error;
pub mod heartbeat;
pub mod identity;
pub mod mailbox;
pub mod message;
pub mod registry;
pub mod status;

pub use error::AgentError;
pub use heartbeat::{AgentLiveness, HeartbeatRecord, HeartbeatSubsystem};
pub use identity::{AgentIdentity, AgentType};
pub use mailbox::MailboxManager;
pub use message::AgentMessage;
pub use registry::{AgentRegistry, RegistryStore};
pub use status::{AgentActivity, StatusAggregator, SwarmStatusReport};

use daft_core::Repository;
use std::path::PathBuf;
use std::sync::Arc;

pub struct AgentManager {
    #[allow(dead_code)]
    repo: Arc<Repository>,
    #[allow(dead_code)]
    dft_dir: PathBuf,
    registry: AgentRegistry,
    heartbeats: HeartbeatSubsystem,
    mailbox: MailboxManager,
    status_agg: StatusAggregator,
}

impl AgentManager {
    pub fn new(repo: Arc<Repository>) -> Self {
        let dft_dir = repo.dft_dir().to_path_buf();
        let registry = AgentRegistry::new(&dft_dir);
        let heartbeats = HeartbeatSubsystem::new(&dft_dir);
        let mailbox = MailboxManager::new(&dft_dir);
        let status_agg = StatusAggregator::new(Arc::clone(&repo));

        Self {
            repo,
            dft_dir,
            registry,
            heartbeats,
            mailbox,
            status_agg,
        }
    }

    // --- Registry Operations ---
    pub fn register(
        &self,
        name: &str,
        agent_type: AgentType,
        capabilities: Vec<String>,
    ) -> Result<AgentIdentity, AgentError> {
        self.registry.register(name, agent_type, capabilities)
    }

    pub fn list(&self) -> Result<Vec<AgentIdentity>, AgentError> {
        self.registry.list()
    }

    pub fn assign(&self, agent_name: &str, dimension: &str) -> Result<AgentIdentity, AgentError> {
        self.registry.assign(agent_name, dimension)
    }

    pub fn get(&self, name_or_id: &str) -> Result<AgentIdentity, AgentError> {
        self.registry.get(name_or_id)
    }

    // --- Status & Real-Time Activity ---
    pub fn status(&self, agent_name: Option<&str>) -> Result<SwarmStatusReport, AgentError> {
        self.status_agg.aggregate(agent_name)
    }

    // --- Heartbeat & Liveness ---
    pub fn heartbeat(
        &self,
        agent_name: &str,
        dimension: Option<&str>,
        message: Option<&str>,
    ) -> Result<HeartbeatRecord, AgentError> {
        self.heartbeats.record(agent_name, dimension, message)
    }

    pub fn get_liveness(&self, agent_name: &str) -> AgentLiveness {
        self.heartbeats.liveness(agent_name)
    }

    pub fn get_active_agents(&self) -> Result<Vec<AgentIdentity>, AgentError> {
        let all = self.registry.list()?;
        Ok(all
            .into_iter()
            .filter(|a| self.heartbeats.liveness(&a.name) == AgentLiveness::Active)
            .collect())
    }

    // --- Inter-Agent Messaging ---
    pub fn broadcast(
        &self,
        sender: &str,
        subject: &str,
        body: &str,
    ) -> Result<AgentMessage, AgentError> {
        self.mailbox.broadcast(sender, subject, body)
    }

    pub fn send_message(
        &self,
        sender: &str,
        recipient: &str,
        subject: &str,
        body: &str,
    ) -> Result<AgentMessage, AgentError> {
        self.mailbox.send(sender, recipient, subject, body)
    }

    pub fn read_inbox(
        &self,
        recipient: Option<&str>,
        unread_only: bool,
    ) -> Result<Vec<AgentMessage>, AgentError> {
        self.mailbox.inbox(recipient, unread_only)
    }

    pub fn mark_read(&self, message_id: &str, recipient: Option<&str>) -> Result<(), AgentError> {
        self.mailbox.mark_read(message_id, recipient)
    }
}
