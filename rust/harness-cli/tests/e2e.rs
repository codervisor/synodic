//! End-to-end integration tests for the synodic CLI.
//!
//! Exercises the full workflow: init → submit → list → search → stats → resolve → rules.

use std::process::Command;

fn synodic() -> Command {
    Command::new(env!("CARGO_BIN_EXE_synodic"))
}

/// Create a temporary git repo with `synodic init` and return the path.
fn init_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create tmpdir");
    // git init (required for synodic to find repo root)
    let status = Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .expect("git init");
    assert!(status.status.success(), "git init failed");

    let out = synodic()
        .args(["init"])
        .current_dir(dir.path())
        .output()
        .expect("synodic init");
    assert!(out.status.success(), "synodic init failed: {}", lossy_both(&out));
    assert!(dir.path().join(".harness/synodic.db").exists());
    dir
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

fn lossy_both(out: &std::process::Output) -> String {
    format!("{}{}", lossy(&out.stdout), lossy(&out.stderr))
}

#[test]
fn test_init_creates_harness_dir() {
    let dir = init_project();
    assert!(dir.path().join(".harness").is_dir());
    assert!(dir.path().join(".harness/synodic.db").exists());
    assert!(dir.path().join(".harness/gates.yml").exists());
}

#[test]
fn test_full_lifecycle() {
    let dir = init_project();
    let wd = dir.path();

    // Submit events
    let out = synodic()
        .args([
            "submit",
            "--type", "hallucination",
            "--title", "Referenced nonexistent API /v2/users",
            "--severity", "high",
            "--source", "claude",
        ])
        .current_dir(wd)
        .output()
        .expect("submit");
    assert!(out.status.success(), "submit failed: {}", lossy_both(&out));
    let event_id = lossy(&out.stdout).trim().to_string();
    assert!(!event_id.is_empty(), "submit should return event ID");

    let out2 = synodic()
        .args([
            "submit",
            "--type", "compliance_violation",
            "--title", "Attempted to read .env file",
            "--severity", "critical",
            "--source", "copilot",
        ])
        .current_dir(wd)
        .output()
        .expect("submit 2");
    assert!(out2.status.success());

    // List events
    let out = synodic()
        .args(["list"])
        .current_dir(wd)
        .output()
        .expect("list");
    assert!(out.status.success());
    let list_text = lossy(&out.stdout);
    assert!(list_text.contains("hallucination"), "list should show event type");
    assert!(list_text.contains("compliance_violation"), "list should show both events");

    // List with --json
    let out = synodic()
        .args(["list", "--json"])
        .current_dir(wd)
        .output()
        .expect("list --json");
    assert!(out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    assert_eq!(json.as_array().unwrap().len(), 2);

    // Search
    let out = synodic()
        .args(["search", "API"])
        .current_dir(wd)
        .output()
        .expect("search");
    assert!(out.status.success());
    let search_text = lossy(&out.stdout);
    assert!(search_text.contains("/v2/users"), "search should find matching event");

    // Stats
    let out = synodic()
        .args(["stats"])
        .current_dir(wd)
        .output()
        .expect("stats");
    assert!(out.status.success());
    let stats_text = lossy_both(&out);
    assert!(stats_text.contains("Total events:    2"), "stats should show 2 events");
    assert!(stats_text.contains("Unresolved:      2"), "all events unresolved");

    // Resolve
    let out = synodic()
        .args(["resolve", &event_id, "--notes", "Verified endpoint exists in v3"])
        .current_dir(wd)
        .output()
        .expect("resolve");
    assert!(out.status.success(), "resolve failed: {}", lossy_both(&out));

    // Stats after resolve
    let out = synodic()
        .args(["stats"])
        .current_dir(wd)
        .output()
        .expect("stats after resolve");
    assert!(out.status.success());
    let stats_text = lossy_both(&out);
    assert!(stats_text.contains("Unresolved:      1"), "one event should remain unresolved");
    assert!(stats_text.contains("Resolution rate: 50%"), "resolution rate should be 50%");
}

#[test]
fn test_rules_list() {
    let dir = init_project();
    let out = synodic()
        .args(["rules", "list"])
        .current_dir(dir.path())
        .output()
        .expect("rules list");
    assert!(out.status.success());
    let text = lossy(&out.stdout);
    assert!(text.contains("secret-in-output"), "built-in rules should be listed");
    assert!(text.contains("rm-rf-dangerous"), "built-in rules should be listed");
    assert!(text.contains("force-push"), "built-in rules should be listed");
}

#[test]
fn test_rules_list_json() {
    let dir = init_project();
    let out = synodic()
        .args(["rules", "list", "--json"])
        .current_dir(dir.path())
        .output()
        .expect("rules list --json");
    assert!(out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid JSON");
    assert!(json.as_array().unwrap().len() >= 5, "should have at least 5 built-in rules");
}

#[test]
fn test_submit_invalid_type() {
    let dir = init_project();
    let out = synodic()
        .args([
            "submit",
            "--type", "bogus",
            "--title", "test",
        ])
        .current_dir(dir.path())
        .output()
        .expect("submit with bad type");
    assert!(!out.status.success(), "invalid event type should fail");
}

#[test]
fn test_empty_list() {
    let dir = init_project();
    let out = synodic()
        .args(["list"])
        .current_dir(dir.path())
        .output()
        .expect("list empty");
    assert!(out.status.success());
    let stderr = lossy_both(&out);
    assert!(stderr.contains("No events found"), "should report no events");
}

#[test]
fn test_stats_empty() {
    let dir = init_project();
    let out = synodic()
        .args(["stats"])
        .current_dir(dir.path())
        .output()
        .expect("stats empty");
    assert!(out.status.success());
    let text = lossy_both(&out);
    assert!(text.contains("Total events:    0"));
}
