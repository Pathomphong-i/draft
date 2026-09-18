use crate::error::AgentError;
use crate::message::AgentMessage;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct LegacyInboxMessage {
    pub sender: String,
    pub content: String,
    pub timestamp: String,
}

pub struct MailboxManager {
    messages_dir: PathBuf,
    agents_dir: PathBuf,
}

impl MailboxManager {
    pub fn new(dft_dir: &Path) -> Self {
        Self {
            messages_dir: dft_dir.join("messages"),
            agents_dir: dft_dir.join("agents"),
        }
    }

    fn init_maildir(&self, box_name: &str) -> Result<PathBuf, AgentError> {
        let box_dir = self.messages_dir.join(box_name);
        fs::create_dir_all(box_dir.join("tmp"))?;
        fs::create_dir_all(box_dir.join("new"))?;
        fs::create_dir_all(box_dir.join("cur"))?;
        Ok(box_dir)
    }

    fn deliver_to_maildir(&self, box_name: &str, msg: &AgentMessage) -> Result<(), AgentError> {
        let box_dir = self.init_maildir(box_name)?;
        let tmp_path = box_dir.join("tmp").join(format!("{}.json", msg.id));
        let new_path = box_dir.join("new").join(format!("{}.json", msg.id));

        let json = serde_json::to_string_pretty(msg)?;
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        drop(file);

        fs::rename(tmp_path, new_path)?;
        Ok(())
    }

    fn dual_write_legacy_inbox(&self, sender: &str, content: &str) {
        let inbox_file = self.agents_dir.join("global_inbox.json");
        let mut messages: Vec<LegacyInboxMessage> = if inbox_file.exists() {
            fs::read_to_string(&inbox_file)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        messages.push(LegacyInboxMessage {
            sender: sender.to_string(),
            content: content.to_string(),
            timestamp: Utc::now().to_rfc3339(),
        });

        let _ = fs::create_dir_all(&self.agents_dir);
        let _ = fs::write(
            &inbox_file,
            serde_json::to_string_pretty(&messages).unwrap_or_default(),
        );
    }

    pub fn broadcast(
        &self,
        sender: &str,
        subject: &str,
        body: &str,
    ) -> Result<AgentMessage, AgentError> {
        let msg = AgentMessage::new_broadcast(sender, subject, body);
        self.deliver_to_maildir("broadcast", &msg)?;
        self.dual_write_legacy_inbox(sender, body);
        Ok(msg)
    }

    pub fn send(
        &self,
        sender: &str,
        recipient: &str,
        subject: &str,
        body: &str,
    ) -> Result<AgentMessage, AgentError> {
        let msg = AgentMessage::new_direct(sender, recipient, subject, body);
        self.deliver_to_maildir(recipient, &msg)?;
        Ok(msg)
    }

    fn read_dir_messages(&self, dir: &Path, is_cur: bool) -> Vec<AgentMessage> {
        let mut msgs = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(mut msg) = serde_json::from_str::<AgentMessage>(&content) {
                            msg.read = is_cur;
                            msgs.push(msg);
                        }
                    }
                }
            }
        }
        msgs
    }

    pub fn inbox(
        &self,
        recipient: Option<&str>,
        unread_only: bool,
    ) -> Result<Vec<AgentMessage>, AgentError> {
        let mut results = Vec::new();

        // 1. Read broadcast box
        let broadcast_dir = self.messages_dir.join("broadcast");
        if broadcast_dir.exists() {
            results.extend(self.read_dir_messages(&broadcast_dir.join("new"), false));
            if !unread_only {
                results.extend(self.read_dir_messages(&broadcast_dir.join("cur"), true));
            }
        }

        // 2. Read recipient-specific box if provided
        if let Some(agent) = recipient {
            let agent_box = self.messages_dir.join(agent);
            if agent_box.exists() {
                results.extend(self.read_dir_messages(&agent_box.join("new"), false));
                if !unread_only {
                    results.extend(self.read_dir_messages(&agent_box.join("cur"), true));
                }
            }
        }

        // 3. Fallback / legacy messages in global_inbox.json
        let legacy_file = self.agents_dir.join("global_inbox.json");
        if legacy_file.exists() {
            if let Ok(content) = fs::read_to_string(&legacy_file) {
                if let Ok(legacy_msgs) = serde_json::from_str::<Vec<LegacyInboxMessage>>(&content) {
                    for (i, lm) in legacy_msgs.into_iter().enumerate() {
                        let id = format!("legacy-{}", i);
                        // Avoid duplicate if already present
                        if !results
                            .iter()
                            .any(|m| m.body == lm.content && m.sender == lm.sender)
                        {
                            results.push(AgentMessage {
                                id,
                                sender: lm.sender,
                                recipient: None,
                                timestamp: Utc::now().timestamp() as u64,
                                timestamp_iso: lm.timestamp,
                                subject: "broadcast".to_string(),
                                body: lm.content,
                                read: false,
                                dimension: None,
                                in_reply_to: None,
                            });
                        }
                    }
                }
            }
        }

        if unread_only {
            results.retain(|m| !m.read);
        }

        // Sort chronologically ascending
        results.sort_by(|a, b| a.timestamp_iso.cmp(&b.timestamp_iso));

        Ok(results)
    }

    pub fn mark_read(&self, message_id: &str, recipient: Option<&str>) -> Result<(), AgentError> {
        let boxes = match recipient {
            Some(r) => vec![r.to_string(), "broadcast".to_string()],
            None => vec!["broadcast".to_string()],
        };

        for b in boxes {
            let new_path = self
                .messages_dir
                .join(&b)
                .join("new")
                .join(format!("{}.json", message_id));
            let cur_path = self
                .messages_dir
                .join(&b)
                .join("cur")
                .join(format!("{}.json", message_id));

            if new_path.exists() {
                if let Ok(content) = fs::read_to_string(&new_path) {
                    if let Ok(mut msg) = serde_json::from_str::<AgentMessage>(&content) {
                        msg.read = true;
                        let _ = fs::write(&new_path, serde_json::to_string_pretty(&msg)?);
                        let _ = fs::rename(new_path, cur_path);
                        return Ok(());
                    }
                }
            }
        }

        Ok(())
    }
}
