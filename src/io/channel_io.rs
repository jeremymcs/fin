// Fin + Channel IO Adapter (for HTTP SSE and other channel-based consumers)

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::io::agent_io::{AgentEvent, AgentIO};
use crate::llm::types::Message;

/// IO adapter that sends events to a channel. Used by HTTP SSE and similar.
pub struct ChannelIO {
    tx: mpsc::UnboundedSender<AgentEvent>,
}

impl ChannelIO {
    pub fn new(tx: mpsc::UnboundedSender<AgentEvent>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl AgentIO for ChannelIO {
    async fn emit(&self, event: AgentEvent) -> anyhow::Result<()> {
        self.tx.send(event).ok();
        Ok(())
    }

    async fn poll_steering(&self) -> Option<Message> {
        None
    }
    async fn poll_follow_up(&self) -> Option<Message> {
        None
    }
    async fn request_input(&self, _: &str) -> anyhow::Result<String> {
        Ok(String::new())
    }
    async fn request_confirmation(&self, _: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
}
