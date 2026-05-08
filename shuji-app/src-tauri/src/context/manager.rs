use std::path::PathBuf;
use tokio::fs;

use crate::models::message::Message;
use crate::models::role::Role;

pub struct ContextManager {
    contexts_dir: PathBuf,
}

impl ContextManager {
    pub fn new(shuji_root: &PathBuf) -> Self {
        Self {
            contexts_dir: shuji_root.join("contexts"),
        }
    }

    pub async fn load_context(&self, role: Role) -> anyhow::Result<Vec<Message>> {
        let path = self.contexts_dir.join(role.context_file());
        if !fs::try_exists(&path).await? {
            return Ok(vec![]);
        }
        let data = fs::read_to_string(&path).await?;
        let messages: Vec<Message> = serde_json::from_str(&data)?;
        Ok(messages)
    }

    pub async fn append_message(&self, role: Role, message: &Message) -> anyhow::Result<()> {
        let mut messages = self.load_context(role).await?;
        messages.push(message.clone());
        self.save_context(role, &messages).await
    }

    pub async fn save_context(&self, role: Role, messages: &[Message]) -> anyhow::Result<()> {
        fs::create_dir_all(&self.contexts_dir).await?;
        let path = self.contexts_dir.join(role.context_file());
        let data = serde_json::to_string_pretty(messages)?;
        fs::write(&path, &data).await?;
        Ok(())
    }

    pub async fn clear_context(&self, role: Role) -> anyhow::Result<()> {
        let path = self.contexts_dir.join(role.context_file());
        if fs::try_exists(&path).await? {
            fs::remove_file(&path).await?;
        }
        Ok(())
    }
}
