// Fin + TUI IO Adapter (bridges agent events to TUI)

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::io::agent_io::{AgentEvent, AgentIO};
use crate::llm::types::Message;

/// IO adapter that sends agent events to the TUI via a channel.
/// Supports bidirectional communication for interactive stages (define, etc.)
/// via an optional follow-up channel that blocks until the user responds.
pub struct TuiIO {
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    steering_rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<Message>>,
    /// When set, poll_follow_up() will async-wait for the next user message.
    /// This enables interactive multi-turn stages like define.
    follow_up_rx: Option<tokio::sync::Mutex<mpsc::UnboundedReceiver<String>>>,
}

impl TuiIO {
    pub fn new(
        event_tx: mpsc::UnboundedSender<AgentEvent>,
        steering_rx: mpsc::UnboundedReceiver<Message>,
    ) -> Self {
        Self {
            event_tx,
            steering_rx: tokio::sync::Mutex::new(steering_rx),
            follow_up_rx: None,
        }
    }

    /// Create a TuiIO that supports interactive follow-up prompts.
    /// The returned sender should receive user input from the TUI main loop.
    pub fn with_follow_up(
        event_tx: mpsc::UnboundedSender<AgentEvent>,
        steering_rx: mpsc::UnboundedReceiver<Message>,
        follow_up_rx: mpsc::UnboundedReceiver<String>,
    ) -> Self {
        Self {
            event_tx,
            steering_rx: tokio::sync::Mutex::new(steering_rx),
            follow_up_rx: Some(tokio::sync::Mutex::new(follow_up_rx)),
        }
    }
}

#[async_trait]
impl AgentIO for TuiIO {
    async fn emit(&self, event: AgentEvent) -> anyhow::Result<()> {
        self.event_tx.send(event).ok();
        Ok(())
    }

    async fn poll_steering(&self) -> Option<Message> {
        self.steering_rx.lock().await.try_recv().ok()
    }

    async fn poll_follow_up(&self) -> Option<Message> {
        // If we have a follow-up channel, block until the user types something.
        // This is what makes interactive stages (define) actually wait for answers.
        if let Some(ref rx) = self.follow_up_rx {
            let mut rx = rx.lock().await;
            if let Some(text) = rx.recv().await {
                return Some(Message::new_user(&text));
            }
        }
        None
    }

    async fn request_input(&self, prompt: &str) -> anyhow::Result<String> {
        // Emit the prompt as a system message so the user sees it
        self.event_tx
            .send(AgentEvent::TextDelta {
                text: format!("{prompt}\n"),
            })
            .ok();
        // If we have a follow-up channel, wait for user input
        if let Some(ref rx) = self.follow_up_rx {
            let mut rx = rx.lock().await;
            if let Some(text) = rx.recv().await {
                return Ok(text);
            }
        }
        Ok(String::new())
    }

    async fn request_confirmation(&self, _prompt: &str) -> anyhow::Result<bool> {
        Ok(true) // Auto-confirm in TUI for now
    }
}
