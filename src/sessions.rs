// Fin + Session Management CLI Handlers

use crate::cli::SessionAction;
use crate::config::paths::FinPaths;
use crate::db::session::SessionStore;

/// Handle `fin sessions [list|resume <id>]` commands.
pub async fn handle_sessions(action: Option<SessionAction>) -> anyhow::Result<()> {
    let paths = FinPaths::resolve()?;
    let store = SessionStore::new(&paths.sessions_dir)?;

    match action {
        None | Some(SessionAction::List) => {
            let sessions = store.list()?;
            if sessions.is_empty() {
                println!("No sessions found.");
                return Ok(());
            }
            println!("{:<38}  {:>10}  LAST MODIFIED", "SESSION ID", "SIZE");
            println!("{}", "-".repeat(70));
            for s in &sessions {
                let modified = humanize_time(s.modified);
                let size = humanize_bytes(s.size);
                println!("{:<38}  {:>10}  {}", s.id, size, modified);
            }
            println!(
                "\n{} session(s). Resume with: fin sessions resume <id>",
                sessions.len()
            );
            Ok(())
        }
        Some(SessionAction::Resume { id }) => {
            let messages = store.load(&id)?;
            if messages.is_empty() {
                anyhow::bail!("Session '{id}' not found or empty.");
            }
            println!("Resuming session {id} ({} messages)...", messages.len());
            // Run interactive mode with resumed session
            crate::io::print::run_with_session(&id, messages).await
        }
    }
}

fn humanize_time(time: std::time::SystemTime) -> String {
    let elapsed = time.elapsed().unwrap_or_default();
    let secs = elapsed.as_secs();
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}

fn humanize_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
