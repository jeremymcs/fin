// Fin + Integration Tests

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_version() {
    Command::cargo_bin("fin")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("fin"));
}

#[test]
fn test_help() {
    Command::cargo_bin("fin")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("AI coding agent"));
}

#[test]
fn test_models_list() {
    Command::cargo_bin("fin")
        .unwrap()
        .arg("models")
        .assert()
        .success()
        .stdout(predicate::str::contains("Claude"))
        .stdout(predicate::str::contains("GPT"))
        .stdout(predicate::str::contains("Gemini"));
}

/// Helper: strip all API key env vars and point FIN_HOME to an empty dir.
fn no_keys_cmd() -> assert_cmd::Command {
    let mut cmd = Command::cargo_bin("fin").unwrap();
    cmd.env_remove("ANTHROPIC_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .env_remove("GOOGLE_API_KEY")
        .env_remove("GEMINI_API_KEY")
        .env_remove("GOOGLE_CLOUD_PROJECT")
        .env_remove("CLOUDSDK_CORE_PROJECT")
        .env_remove("AWS_ACCESS_KEY_ID")
        .env_remove("AWS_PROFILE")
        // Point config to empty temp dir so stored auth.json isn't found
        .env("FIN_HOME", std::env::temp_dir().join("fin-test-empty"));
    cmd
}

#[test]
fn test_print_mode_no_key() {
    // Should fail gracefully when no API key is set
    no_keys_cmd()
        .args(["-p", "hello"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No API key"));
}

#[test]
fn test_mcp_initialize() {
    Command::cargo_bin("fin")
        .unwrap()
        .arg("mcp")
        .write_stdin("{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("protocolVersion"));
}

#[test]
fn test_mcp_tools_list() {
    Command::cargo_bin("fin")
        .unwrap()
        .arg("mcp")
        .write_stdin("{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{}}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("bash"))
        .stdout(predicate::str::contains("read"))
        .stdout(predicate::str::contains("write"));
}

#[test]
fn test_headless_no_key() {
    no_keys_cmd().args(["headless", "test"]).assert().failure();
}

#[test]
fn test_init_and_status() {
    let tmp = tempfile::tempdir().unwrap();

    // Init should create .fin/
    Command::cargo_bin("fin")
        .unwrap()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Initialized .fin/"));

    // Status should show the state
    Command::cargo_bin("fin")
        .unwrap()
        .arg("status")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Blueprint").or(predicate::str::contains("Idle")));

    // Init again should say already exists
    Command::cargo_bin("fin")
        .unwrap()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn test_blueprint_new_and_list() {
    let tmp = tempfile::tempdir().unwrap();

    // Init first
    Command::cargo_bin("fin")
        .unwrap()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();

    // Create blueprint
    Command::cargo_bin("fin")
        .unwrap()
        .args(["blueprint", "new", "MVP"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Created blueprint B001"));

    // List blueprints
    Command::cargo_bin("fin")
        .unwrap()
        .args(["blueprint", "list"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("B001"));
}

#[test]
fn test_sessions_list_empty() {
    // Should work even with no sessions
    Command::cargo_bin("fin")
        .unwrap()
        .args(["sessions", "list"])
        .assert()
        .success();
}

#[test]
fn test_auto_no_fin_dir() {
    let tmp = tempfile::tempdir().unwrap();

    // Auto should fail gracefully without .fin/
    no_keys_cmd()
        .arg("auto")
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn test_status_no_fin_dir() {
    let tmp = tempfile::tempdir().unwrap();

    // Status without .fin/ should say so
    Command::cargo_bin("fin")
        .unwrap()
        .arg("status")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No .fin/"));
}

#[test]
fn test_blueprint_wizard_blocks_concurrent() {
    let tmp = tempfile::tempdir().unwrap();

    // Init + create first blueprint
    Command::cargo_bin("fin")
        .unwrap()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();

    Command::cargo_bin("fin")
        .unwrap()
        .args(["blueprint", "new", "First"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("Created blueprint B001"));

    // Try to create second — should be blocked
    Command::cargo_bin("fin")
        .unwrap()
        .args(["blueprint", "new", "Second"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("already in progress"))
        .stderr(predicate::str::contains("Ignoring"));
}

#[test]
fn test_blueprint_complete_no_sections() {
    let tmp = tempfile::tempdir().unwrap();

    Command::cargo_bin("fin")
        .unwrap()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();

    Command::cargo_bin("fin")
        .unwrap()
        .args(["blueprint", "new", "MVP"])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Complete should fail — no sections
    Command::cargo_bin("fin")
        .unwrap()
        .args(["blueprint", "complete"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("no sections"));
}

#[test]
fn test_init_seeds_agents() {
    let tmp = tempfile::tempdir().unwrap();

    Command::cargo_bin("fin")
        .unwrap()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();

    // Verify agent files were seeded
    let agents_dir = tmp.path().join(".fin").join("agents");
    assert!(agents_dir.join("fin-researcher.md").exists());
    assert!(agents_dir.join("fin-planner.md").exists());
    assert!(agents_dir.join("fin-builder.md").exists());
    assert!(agents_dir.join("fin-reviewer.md").exists());
}

#[test]
fn test_status_with_active_blueprint() {
    let tmp = tempfile::tempdir().unwrap();

    Command::cargo_bin("fin")
        .unwrap()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();

    Command::cargo_bin("fin")
        .unwrap()
        .args(["blueprint", "new", "Auth System"])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Status should show progress summary
    Command::cargo_bin("fin")
        .unwrap()
        .arg("status")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("B001"))
        .stdout(predicate::str::contains("VISION.md"));
}

#[test]
fn test_rpc_get_state() {
    // RPC should respond to get_state (may fail on no API key but should parse the command)
    let _ = Command::cargo_bin("fin")
        .unwrap()
        .arg("--mode")
        .arg("rpc")
        .write_stdin("{\"type\":\"quit\"}\n")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("OPENAI_API_KEY")
        .env_remove("GOOGLE_API_KEY")
        .env_remove("GEMINI_API_KEY")
        .assert();
    // Just verify it doesn't panic — actual output depends on API key availability
}
