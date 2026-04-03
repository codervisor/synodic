//! Pipeline configuration, check runner, and Build→Inspect→Route state machine.
//!
//! Parses `.harness/pipeline.yml` — the single source of truth for a
//! project's quality gates — and executes the governed pipeline loop.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// When a check runs in git-hook mode.
///
/// - `Commit` → pre-commit hook
/// - `Push` → pre-push hook
/// - Omitted (`None`) → only during `synodic run`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Commit,
    Push,
}

/// A single quality check (format, lint, test, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Check {
    /// Human-readable name (e.g., "format", "lint", "test").
    pub name: String,
    /// Shell command to run the check.
    pub run: String,
    /// Optional shell command to auto-fix failures.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
    /// Optional git-hook stage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<Stage>,
}

/// Pipeline execution settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineSettings {
    /// Maximum Build↔Inspect rework cycles.
    #[serde(default = "default_max_rework")]
    pub max_rework: u32,
    /// Whether to auto-merge the PR on pass.
    #[serde(default)]
    pub auto_merge: bool,
    /// Claude model to use for BUILD phase (e.g. "sonnet", "opus", "claude-sonnet-4-6").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Claude thinking effort level (low, medium, high, max).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
}

fn default_max_rework() -> u32 {
    3
}

impl Default for PipelineSettings {
    fn default() -> Self {
        Self {
            max_rework: default_max_rework(),
            auto_merge: false,
            model: None,
            effort: None,
        }
    }
}

/// Top-level pipeline configuration (parsed from `.harness/pipeline.yml`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Project language (rust, node, python, go, generic).
    pub language: String,
    /// Quality checks to run.
    pub checks: Vec<Check>,
    /// Pipeline execution settings.
    #[serde(default)]
    pub pipeline: PipelineSettings,
}

/// Result of executing a single check as a subprocess.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    /// Name of the check.
    pub name: String,
    /// Whether the check passed (exit code 0).
    pub passed: bool,
    /// Process exit code.
    pub exit_code: i32,
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// Config loading
// ---------------------------------------------------------------------------

/// Load and parse a pipeline configuration file.
pub fn load_config(path: &Path) -> Result<PipelineConfig> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read pipeline config: {}", path.display()))?;
    let config: PipelineConfig = serde_yaml::from_str(&contents)
        .with_context(|| format!("failed to parse pipeline config: {}", path.display()))?;
    Ok(config)
}

// ---------------------------------------------------------------------------
// Check runner
// ---------------------------------------------------------------------------

/// Execute checks as subprocesses and collect results (output captured, silent).
///
/// Runs each check sequentially in the given working directory.
/// Does not short-circuit on failure — all checks run regardless.
pub async fn run_checks(checks: &[Check], cwd: &Path) -> Result<Vec<CheckResult>> {
    let mut results = Vec::with_capacity(checks.len());

    for check in checks {
        let start = Instant::now();

        let output = Command::new("sh")
            .arg("-c")
            .arg(&check.run)
            .current_dir(cwd)
            .output()
            .await
            .with_context(|| format!("failed to execute check '{}'", check.name))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        results.push(CheckResult {
            name: check.name.clone(),
            passed: output.status.success(),
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            duration_ms,
        });
    }

    Ok(results)
}

