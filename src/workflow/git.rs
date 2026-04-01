// Fin — Workflow Git Automation
// Copyright (c) 2026 Jeremy McSpadden <jeremy@fluxlabs.net>

use anyhow::Context;
use std::path::{Path, PathBuf};

pub struct WorkflowGit {
    cwd: PathBuf,
}

impl WorkflowGit {
    pub fn new(cwd: &Path) -> Self {
        Self {
            cwd: cwd.to_path_buf(),
        }
    }

    // ── Branch management ───────────────────────────────────────────────

    /// Create and checkout a section branch: fin/<b_id>/<s_id>
    pub async fn create_section_branch(&self, b_id: &str, s_id: &str) -> anyhow::Result<()> {
        let branch = format!("fin/{}/{}", b_id, s_id);
        self.run_git(&["checkout", "-b", &branch])
            .await
            .with_context(|| format!("failed to create section branch: {}", branch))?;
        Ok(())
    }

    /// Create and checkout a fix branch: fin/<b_id>/<s_id>-fix
    pub async fn create_fix_branch(&self, b_id: &str, s_id: &str) -> anyhow::Result<()> {
        let branch = format!("fin/{}/{}-fix", b_id, s_id);
        self.run_git(&["checkout", "-b", &branch])
            .await
            .with_context(|| format!("failed to create fix branch: {}", branch))?;
        Ok(())
    }

    /// Returns the name of the current branch.
    pub async fn current_branch(&self) -> anyhow::Result<String> {
        let output = self
            .run_git(&["branch", "--show-current"])
            .await
            .context("failed to get current branch")?;
        Ok(output.trim().to_string())
    }

    /// Switch to an existing branch.
    pub async fn switch_branch(&self, branch: &str) -> anyhow::Result<()> {
        self.run_git(&["checkout", branch])
            .await
            .with_context(|| format!("failed to switch to branch: {}", branch))?;
        Ok(())
    }

    /// Detect the main branch name.
    /// Uses preference if set, otherwise tries "main", falls back to "master".
    pub async fn main_branch(&self) -> String {
        // Check preference first
        let cwd = &self.cwd;
        let prefs = crate::config::preferences::Preferences::resolve(cwd);
        if let Some(ref branch) = prefs.git.main_branch {
            return branch.clone();
        }
        // Auto-detect: check if "main" exists as a local branch
        if self
            .run_git(&["rev-parse", "--verify", "main"])
            .await
            .is_ok()
        {
            return "main".to_string();
        }
        "master".to_string()
    }

    // ── Conventional commits ────────────────────────────────────────────

    /// Stage all changes and create a conventional commit for a task.
    /// Returns the commit SHA.
    pub async fn commit_task(
        &self,
        s_id: &str,
        t_id: &str,
        commit_type: &str,
        one_liner: &str,
    ) -> anyhow::Result<String> {
        self.run_git(&["add", "-A"])
            .await
            .context("failed to stage changes")?;

        let message = format!("{}({}/{}): {}", commit_type, s_id, t_id, one_liner);
        self.run_git(&["commit", "-m", &message])
            .await
            .context("failed to commit task")?;

        let sha = self
            .run_git(&["rev-parse", "HEAD"])
            .await
            .context("failed to get commit SHA")?;
        Ok(sha.trim().to_string())
    }

    /// Stage specific paths and create a docs commit.
    /// Returns the commit SHA.
    pub async fn commit_artifacts(
        &self,
        scope: &str,
        message: &str,
        paths: &[PathBuf],
    ) -> anyhow::Result<String> {
        for path in paths {
            let path_str = path.to_string_lossy();
            self.run_git(&["add", &path_str])
                .await
                .with_context(|| format!("failed to stage: {}", path_str))?;
        }

        let commit_msg = format!("docs({}): {}", scope, message);
        self.run_git(&["commit", "-m", &commit_msg])
            .await
            .context("failed to commit artifacts")?;

        let sha = self
            .run_git(&["rev-parse", "HEAD"])
            .await
            .context("failed to get commit SHA")?;
        Ok(sha.trim().to_string())
    }

    // ── Squash merge ────────────────────────────────────────────────────

    /// Squash-merge a section branch back to main, then delete the branch.
    pub async fn squash_merge_section(
        &self,
        b_id: &str,
        s_id: &str,
        section_title: &str,
        task_summaries: &[String],
    ) -> anyhow::Result<()> {
        let main = self.main_branch().await;
        let branch = format!("fin/{}/{}", b_id, s_id);

        // 1. Switch to main
        self.run_git(&["checkout", &main])
            .await
            .with_context(|| format!("failed to checkout {}", main))?;

        // 2. Squash merge
        self.run_git(&["merge", "--squash", &branch])
            .await
            .with_context(|| format!("failed to squash merge {}", branch))?;

        // 3. Build commit message
        let mut msg = format!("feat({}/{}): {}", b_id, s_id, section_title);
        if !task_summaries.is_empty() {
            msg.push_str("\n\nTasks completed:");
            for summary in task_summaries {
                msg.push_str(&format!("\n- {}", summary));
            }
        }

        self.run_git(&["commit", "-m", &msg])
            .await
            .context("failed to commit squash merge")?;

        // 4. Delete the section branch
        self.run_git(&["branch", "-D", &branch])
            .await
            .with_context(|| format!("failed to delete branch {}", branch))?;

        Ok(())
    }

    // ── Utility ─────────────────────────────────────────────────────────

    /// Fetch the latest commit: `git log -1 --format='%h %s'`.
    /// Returns (short_hash, subject). Uses parse_git_log_line from widgets.
    pub async fn last_commit(&self) -> anyhow::Result<(String, String)> {
        let output = self.run_git(&["log", "-1", "--format=%h %s"]).await
            .context("failed to read last commit")?;
        Ok(crate::tui::widgets::parse_git_log_line(&output))
    }

