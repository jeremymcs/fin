// Fin — Worktree Management
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use crate::cli::WorktreeAction;
use std::path::{Path, PathBuf};

/// Handle `fin worktree <action>` commands.
pub async fn handle_worktree(action: WorktreeAction) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;

    match action {
        WorktreeAction::List => cmd_list(&cwd).await,
        WorktreeAction::Create { name } => cmd_create(&cwd, &name).await,
        WorktreeAction::Merge { name } => cmd_merge(&cwd, &name).await,
        WorktreeAction::Remove { name } => cmd_remove(&cwd, &name).await,
        WorktreeAction::Clean => cmd_clean(&cwd).await,
    }
}

async fn cmd_list(cwd: &Path) -> anyhow::Result<()> {
    let output = run_git(cwd, &["worktree", "list", "--porcelain"]).await?;

    if output.trim().is_empty() {
        println!("No worktrees found.");
        return Ok(());
    }

    // Parse porcelain output into readable format
    let mut current_path = String::new();
    let mut current_branch = String::new();

    println!("{:<50} BRANCH", "PATH");
    println!("{}", "-".repeat(70));

    for line in output.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_path = path.to_string();
        } else if let Some(branch) = line.strip_prefix("branch ") {
            current_branch = branch.replace("refs/heads/", "");
        } else if line.is_empty() && !current_path.is_empty() {
            let display_branch = if current_branch.is_empty() {
                "(detached)".to_string()
            } else {
                current_branch.clone()
            };
            println!("{:<50} {}", current_path, display_branch);
            current_path.clear();
            current_branch.clear();
        }
    }
    // Print last entry if no trailing newline
    if !current_path.is_empty() {
        let display_branch = if current_branch.is_empty() {
            "(detached)".to_string()
        } else {
            current_branch
        };
        println!("{:<50} {}", current_path, display_branch);
    }

    Ok(())
}

async fn cmd_create(cwd: &Path, name: &str) -> anyhow::Result<()> {
    let worktree_path = worktree_dir(cwd, name);
    let branch_name = format!("fin/worktree/{name}");

    // Create the worktree with a new branch
    run_git(
        cwd,
        &[
            "worktree",
            "add",
            &worktree_path.to_string_lossy(),
            "-b",
            &branch_name,
        ],
    )
    .await?;

    println!("Created worktree: {}", worktree_path.display());
    println!("  Branch: {branch_name}");
    println!("  cd {} to work in it.", worktree_path.display());

    Ok(())
}

async fn cmd_merge(cwd: &Path, name: &str) -> anyhow::Result<()> {
    let worktree_path = worktree_dir(cwd, name);
    let branch_name = format!("fin/worktree/{name}");

    if !worktree_path.exists() {
        anyhow::bail!(
            "Worktree '{}' not found at {}",
            name,
            worktree_path.display()
        );
    }

    // Detect main branch
    let main_branch = detect_main_branch(cwd).await;

    // Check for uncommitted changes in the worktree
    let status = run_git(&worktree_path, &["status", "--porcelain"]).await?;
    if !status.trim().is_empty() {
        anyhow::bail!("Worktree '{name}' has uncommitted changes. Commit or stash them first.");
    }

    // Squash merge from the worktree branch into main
    run_git(cwd, &["checkout", &main_branch]).await?;
    run_git(cwd, &["merge", "--squash", &branch_name]).await?;
    run_git(
        cwd,
        &["commit", "-m", &format!("feat(worktree): merge {name}")],
    )
    .await?;

    // Remove the worktree and branch
    run_git(
        cwd,
        &["worktree", "remove", &worktree_path.to_string_lossy()],
    )
    .await?;
    let _ = run_git(cwd, &["branch", "-D", &branch_name]).await;

    println!("Merged worktree '{name}' into {main_branch} and cleaned up.");

    Ok(())
}

async fn cmd_remove(cwd: &Path, name: &str) -> anyhow::Result<()> {
    let worktree_path = worktree_dir(cwd, name);
    let branch_name = format!("fin/worktree/{name}");

    if !worktree_path.exists() {
        anyhow::bail!(
            "Worktree '{}' not found at {}",
            name,
            worktree_path.display()
        );
    }

    run_git(
        cwd,
        &[
            "worktree",
            "remove",
            "--force",
            &worktree_path.to_string_lossy(),
        ],
    )
    .await?;
    let _ = run_git(cwd, &["branch", "-D", &branch_name]).await;

    println!("Removed worktree '{name}'.");

    Ok(())
}

async fn cmd_clean(cwd: &Path) -> anyhow::Result<()> {
    // Prune stale worktrees (ones whose directories no longer exist)
    run_git(cwd, &["worktree", "prune"]).await?;
    println!("Pruned stale worktrees.");
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────

fn worktree_dir(cwd: &Path, name: &str) -> PathBuf {
    cwd.join(".fin-worktrees").join(name)
}

async fn detect_main_branch(cwd: &Path) -> String {
    if run_git(cwd, &["rev-parse", "--verify", "main"])
        .await
        .is_ok()
    {
        "main".to_string()
    } else {
        "master".to_string()
    }
}

async fn run_git(cwd: &Path, args: &[&str]) -> anyhow::Result<String> {
    let output = tokio::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .await?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            stderr.trim()
        );
    }
}