/// Execute checks with styled UI output (spinners + streaming lines).
pub async fn run_checks_ui(
    checks: &[Check],
    cwd: &Path,
    ui: &crate::ui::PipelineUi,
) -> Result<Vec<CheckResult>> {
    use tokio::io::{AsyncBufReadExt, BufReader};

    let mut results = Vec::with_capacity(checks.len());

    for check in checks {
        let start = Instant::now();
        let pb = ui.check_spinner(&check.name);

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&check.run)
            .current_dir(cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .with_context(|| format!("failed to execute check '{}'", check.name))?;

        let mut stdout_buf = Vec::new();
        let mut stderr_buf = Vec::new();

        // Read stdout and stderr concurrently (prevents pipe-buffer deadlock)
        let stdout = child.stdout.take().map(BufReader::new);
        let stderr = child.stderr.take().map(BufReader::new);

        let mut stdout_lines = stdout.map(|r| r.lines());
        let mut stderr_lines = stderr.map(|r| r.lines());
        let mut stdout_done = stdout_lines.is_none();
        let mut stderr_done = stderr_lines.is_none();

        // Show first N and last N lines, suppress the middle
        const HEAD_LINES: usize = 10;
        const TAIL_LINES: usize = 10;
        let mut displayed = 0usize;
        let mut suppressed = 0usize;
        let mut tail_buf: Vec<String> = Vec::new();

        while !stdout_done || !stderr_done {
            tokio::select! {
                line = async { stdout_lines.as_mut().unwrap().next_line().await },
                    if !stdout_done => {
                    match line? {
                        Some(l) => {
                            if displayed < HEAD_LINES {
                                ui.check_line(&pb, &l);
                                displayed += 1;
                            } else {
                                suppressed += 1;
                                tail_buf.push(l.clone());
                                if tail_buf.len() > TAIL_LINES {
                                    tail_buf.remove(0);
                                }
                            }
                            stdout_buf.push(l);
                        }
                        None => stdout_done = true,
                    }
                }
                line = async { stderr_lines.as_mut().unwrap().next_line().await },
                    if !stderr_done => {
                    match line? {
                        Some(l) => {
                            if displayed < HEAD_LINES {
                                ui.check_line(&pb, &l);
                                displayed += 1;
                            } else {
                                suppressed += 1;
                                tail_buf.push(l.clone());
                                if tail_buf.len() > TAIL_LINES {
                                    tail_buf.remove(0);
                                }
                            }
                            stderr_buf.push(l);
                        }
                        None => stderr_done = true,
                    }
                }
            }
        }

        // Show suppressed indicator + tail
        if suppressed > 0 {
            let hidden = suppressed.saturating_sub(tail_buf.len());
            if hidden > 0 {
                ui.check_line(&pb, &format!("...{hidden} lines hidden..."));
            }
            for line in &tail_buf {
                ui.check_line(&pb, line);
            }
        }

        let status = child.wait().await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        ui.check_done(pb, &check.name, status.success(), duration_ms);

        results.push(CheckResult {
            name: check.name.clone(),
            passed: status.success(),
            exit_code: status.code().unwrap_or(-1),
            stdout: stdout_buf.join("\n"),
            stderr: stderr_buf.join("\n"),
            duration_ms,
        });
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Filter checks to those matching a given stage.
///
/// Checks with no stage are excluded (they only run in `synodic run`).
pub fn filter_checks_by_stage(checks: &[Check], stage: Stage) -> Vec<&Check> {
    checks
        .iter()
        .filter(|c| c.stage.as_ref() == Some(&stage))
        .collect()
}

// ---------------------------------------------------------------------------
// Pipeline runner — Build→Inspect→Route state machine
// ---------------------------------------------------------------------------

/// Configuration for a pipeline run.
pub struct RunConfig {
    /// Task description for the BUILD agent.
    pub prompt: String,
    /// Maximum Build↔Inspect rework cycles.
    pub max_rework: u32,
    /// INSPECT only — skip BUILD and PR.
    pub dry_run: bool,
    /// Skip PR creation (run BUILD+INSPECT only).
    pub local: bool,
    /// Custom branch name (default: auto-generated).
    pub branch: Option<String>,
    /// Claude model (e.g. "sonnet", "opus"). None = claude default.
    pub model: Option<String>,
    /// Claude thinking effort (low, medium, high, max). None = claude default.
    pub effort: Option<String>,
    /// Project directory.
    pub project_dir: PathBuf,
}

/// Outcome of a pipeline run.
#[derive(Debug)]
pub enum RunOutcome {
    /// All checks passed.
    Passed {
        attempts: u32,
        pr_url: Option<String>,
    },
    /// Exhausted rework budget with remaining failures.
    Failed {
        attempts: u32,
        last_failures: Vec<CheckResult>,
    },
    /// Something went wrong outside the loop.
    Error(String),
}

/// Build the prompt for the BUILD phase.
///
/// On the first attempt, just the task. On rework attempts, append
/// check failure feedback so the agent knows what to fix.
pub fn build_prompt(task: &str, attempt: u32, failures: &[CheckResult]) -> String {
    if attempt <= 1 || failures.is_empty() {
        return format!(
            "## Task\n{task}\n\n\
             Rules:\n\
             - Implement the task described above\n\
             - Follow existing code conventions\n\
             - Commit your changes with a clear message"
        );
    }

    let mut feedback = String::new();
    for f in failures {
        feedback.push_str(&format!("### {} (exit {})\n", f.name, f.exit_code));
        let output = if !f.stderr.is_empty() {
            &f.stderr
        } else {
            &f.stdout
        };
        // Truncate long output
        let truncated: String = output
            .lines()
            .rev()
            .take(40)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        feedback.push_str(&format!("```\n{truncated}\n```\n\n"));
    }

    format!(
        "## Task\n{task}\n\n\
         ## Rework Required (attempt {attempt})\n\
         The previous attempt failed quality checks. Fix ALL issues:\n\n\
         {feedback}\
         Rules:\n\
         - Fix every issue listed above\n\
         - Do not break existing functionality\n\
         - Commit your changes with a clear message"
    )
}

/// Run the full Build→Inspect→Route pipeline.
///
/// For non-dry-run: creates a git worktree so Claude works in an isolated
/// copy of the repo, leaving the user's working tree untouched.
pub async fn run_pipeline(
    config: &PipelineConfig,
    run_cfg: &RunConfig,
    ui: &crate::ui::PipelineUi,
) -> Result<RunOutcome> {
    ui.header(&run_cfg.prompt, run_cfg.dry_run);

    // --- INIT: create worktree + branch (skip in dry-run) ---
    let (work_dir, branch, worktree_path) = if !run_cfg.dry_run {
        let branch_name = run_cfg.branch.clone().unwrap_or_else(|| {
            let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
            format!("synodic/{ts}")
        });

        let wt_path = run_cfg
            .project_dir
            .join(".synodic/worktrees")
            .join(branch_name.replace('/', "-"));

        // Clean up stale worktree at this path if it exists
        if wt_path.exists() {
            Command::new("git")
                .args(["worktree", "remove", "--force"])
                .arg(&wt_path)
                .current_dir(&run_cfg.project_dir)
                .status()
                .await
                .ok();
            if wt_path.exists() {
                tokio::fs::remove_dir_all(&wt_path).await.ok();
            }
        }

        if let Some(parent) = wt_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let status = Command::new("git")
            .args(["worktree", "add", "-b", &branch_name])
            .arg(&wt_path)
            .current_dir(&run_cfg.project_dir)
            .status()
            .await
            .context("failed to create worktree")?;

        if !status.success() {
            return Ok(RunOutcome::Error(format!(
                "git worktree add -b {branch_name} failed"
            )));
        }

        ui.worktree_info(&branch_name, &wt_path.display().to_string());

        (wt_path.clone(), Some(branch_name), Some(wt_path))
    } else {
        (run_cfg.project_dir.clone(), None, None)
    };

    let outcome = run_pipeline_loop(config, run_cfg, &work_dir, branch.as_deref(), ui).await;

    // --- CLEANUP: remove worktree ---
    if let Some(wt) = &worktree_path {
        ui.cleanup();
        Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(wt)
            .current_dir(&run_cfg.project_dir)
            .status()
            .await
            .ok();
    }

    outcome
}

/// The inner Build→Inspect→Route loop, separated so cleanup always runs.
async fn run_pipeline_loop(
    config: &PipelineConfig,
    run_cfg: &RunConfig,
    cwd: &Path,
    branch: Option<&str>,
    ui: &crate::ui::PipelineUi,
) -> Result<RunOutcome> {
    let mut last_failures: Vec<CheckResult> = Vec::new();

    let max_attempts = if run_cfg.dry_run {
        1
    } else {
        run_cfg.max_rework
    };

    for attempt in 1..=max_attempts {
        ui.separator();

        // BUILD: invoke claude with stream-json for real-time visibility
        if !run_cfg.dry_run {
            let prompt = build_prompt(&run_cfg.prompt, attempt, &last_failures);
            run_build(
                &prompt,
                cwd,
                run_cfg.model.as_deref(),
                run_cfg.effort.as_deref(),
                ui,
            )
            .await?;
        }

        // INSPECT
        ui.section("INSPECT");
        let results = run_checks_ui(&config.checks, cwd, ui).await?;

        let mut all_passed = true;
        last_failures.clear();

        for r in &results {
            if !r.passed {
                all_passed = false;
                last_failures.push(r.clone());
            }
        }

        // ROUTE
        if all_passed {
            ui.all_passed();

            let pr_url = if !run_cfg.dry_run && !run_cfg.local {
                create_pr(cwd, branch, &run_cfg.prompt, attempt, ui).await?
            } else {
                None
            };

            return Ok(RunOutcome::Passed {
                attempts: attempt,
                pr_url,
            });
        }

        if attempt < max_attempts {
            ui.rework(last_failures.len());
        }
    }

    Ok(RunOutcome::Failed {
        attempts: max_attempts,
        last_failures,
    })
}

/// Run the BUILD phase — invoke Claude with stream-json for real-time visibility.
async fn run_build(
    prompt: &str,
    cwd: &Path,
    model: Option<&str>,
    effort: Option<&str>,
    ui: &crate::ui::PipelineUi,
) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, BufReader};

    ui.section("BUILD");
    let pb = ui.build_spinner();
    let start = Instant::now();

    let mut args = vec![
        "--print",
        "-p",
        prompt,
        "--output-format",
        "stream-json",
        "--verbose",
    ];
    let model_str;
    if let Some(m) = model {
        model_str = m.to_string();
        args.push("--model");
        args.push(&model_str);
    }
    let effort_str;
    if let Some(e) = effort {
        effort_str = e.to_string();
        args.push("--effort");
        args.push(&effort_str);
    }

    let mut child = Command::new("claude")
        .args(&args)
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("failed to invoke claude")?;

    let mut cost: Option<f64> = None;

    if let Some(stdout) = child.stdout.take() {
        let mut lines = BufReader::new(stdout).lines();
        while let Some(line) = lines.next_line().await? {
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
                match event.get("type").and_then(|t| t.as_str()) {
                    Some("system") => {
                        // Session started — show model + effort
                        if let Some(model_val) = event.get("model").and_then(|m| m.as_str()) {
                            let clean = model_val.split('[').next().unwrap_or(model_val);
                            let info = match effort {
                                Some(e) => format!("{clean} (effort: {e})"),
                                None => clean.to_string(),
                            };
                            ui.build_tool_call(&pb, "Model", &info);
                        }
                        pb.set_message("working...");
                    }
                    Some("assistant") => {
                        if let Some(content) =
                            event.pointer("/message/content").and_then(|c| c.as_array())
                        {
                            for item in content {
                                match item.get("type").and_then(|t| t.as_str()) {
                                    Some("tool_use") => {
                                        let tool = item
                                            .get("name")
                                            .and_then(|n| n.as_str())
                                            .unwrap_or("?");
                                        let summary = extract_tool_summary(item);
                                        ui.build_tool_call(&pb, tool, &summary);
                                        pb.set_message(format!("{tool}..."));
                                    }
                                    Some("thinking") => {
                                        if let Some(text) =
                                            item.get("thinking").and_then(|t| t.as_str())
                                        {
                                            ui.build_text_block(&pb, "Think", text, 4);
                                            pb.set_message("thinking...");
                                        }
                                    }
                                    Some("text") => {
                                        if let Some(text) =
                                            item.get("text").and_then(|t| t.as_str())
                                        {
                                            ui.build_text_block(&pb, "Output", text, 3);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    Some("result") => {
                        cost = event.get("total_cost_usd").and_then(|c| c.as_f64());
                    }
                    _ => {}
                }
            }
        }
    }

    let status = child.wait().await?;
    let duration_ms = start.elapsed().as_millis() as u64;
    ui.build_done(pb, status.success(), duration_ms, cost);

    Ok(())
}

/// Extract a short summary from a tool_use JSON content block.
fn extract_tool_summary(item: &serde_json::Value) -> String {
    let input = item.get("input");

    // Try common input fields in priority order
    let candidates = [
        "file_path",
        "command",
        "pattern",
        "skill",
        "description",
        "prompt",
        "query",
        "url",
    ];

    for key in candidates {
        if let Some(val) = input.and_then(|i| i.get(key)).and_then(|v| v.as_str()) {
            if key == "file_path" {
                // Shorten to last 2 path components
                let parts: Vec<&str> = val.rsplitn(3, '/').collect();
                return parts
                    .into_iter()
                    .rev()
                    .skip(1)
                    .collect::<Vec<_>>()
                    .join("/");
            }
            return val.to_string();
        }
    }

    String::new()
}

/// Push branch and create a PR via `gh`.
async fn create_pr(
    cwd: &Path,
    branch: Option<&str>,
    prompt: &str,
    attempts: u32,
    ui: &crate::ui::PipelineUi,
) -> Result<Option<String>> {
    let Some(branch_name) = branch else {
        return Ok(None);
    };

    ui.section("PR");
    ui.pr_status("pushing branch...");

    let push = Command::new("git")
        .args(["push", "-u", "origin", branch_name])
        .current_dir(cwd)
        .status()
        .await
        .context("git push failed")?;

    if !push.success() {
        ui.pr_status("git push failed, skipping PR creation");
        return Ok(None);
    }

    let title = format!("synodic: {}", truncate(prompt, 60));
    let body = format!(
        "## Summary\n\n\
         Automated pipeline run via [Synodic](https://github.com/codervisor/synodic).\n\n\
         **Prompt:** {prompt}\n\
         **Attempts:** {attempts}\n"
    );

    let output = Command::new("gh")
        .args(["pr", "create", "--title", &title, "--body", &body])
        .current_dir(cwd)
        .output()
        .await;

    match output {
        Ok(o) if o.status.success() => {
            let url = String::from_utf8_lossy(&o.stdout).trim().to_string();
            ui.pr_status(&format!("PR created: {url}"));
            Ok(Some(url))
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            ui.pr_status(&format!("gh pr create failed: {err}"));
            Ok(None)
        }
        Err(_) => {
            ui.pr_status("gh not found, skipping PR creation");
            ui.pr_status(&format!(
                "push succeeded -- create PR manually for branch '{branch_name}'"
            ));
            Ok(None)
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

// ---------------------------------------------------------------------------
// Generators — derive git hooks and GHA workflow from pipeline.yml
// ---------------------------------------------------------------------------

/// Generate a git hook script from checks matching a stage.
pub fn generate_hook_script(checks: &[Check], stage: Stage) -> Option<String> {
    let stage_checks = filter_checks_by_stage(checks, stage.clone());
    if stage_checks.is_empty() {
        return None;
    }

    let stage_name = match stage {
        Stage::Commit => "pre-commit",
        Stage::Push => "pre-push",
    };

    let mut script = format!(
        "#!/usr/bin/env bash\n\
         set -euo pipefail\n\
         # Generated by synodic init from .harness/pipeline.yml\n\n\
         echo \"Running {stage_name} checks...\"\n\n"
    );

    for check in &stage_checks {
        script.push_str(&format!(
            "echo \"  {name}\"\n\
             {cmd} || {{ echo \"FAILED: {name}\"; exit 1; }}\n\n",
            name = check.name,
            cmd = check.run,
        ));
    }

    script.push_str("echo \"All checks passed.\"\n");
    Some(script)
}

/// Generate a simplified GHA workflow that delegates to `synodic run`.
pub fn generate_workflow() -> String {
    r#"# Generated by synodic init
# Docs: https://github.com/codervisor/synodic

name: Synodic Pipeline

on:
  workflow_dispatch:
    inputs:
      prompt:
        description: "Task description"
        required: true
        type: string

jobs:
  pipeline:
    runs-on: ubuntu-latest
    timeout-minutes: 60
    permissions:
      contents: write
      pull-requests: write
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install Synodic + Claude Code
        run: npm install -g @codervisor/synodic @anthropic-ai/claude-code

      - name: Run pipeline
        run: synodic run --prompt "${{ inputs.prompt }}"
        env:
          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
"#
    .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Parsing ----------------------------------------------------------

    #[test]
    fn parse_full_config() {
        let yaml = r#"
language: rust

checks:
  - name: format
    run: "cargo fmt --all -- --check"
    fix: "cargo fmt --all"
  - name: lint
    run: "cargo clippy --all-targets -- -D warnings"
  - name: test
    run: "cargo test"

pipeline:
  max_rework: 3
  auto_merge: false
"#;
        let config: PipelineConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.language, "rust");
        assert_eq!(config.checks.len(), 3);
        assert_eq!(config.checks[0].name, "format");
        assert_eq!(config.checks[0].run, "cargo fmt --all -- --check");
        assert_eq!(config.checks[0].fix.as_deref(), Some("cargo fmt --all"));
        assert!(config.checks[0].stage.is_none());
        assert_eq!(config.checks[1].name, "lint");
        assert!(config.checks[1].fix.is_none());
        assert_eq!(config.pipeline.max_rework, 3);
        assert!(!config.pipeline.auto_merge);
    }

    #[test]
    fn parse_minimal_config() {
        let yaml = r#"
language: rust
checks:
  - name: test
    run: "cargo test"
"#;
        let config: PipelineConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.language, "rust");
        assert_eq!(config.checks.len(), 1);
        assert_eq!(config.pipeline.max_rework, 3);
        assert!(!config.pipeline.auto_merge);
    }

    #[test]
    fn parse_config_with_stages() {
        let yaml = r#"
language: rust
checks:
  - name: format
    run: "cargo fmt -- --check"
    stage: commit
  - name: lint
    run: "cargo clippy"
    stage: push
  - name: test
    run: "cargo test"
"#;
        let config: PipelineConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.checks[0].stage, Some(Stage::Commit));
        assert_eq!(config.checks[1].stage, Some(Stage::Push));
        assert!(config.checks[2].stage.is_none());
    }

    #[test]
    fn parse_invalid_yaml_returns_error() {
        let yaml = "not: [valid: yaml: {";
        let result = serde_yaml::from_str::<PipelineConfig>(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn parse_missing_checks_returns_error() {
        let yaml = "language: rust\n";
        let result = serde_yaml::from_str::<PipelineConfig>(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn load_missing_file_returns_error() {
        let result = load_config(Path::new("/nonexistent/pipeline.yml"));
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("failed to read pipeline config"));
    }

    // -- Stage filtering --------------------------------------------------

    #[test]
    fn filter_by_stage() {
        let checks = vec![
            Check {
                name: "format".into(),
                run: "cargo fmt -- --check".into(),
                fix: None,
                stage: Some(Stage::Commit),
            },
            Check {
                name: "lint".into(),
                run: "cargo clippy".into(),
                fix: None,
                stage: Some(Stage::Push),
            },
            Check {
                name: "test".into(),
                run: "cargo test".into(),
                fix: None,
                stage: None,
            },
        ];

        let commit = filter_checks_by_stage(&checks, Stage::Commit);
        assert_eq!(commit.len(), 1);
        assert_eq!(commit[0].name, "format");

        let push = filter_checks_by_stage(&checks, Stage::Push);
        assert_eq!(push.len(), 1);
        assert_eq!(push[0].name, "lint");
    }

    // -- Check execution --------------------------------------------------

    #[tokio::test]
    async fn run_passing_check() {
        let checks = vec![Check {
            name: "pass".into(),
            run: "true".into(),
            fix: None,
            stage: None,
        }];

        let results = run_checks(&checks, Path::new("/tmp")).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].passed);
        assert_eq!(results[0].exit_code, 0);
        assert_eq!(results[0].name, "pass");
    }

    #[tokio::test]
    async fn run_failing_check() {
        let checks = vec![Check {
            name: "fail".into(),
            run: "false".into(),
            fix: None,
            stage: None,
        }];

        let results = run_checks(&checks, Path::new("/tmp")).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(!results[0].passed);
        assert_ne!(results[0].exit_code, 0);
    }

    #[tokio::test]
    async fn run_captures_stdout() {
        let checks = vec![Check {
            name: "echo".into(),
            run: "echo hello".into(),
            fix: None,
            stage: None,
        }];

        let results = run_checks(&checks, Path::new("/tmp")).await.unwrap();
        assert_eq!(results[0].stdout.trim(), "hello");
    }

    #[tokio::test]
    async fn run_captures_stderr() {
        let checks = vec![Check {
            name: "stderr".into(),
            run: "echo error >&2".into(),
            fix: None,
            stage: None,
        }];

        let results = run_checks(&checks, Path::new("/tmp")).await.unwrap();
        assert_eq!(results[0].stderr.trim(), "error");
    }

    #[tokio::test]
    async fn run_multiple_no_shortcircuit() {
        let checks = vec![
            Check {
                name: "first".into(),
                run: "true".into(),
                fix: None,
                stage: None,
            },
            Check {
                name: "second".into(),
                run: "false".into(),
                fix: None,
                stage: None,
            },
            Check {
                name: "third".into(),
                run: "true".into(),
                fix: None,
                stage: None,
            },
        ];

        let results = run_checks(&checks, Path::new("/tmp")).await.unwrap();
        assert_eq!(results.len(), 3);
        assert!(results[0].passed);
        assert!(!results[1].passed);
        assert!(results[2].passed);
    }

    #[tokio::test]
    async fn run_measures_duration() {
        let checks = vec![Check {
            name: "sleep".into(),
            run: "sleep 0.1".into(),
            fix: None,
            stage: None,
        }];

        let results = run_checks(&checks, Path::new("/tmp")).await.unwrap();
        assert!(results[0].duration_ms >= 50);
    }

    #[tokio::test]
    async fn run_uses_cwd() {
        let checks = vec![Check {
            name: "pwd".into(),
            run: "pwd".into(),
            fix: None,
            stage: None,
        }];

        let results = run_checks(&checks, Path::new("/tmp")).await.unwrap();
        assert!(results[0].stdout.trim().starts_with("/tmp"));
    }

    // -- Prompt construction ----------------------------------------------

    #[test]
    fn build_prompt_first_attempt() {
        let prompt = build_prompt("add rate limiting", 1, &[]);
        assert!(prompt.contains("## Task"));
        assert!(prompt.contains("add rate limiting"));
        assert!(!prompt.contains("Rework"));
    }

    #[test]
    fn build_prompt_with_rework_feedback() {
        let failures = vec![CheckResult {
            name: "test".into(),
            passed: false,
            exit_code: 1,
            stdout: String::new(),
            stderr: "thread 'main' panicked".into(),
            duration_ms: 100,
        }];

        let prompt = build_prompt("add rate limiting", 2, &failures);
        assert!(prompt.contains("Rework Required (attempt 2)"));
        assert!(prompt.contains("### test (exit 1)"));
        assert!(prompt.contains("thread 'main' panicked"));
    }

    #[test]
    fn build_prompt_rework_ignores_empty_failures() {
        let prompt = build_prompt("task", 2, &[]);
        // No failures = treat as first attempt style
        assert!(!prompt.contains("Rework"));
    }

    // -- Hook generation --------------------------------------------------

    #[test]
    fn generate_pre_commit_hook() {
        let checks = vec![
            Check {
                name: "format".into(),
                run: "cargo fmt -- --check".into(),
                fix: None,
                stage: Some(Stage::Commit),
            },
            Check {
                name: "test".into(),
                run: "cargo test".into(),
                fix: None,
                stage: Some(Stage::Push),
            },
        ];

        let script = generate_hook_script(&checks, Stage::Commit).unwrap();
        assert!(script.contains("#!/usr/bin/env bash"));
        assert!(script.contains("cargo fmt -- --check"));
        assert!(!script.contains("cargo test"));
        assert!(script.contains("pre-commit"));
    }

    #[test]
    fn generate_hook_empty_stage_returns_none() {
        let checks = vec![Check {
            name: "test".into(),
            run: "cargo test".into(),
            fix: None,
            stage: None,
        }];

        assert!(generate_hook_script(&checks, Stage::Commit).is_none());
    }

    // -- Workflow generation ----------------------------------------------

    #[test]
    fn generate_workflow_contains_synodic_run() {
        let wf = generate_workflow();
        assert!(wf.contains("synodic run"));
        assert!(wf.contains("ANTHROPIC_API_KEY"));
        assert!(wf.contains("workflow_dispatch"));
    }
}