    /// Returns true if the working tree has uncommitted changes.
    pub async fn has_changes(&self) -> anyhow::Result<bool> {
        let output = self
            .run_git(&["status", "--porcelain"])
            .await
            .context("failed to check git status")?;
        Ok(!output.trim().is_empty())
    }

    /// Execute a git command and return stdout on success, or an error with stderr.
    async fn run_git(&self, args: &[&str]) -> anyhow::Result<String> {
        let output = tokio::process::Command::new("git")
            .args(args)
            .current_dir(&self.cwd)
            .output()
            .await
            .with_context(|| format!("failed to execute git {:?}", args))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            anyhow::bail!(
                "git {} failed (exit {}): {}",
                args.first().unwrap_or(&""),
                output.status.code().unwrap_or(-1),
                stderr.trim()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: create a temporary git repo for testing.
    async fn setup_temp_repo() -> (tempfile::TempDir, WorkflowGit) {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let git = WorkflowGit::new(dir.path());

        git.run_git(&["init"]).await.expect("git init failed");
        git.run_git(&["config", "user.email", "test@test.com"])
            .await
            .unwrap();
        git.run_git(&["config", "user.name", "Test"]).await.unwrap();

        // Create an initial commit so we have a branch
        let readme = dir.path().join("README.md");
        fs::write(&readme, "# test").expect("write failed");
        git.run_git(&["add", "-A"]).await.unwrap();
        git.run_git(&["commit", "-m", "initial"]).await.unwrap();

        (dir, git)
    }

    #[tokio::test]
    async fn test_current_branch() {
        let (_dir, git) = setup_temp_repo().await;
        let branch = git.current_branch().await.unwrap();
        // Could be "main" or "master" depending on git config
        assert!(!branch.is_empty());
    }

    #[tokio::test]
    async fn test_create_section_branch() {
        let (_dir, git) = setup_temp_repo().await;
        git.create_section_branch("B001", "S01").await.unwrap();
        let branch = git.current_branch().await.unwrap();
        assert_eq!(branch, "fin/B001/S01");
    }

    #[tokio::test]
    async fn test_create_fix_branch() {
        let (_dir, git) = setup_temp_repo().await;
        git.create_fix_branch("B001", "S01").await.unwrap();
        let branch = git.current_branch().await.unwrap();
        assert_eq!(branch, "fin/B001/S01-fix");
    }

    #[tokio::test]
    async fn test_has_changes() {
        let (dir, git) = setup_temp_repo().await;
        assert!(!git.has_changes().await.unwrap());

        fs::write(dir.path().join("new_file.txt"), "hello").unwrap();
        assert!(git.has_changes().await.unwrap());
    }

    #[tokio::test]
    async fn test_commit_task() {
        let (dir, git) = setup_temp_repo().await;
        git.create_section_branch("B001", "S01").await.unwrap();

        fs::write(dir.path().join("code.rs"), "fn main() {}").unwrap();
        let sha = git
            .commit_task("S01", "T01", "feat", "add main function")
            .await
            .unwrap();
        assert!(!sha.is_empty());
        assert!(!git.has_changes().await.unwrap());
    }

    #[tokio::test]
    async fn test_commit_artifacts() {
        let (dir, git) = setup_temp_repo().await;

        let doc = dir.path().join("SPEC.md");
        fs::write(&doc, "# Spec").unwrap();

        let sha = git
            .commit_artifacts("architect", "add project spec", &[doc])
            .await
            .unwrap();
        assert!(!sha.is_empty());
    }

    #[tokio::test]
    async fn test_squash_merge() {
        let (dir, git) = setup_temp_repo().await;
        let main = git.main_branch().await;

        // Create section branch with a commit
        git.create_section_branch("B001", "S01").await.unwrap();
        fs::write(dir.path().join("feature.rs"), "// feature").unwrap();
        git.commit_task("S01", "T01", "feat", "implement feature")
            .await
            .unwrap();

        // Squash merge back
        git.squash_merge_section(
            "B001",
            "S01",
            "Implement the feature",
            &["T01: implement feature".to_string()],
        )
        .await
        .unwrap();

        let branch = git.current_branch().await.unwrap();
        assert_eq!(branch, main);
    }

    #[test]
    fn test_last_commit_parse() {
        use crate::tui::widgets::parse_git_log_line;

        let (hash, msg) = parse_git_log_line("abc1234 feat: add scaffold");
        assert_eq!(hash, "abc1234");
        assert_eq!(msg, "feat: add scaffold");

        // Edge: no subject
        let (hash2, msg2) = parse_git_log_line("abc1234");
        assert_eq!(hash2, "abc1234");
        assert!(msg2.is_empty());

        // Edge: multiple spaces in subject
        let (hash3, msg3) = parse_git_log_line("def5678 fix: handle edge case");
        assert_eq!(hash3, "def5678");
        assert_eq!(msg3, "fix: handle edge case");

        // Edge: empty input
        let (hash4, msg4) = parse_git_log_line("");
        assert!(hash4.is_empty());
        assert!(msg4.is_empty());
    }

    #[tokio::test]
    async fn test_last_commit_returns_hash_and_msg() {
        let (dir, git) = setup_temp_repo().await;
        // Create a file and commit it
        fs::write(dir.path().join("hello.txt"), "world").unwrap();
        git.run_git(&["add", "."]).await.unwrap();
        git.run_git(&["commit", "-m", "feat: initial commit"]).await.unwrap();

        let (hash, msg) = git.last_commit().await.unwrap();
        assert_eq!(hash.len(), 7, "short hash should be 7 chars, got: {hash}");
        assert_eq!(msg, "feat: initial commit");
    }
}
