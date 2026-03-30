// Fin — Sub-Agent Output Collector IO
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::io::agent_io::{AgentEvent, AgentIO};
use crate::llm::types::{Message, Usage};

/// IO adapter that captures sub-agent output via channel.
pub struct CollectorIO {
    tx: mpsc::UnboundedSender<AgentEvent>,
}

/// Receiver side — drains events into final output.
pub struct CollectorReceiver {
    rx: mpsc::UnboundedReceiver<AgentEvent>,
}

/// Create a paired collector IO and receiver.
pub fn collector_pair() -> (CollectorIO, CollectorReceiver) {
    let (tx, rx) = mpsc::unbounded_channel();
    (CollectorIO { tx }, CollectorReceiver { rx })
}

impl CollectorReceiver {
    /// Drain all events and return concatenated text output + usage.
    pub async fn collect(mut self) -> (String, Usage) {
        let mut text = String::new();
        let mut usage = Usage::default();

        while let Some(event) = self.rx.recv().await {
            match event {
                AgentEvent::TextDelta { text: delta } => {
                    text.push_str(&delta);
                }
                AgentEvent::AgentEnd { usage: u } => {
                    usage = u;
                }
                // Ignore other events for collection
                _ => {}
            }
        }

        (text, usage)
    }
}

#[async_trait]
impl AgentIO for CollectorIO {
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

    async fn request_input(&self, _prompt: &str) -> anyhow::Result<String> {
        Ok(String::new())
    }

    async fn request_confirmation(&self, _prompt: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
}
