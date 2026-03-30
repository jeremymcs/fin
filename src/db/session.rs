// Fin — Session Store (JSONL File-Based)
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use crate::llm::types::Message;
use std::path::{Path, PathBuf};

/// Current session file format version.
/// Bump when changing the JSONL schema to enable migration.
const SESSION_FORMAT_VERSION: u32 = 1;

/// File-based session persistence using JSONL format.
/// First line is a version header: {"fin_session_version": N}
/// Subsequent lines are serialized Message objects.
pub struct SessionStore {
    sessions_dir: PathBuf,
}

impl SessionStore {
    pub fn new(sessions_dir: &Path) -> anyhow::Result<Self> {
        std::fs::create_dir_all(sessions_dir)?;
        Ok(Self {
            sessions_dir: sessions_dir.to_path_buf(),
        })
    }

    /// Append a message to a session file.
    /// Creates the file with a version header if it doesn't exist.
    pub fn append(&self, session_id: &str, message: &Message) -> anyhow::Result<()> {
        let path = self.session_path(session_id);
        let is_new = !path.exists();
        let line = serde_json::to_string(message)?;

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        // Write version header for new sessions
        if is_new {
            writeln!(
                file,
                "{{\"fin_session_version\":{}}}",
                SESSION_FORMAT_VERSION
            )?;
        }

        writeln!(file, "{line}")?;
        Ok(())
    }

    /// Load all messages from a session.
    /// Skips the version header line and any lines that aren't valid messages
    /// (backwards-compatible with pre-versioned session files).
    pub fn load(&self, session_id: &str) -> anyhow::Result<Vec<Message>> {
        let path = self.session_path(session_id);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(path)?;
        let messages: Vec<Message> = content
            .lines()
            .filter(|l| !l.is_empty())
            // Skip version header lines (they don't deserialize as Message)
            .filter(|l| !l.contains("\"fin_session_version\""))
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();

        Ok(messages)
    }

    /// List available sessions.
    pub fn list(&self) -> anyhow::Result<Vec<SessionInfo>> {
        let mut sessions = Vec::new();
        for entry in std::fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "jsonl") {
                let id = path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                let metadata = entry.metadata()?;
                sessions.push(SessionInfo {
                    id,
                    modified: metadata.modified()?,
                    size: metadata.len(),
                });
            }
        }
        sessions.sort_by(|a, b| b.modified.cmp(&a.modified));
        Ok(sessions)
    }

    fn session_path(&self, session_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{session_id}.jsonl"))
    }
}

pub struct SessionInfo {
    pub id: String,
    pub modified: std::time::SystemTime,
    pub size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::{Content, Message, Role};

    #[test]
    fn test_append_and_load_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SessionStore::new(tmp.path()).unwrap();

        let msg1 = Message::new_user("Hello");
        let mut msg2 = Message::new_assistant();
        msg2.content.push(Content::Text {
            text: "Hi there!".into(),
        });

        store.append("test-session", &msg1).unwrap();
        store.append("test-session", &msg2).unwrap();

        let loaded = store.load("test-session").unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].role, Role::User);
        assert_eq!(loaded[1].role, Role::Assistant);

        // Check content
        if let Content::Text { text } = &loaded[0].content[0] {
            assert_eq!(text, "Hello");
        } else {
            panic!("Expected text content");
        }
    }

    #[test]
    fn test_load_nonexistent_session() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SessionStore::new(tmp.path()).unwrap();

        let loaded = store.load("nonexistent").unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_list_sessions() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SessionStore::new(tmp.path()).unwrap();

        // No sessions
        let sessions = store.list().unwrap();
        assert!(sessions.is_empty());

        // Create two sessions
        store.append("session-a", &Message::new_user("a")).unwrap();
        store.append("session-b", &Message::new_user("b")).unwrap();

        let sessions = store.list().unwrap();
        assert_eq!(sessions.len(), 2);

        // Should be sorted by modification time (newest first)
        // Both are created almost simultaneously, so just check count
        let ids: Vec<&str> = sessions.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"session-a"));
        assert!(ids.contains(&"session-b"));
    }

    #[test]
    fn test_append_multiple_messages() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SessionStore::new(tmp.path()).unwrap();

        for i in 0..10 {
            store
                .append("bulk", &Message::new_user(&format!("msg {i}")))
                .unwrap();
        }

        let loaded = store.load("bulk").unwrap();
        assert_eq!(loaded.len(), 10);
    }
}
