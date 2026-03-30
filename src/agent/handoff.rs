// Fin — Context Handoff (Fresh Memory After /clear)
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use crate::llm::types::{Content, Message, Role};

/// Build a concise handoff from conversation history.
/// Used when /clear is called outside of a .fin/ workflow project.
///
/// This extracts the essential context — what was asked, what was done,
/// what files were touched, what decisions were made — so the agent can
/// continue with a fresh context window and zero token rot.
pub fn build_handoff(messages: &[Message]) -> String {
    let mut handoff = String::from("# Context Handoff\n\n");

    // Extract the original task (first user message)
    if let Some(first_user) = messages.iter().find(|m| m.role == Role::User) {
        if let Some(text) = extract_text(first_user) {
            let truncated = truncate(text, 500);
            handoff.push_str(&format!("## Original Task\n\n{truncated}\n\n"));
        }
    }

    // Collect files that were read/written/edited
    let mut files_read: Vec<String> = Vec::new();
    let mut files_modified: Vec<String> = Vec::new();
    let mut commands_run: Vec<String> = Vec::new();

    for msg in messages {
        for content in &msg.content {
            if let Content::ToolCall(tc) = content {
                match tc.name.as_str() {
                    "read" => {
                        if let Some(path) = tc.arguments.get("file_path").and_then(|v| v.as_str()) {
                            if !files_read.contains(&path.to_string()) {
                                files_read.push(path.to_string());
                            }
                        }
                    }
                    "write" | "edit" => {
                        if let Some(path) = tc.arguments.get("file_path").and_then(|v| v.as_str()) {
                            if !files_modified.contains(&path.to_string()) {
                                files_modified.push(path.to_string());
                            }
                        }
                    }
                    "bash" => {
                        if let Some(cmd) = tc.arguments.get("command").and_then(|v| v.as_str()) {
                            let short = truncate(cmd, 120);
                            commands_run.push(short.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if !files_modified.is_empty() {
        handoff.push_str("## Files Modified\n\n");
        for f in &files_modified {
            handoff.push_str(&format!("- {f}\n"));
        }
        handoff.push('\n');
    }

    if !files_read.is_empty() {
        // Only show files read that weren't also modified (reduce noise)
        let read_only: Vec<&String> = files_read
            .iter()
            .filter(|f| !files_modified.contains(f))
            .collect();
        if !read_only.is_empty() {
            handoff.push_str("## Files Read\n\n");
            for f in &read_only {
                handoff.push_str(&format!("- {f}\n"));
            }
            handoff.push('\n');
        }
    }

    if !commands_run.is_empty() {
        handoff.push_str("## Commands Run\n\n");
        // Only show last 10 commands to keep it tight
        let start = commands_run.len().saturating_sub(10);
        for cmd in &commands_run[start..] {
            handoff.push_str(&format!("- `{cmd}`\n"));
        }
        handoff.push('\n');
    }

    // Extract key decisions/results from assistant messages
    let mut key_outputs: Vec<String> = Vec::new();
    for msg in messages.iter().rev() {
        if msg.role == Role::Assistant {
            if let Some(text) = extract_text(msg) {
                if !text.is_empty() {
                    key_outputs.push(truncate(text, 300).to_string());
                    if key_outputs.len() >= 3 {
                        break;
                    }
                }
            }
        }
    }

    if !key_outputs.is_empty() {
        handoff.push_str("## Recent Assistant Output (newest first)\n\n");
        for output in &key_outputs {
            handoff.push_str(&format!("{output}\n\n---\n\n"));
        }
    }

    // Most recent user message (what was being worked on)
    if let Some(last_user) = messages.iter().rev().find(|m| m.role == Role::User) {
        if let Some(text) = extract_text(last_user) {
            let truncated = truncate(text, 300);
            handoff.push_str(&format!("## Last User Request\n\n{truncated}\n\n"));
        }
    }

    // Token estimate
    let token_est = crate::agent::compaction::estimate_tokens_rough(messages);
    let msg_count = messages.len();
    handoff.push_str(&format!(
        "## Session Stats\n\n- Messages: {msg_count}\n- Est. tokens: ~{token_est}\n"
    ));

    handoff
}

fn extract_text(msg: &Message) -> Option<&str> {
    msg.content.iter().find_map(|c| match c {
        Content::Text { text } if !text.is_empty() => Some(text.as_str()),
        _ => None,
    })
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        // Find a safe char boundary
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::types::{Content, Message, ToolCall};

    fn user_msg(text: &str) -> Message {
        Message::new_user(text)
    }

    fn assistant_msg(text: &str) -> Message {
        let mut m = Message::new_assistant();
        m.content.push(Content::Text {
            text: text.to_string(),
        });
        m
    }

    fn tool_call_msg(name: &str, args: serde_json::Value) -> Message {
        let mut m = Message::new_assistant();
        m.content.push(Content::ToolCall(ToolCall {
            id: "tc_1".into(),
            name: name.into(),
            arguments: args,
        }));
        m
    }

    #[test]
    fn test_handoff_includes_original_task() {
        let msgs = vec![
            user_msg("Build an auth system"),
            assistant_msg("Sure, starting."),
        ];
        let handoff = build_handoff(&msgs);
        assert!(handoff.contains("Build an auth system"));
        assert!(handoff.contains("Original Task"));
    }

    #[test]
    fn test_handoff_includes_files_modified() {
        let msgs = vec![
            user_msg("Fix the bug"),
            tool_call_msg("edit", serde_json::json!({"file_path": "/src/main.rs"})),
            assistant_msg("Fixed."),
        ];
        let handoff = build_handoff(&msgs);
        assert!(handoff.contains("/src/main.rs"));
        assert!(handoff.contains("Files Modified"));
    }

    #[test]
    fn test_handoff_empty_messages() {
        let handoff = build_handoff(&[]);
        assert!(handoff.contains("Context Handoff"));
        assert!(handoff.contains("Messages: 0"));
    }

    #[test]
    fn test_truncate_ascii() {
        assert_eq!(truncate("hello world", 5), "hello");
        assert_eq!(truncate("hi", 10), "hi");
    }
}
