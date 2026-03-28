use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use regex::Regex;
use serde_json::{json, Value};

/// Configuration for a governed run.
pub struct RunConfig {
    pub max_rework: u32,
    pub workdir: Option<String>,
    pub no_l2: bool,
    pub judge: String,
    pub base_ref: Option<String>,
    pub dry_run: bool,
    pub quiet: bool,
    pub json_output: bool,
    pub agent_cmd: Vec<String>,
}

/// Execute the governance loop natively.
pub fn execute(config: RunConfig) -> Result<()> {
    let repo_root = crate::util::find_repo_root()?;
    let harness_dir = resolve_harness_dir(&repo_root)?;
    let workdir = match &config.workdir {
        Some(d) => PathBuf::from(d),
        None => git_repo_root(&repo_root),
    };

    let run_id = format!("harness-{}", Utc::now().timestamp());
    let run_dir = harness_dir.join(".runs").join(&run_id);
    fs::create_dir_all(&run_dir)?;

    // Snapshot base ref
    let base_ref = config
        .base_ref
        .clone()
        .unwrap_or_else(|| git_rev_parse(&workdir, "HEAD").unwrap_or_default());

    log_info(&config, &format!("Starting governed run: {run_id}"));
    log_info(
        &config,
        &format!("Agent command: {}", config.agent_cmd.join(" ")),
    );
    log_info(
        &config,
        &format!("Working directory: {}", workdir.display()),
    );
    log_info(
        &config,
        &format!(
            "Base ref: {}",
            if base_ref.is_empty() {
                "(no git)"
            } else {
                &base_ref
            }
        ),
    );
    log_info(&config, &format!("Max rework: {}", config.max_rework));
    log_info(
        &config,
        &format!(
            "Layer 2 (AI judge): {}",
            if config.no_l2 { "disabled" } else { "enabled" }
        ),
    );
    log_info(&config, "");

    if config.dry_run {
        log_info(&config, "DRY RUN — would execute:");
        log_info(
            &config,
            &format!("  1. Run agent: {}", config.agent_cmd.join(" ")),
        );
        log_info(
            &config,
            &format!("  2. Observe: git diff {base_ref}...HEAD"),
        );
        log_info(
            &config,
            &format!("  3. AI judge via {}", config.judge),
        );
        log_info(
            &config,
            &format!("  4. Rework loop (up to {} times)", config.max_rework),
        );
        log_info(
            &config,
            &format!(
                "  5. Log to {}/harness.governance.jsonl",
                harness_dir.display()
            ),
        );
        return Ok(());
    }

    // --- Governance loop ---
    let mut attempt: u32 = 0;
    let mut status = "running".to_string();
    let mut last_agent_exit: i32 = 0;
    let feedback_file = run_dir.join("feedback.md");
    let mut all_rework_items: Vec<Value> = Vec::new();
    let start_time = Utc::now();

    // Pre-compile regexes used inside the governance loop
    let verdict_re = Regex::new(r"VERDICT:\s*(\w+)").unwrap();
    let summary_re = Regex::new(r"SUMMARY:\s*(.*)").unwrap();
    let items_re = Regex::new(r"(?s)ITEMS:\s*\n(.*?)(?:SUMMARY:|===)").unwrap();
    let item_re = Regex::new(r"\[(\w+)\]\s*(.*)").unwrap();

    while attempt <= config.max_rework && status == "running" {
        attempt += 1;
        log_info(
            &config,
            &format!("━━━ Attempt {}/{} ━━━", attempt, config.max_rework + 1),
        );

        // --- Run the agent ---
        log_info(&config, "Running agent...");
        let agent_log = run_dir.join(format!("agent-attempt-{attempt}.log"));

        let agent_exit = if feedback_file.exists() && attempt > 1 {
            let feedback = fs::read_to_string(&feedback_file)?;
            log_info(
                &config,
                &format!(
                    "Providing rework feedback ({} lines)",
                    feedback.lines().count()
                ),
            );

            let rework_input = format!(
                "## Harness Governance Feedback (Rework Required)\n\n\
                 The governance layer has reviewed your previous output and found issues.\n\
                 Please address the following feedback, then make your changes.\n\n\
                 {feedback}\n\n\
                 Re-read the original task and fix the issues above.\n"
            );
            run_agent_with_stdin(
                &config.agent_cmd,
                &workdir,
                &rework_input,
                &agent_log,
                &repo_root,
            )?
        } else {
            run_agent(&config.agent_cmd, &workdir, &agent_log, &repo_root)?
        };

        last_agent_exit = agent_exit;
        log_info(
            &config,
            &format!("Agent finished (exit code: {agent_exit})"),
        );

        // --- Observe: what changed? ---
        let diff_file = run_dir.join(format!("diff-attempt-{attempt}.patch"));
        let (has_changes, diff_stat) = observe_changes(&workdir, &base_ref, &diff_file)?;

        if !has_changes {
            log_info(
                &config,
                "No changes detected. Agent may have failed silently.",
            );
            if agent_exit != 0 {
                status = "error".to_string();
            } else {
                status = "passed".to_string();
            }
            break;
        }

        log_info(&config, "Changes detected:");
        for line in diff_stat.lines().take(20) {
            log_info(&config, &format!("  {line}"));
        }

        // =================================================================
        // AI Judge (L2) — semantic review
        // L1 (lint, format, test) is handled by git hooks and CI.
        // =================================================================
        if config.no_l2 {
            log_info(&config, "");
            log_info(&config, "Layer 2: SKIPPED (--no-l2)");
            status = "passed".to_string();
            break;
        }

        log_info(&config, "");
        log_info(&config, &format!("Layer 2: AI judge ({})...", config.judge));

        let judge_output_file = run_dir.join(format!("judge-attempt-{attempt}.log"));
        let diff_content = fs::read_to_string(&diff_file).unwrap_or_default();
        let capped_diff: String = diff_content
            .lines()
            .take(2000)
            .collect::<Vec<_>>()
            .join("\n");

        let judge_prompt = format!(
            "You are an independent code reviewer for the Harness governance system.\n\
             You have NOT seen the agent's reasoning — you only see the diff.\n\n\
             ## Review the following changes\n\n\
             ```diff\n{capped_diff}\n```\n\n\
             ## Review dimensions\n\n\
             Evaluate against these criteria:\n\n\
             1. **Completeness** — Does the change look like it addresses a coherent goal?\n\
             2. **Correctness** — Logic errors, bugs, off-by-one, null handling?\n\
             3. **Security** — Injection, hardcoded secrets, unsafe operations?\n\
             4. **Conformance** — Does the approach make sense for the codebase?\n\
             5. **Quality** — Clean, maintainable, no dead code or debug artifacts?\n\n\
             ## Output format\n\n\
             Respond with EXACTLY one of these two formats:\n\n\
             If approved:\n```\n=== VERDICT ===\nVERDICT: APPROVE\nSUMMARY: <one line>\n=== END VERDICT ===\n```\n\n\
             If rework needed:\n```\n=== VERDICT ===\nVERDICT: REWORK\nITEMS:\n- [category] specific actionable issue\n- [category] specific actionable issue\nSUMMARY: <one line>\n=== END VERDICT ===\n```\n\n\
             Categories must be one of: completeness, correctness, security, conformance, quality.\n\
             Be rigorous but fair. Only flag genuine issues, not style preferences."
        );

        let judge_exit = run_judge(&config.judge, &judge_prompt, &judge_output_file)?;

        if judge_exit != 0 {
            log_info(
                &config,
                &format!("  AI judge failed to run (exit {judge_exit}). Accepting by default."),
            );
            status = "passed".to_string();
            break;
        }

        let judge_text = fs::read_to_string(&judge_output_file).unwrap_or_default();

        let verdict = verdict_re
            .captures(&judge_text)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();

        let summary = summary_re
            .captures(&judge_text)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();

        if verdict == "APPROVE" {
            log_info(&config, &format!("  AI judge: APPROVE — {summary}"));
            status = "passed".to_string();
            break;
        } else if verdict == "REWORK" {
            log_info(&config, &format!("  AI judge: REWORK — {summary}"));

            // Extract rework items

            let rework_items: Vec<Value> = items_re
                .captures(&judge_text)
                .map(|c| c.get(1).unwrap().as_str())
                .unwrap_or("")
                .lines()
                .filter_map(|line| {
                    let line = line.trim().trim_start_matches('-').trim();
                    if line.is_empty() {
                        return None;
                    }
                    if let Some(caps) = item_re.captures(line) {
                        Some(json!({
                            "category": format!("[{}]", &caps[1]),
                            "description": caps[2].to_string()
                        }))
                    } else {
                        Some(json!({
                            "category": "[quality]",
                            "description": line.to_string()
                        }))
                    }
                })
                .collect();

            all_rework_items.extend(rework_items.iter().cloned());

            // Save feedback for next rework cycle
            let mut feedback_lines = vec!["## Layer 2 (AI Judge) Review Findings\n".to_string()];
            for item in &rework_items {
                let cat = item
                    .get("category")
                    .and_then(|v| v.as_str())
                    .unwrap_or("[quality]");
                let desc = item
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                feedback_lines.push(format!("- **{cat}** {desc}"));
            }
            feedback_lines.push(format!("\nSummary: {summary}"));
            feedback_lines
                .push("\nFix these issues. Your changes will be re-evaluated.".to_string());
            fs::write(&feedback_file, feedback_lines.join("\n"))?;

            // Save L2 report
            fs::write(
                run_dir.join(format!("l2-attempt-{attempt}.json")),
                serde_json::to_string_pretty(&json!({
                    "verdict": "REWORK",
                    "summary": &summary,
                    "items": &rework_items
                }))?,
            )?;

            if attempt > config.max_rework {
                log_info(&config, "");
                log_info(
                    &config,
                    &format!(
                        "Rework limit reached ({}). Escalating to human.",
                        config.max_rework
                    ),
                );
                status = "escalated".to_string();
            }
        } else {
            log_info(
                &config,
                "  AI judge: could not parse verdict. Accepting by default.",
            );
            log_info(
                &config,
                &format!("  (Raw output saved to {})", judge_output_file.display()),
            );
            status = "passed".to_string();
            break;
        }
    }

    // =================================================================
    // Finalize — record governance log
    // =================================================================

    // Agent exit code is authoritative: if the agent reports failure (e.g.
    // eval resolved=false), governance approval alone doesn't override that.
    if status == "passed" && last_agent_exit != 0 {
        log_info(
            &config,
            &format!("Agent exited with code {last_agent_exit} — overriding governance pass"),
        );
        status = "error".to_string();
    }

    let end_time = Utc::now();
    let duration = (end_time - start_time).num_seconds();

    log_info(&config, "");
    log_info(&config, "━━━ Result ━━━");
    log_info(&config, &format!("Status:   {status}"));
    log_info(&config, &format!("Attempts: {attempt}"));
    log_info(&config, &format!("Duration: {duration}s"));
    log_info(&config, &format!("Run dir:  {}", run_dir.display()));

    let gov_record = json!({
        "work_id": run_id,
        "source": "harness",
        "timestamp": Utc::now().to_rfc3339(),
        "status": status,
        "agent_command": config.agent_cmd.join(" "),
        "rework_items": all_rework_items,
        "metrics": {
            "attempt_count": attempt,
            "duration_s": duration,
            "base_ref": base_ref
        }
    });

    // Append to governance log
    let gov_log = harness_dir.join("harness.governance.jsonl");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&gov_log)?;
    writeln!(file, "{}", serde_json::to_string(&gov_record)?)?;
    log_info(&config, &format!("Governance log: {}", gov_log.display()));

    if config.json_output {
        println!("{}", serde_json::to_string_pretty(&gov_record)?);
    }

    // Write run manifest
    let manifest = json!({
        "run_id": run_id,
        "status": status,
        "attempts": attempt,
        "duration_s": duration,
        "agent_command": config.agent_cmd.join(" "),
        "base_ref": base_ref,
        "rework_items": all_rework_items,
        "workdir": workdir.display().to_string(),
        "harness_dir": harness_dir.display().to_string()
    });
    fs::write(
        run_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;

    // Exit code
    match status.as_str() {
        "passed" => Ok(()),
        "escalated" => std::process::exit(2),
        _ => std::process::exit(1),
    }
}

// --- Helpers ---

fn log_info(config: &RunConfig, msg: &str) {
    if !config.quiet {
        if msg.is_empty() {
            eprintln!();
        } else {
            eprintln!("harness: {msg}");
        }
    }
}

fn resolve_harness_dir(repo_root: &Path) -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("HARNESS_DIR") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return Ok(p);
        }
    }
    let hdir = repo_root.join(".harness");
    if !hdir.is_dir() {
        bail!(".harness/ directory not found at {}", hdir.display());
    }
    Ok(hdir)
}

fn git_repo_root(fallback: &Path) -> PathBuf {
    Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| PathBuf::from(s.trim()))
            } else {
                None
            }
        })
        .unwrap_or_else(|| fallback.to_path_buf())
}

fn git_rev_parse(workdir: &Path, refspec: &str) -> Option<String> {
    Command::new("git")
        .args(["-C", &workdir.display().to_string(), "rev-parse", refspec])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
}

fn run_agent(cmd: &[String], workdir: &Path, log_path: &Path, repo_root: &Path) -> Result<i32> {
    let log_file = fs::File::create(log_path)?;
    let status = Command::new(&cmd[0])
        .args(&cmd[1..])
        .current_dir(workdir)
        .env("SYNODIC_ROOT", repo_root)
        .env("EVAL_ROOT", repo_root)
        .stdout(log_file.try_clone()?)
        .stderr(log_file)
        .status()
        .with_context(|| format!("failed to run agent: {}", cmd[0]))?;
    Ok(status.code().unwrap_or(1))
}

fn run_agent_with_stdin(
    cmd: &[String],
    workdir: &Path,
    input: &str,
    log_path: &Path,
    repo_root: &Path,
) -> Result<i32> {
    let log_file = fs::File::create(log_path)?;
    let mut child = Command::new(&cmd[0])
        .args(&cmd[1..])
        .current_dir(workdir)
        .env("SYNODIC_ROOT", repo_root)
        .env("EVAL_ROOT", repo_root)
        .stdin(std::process::Stdio::piped())
        .stdout(log_file.try_clone()?)
        .stderr(log_file)
        .spawn()
        .with_context(|| format!("failed to run agent: {}", cmd[0]))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.as_bytes());
    }

    let status = child.wait()?;
    Ok(status.code().unwrap_or(1))
}

fn run_judge(judge_cmd: &str, prompt: &str, output_path: &Path) -> Result<i32> {
    let output_file = fs::File::create(output_path)?;
    let mut child = Command::new(judge_cmd)
        .args(["--print", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(output_file.try_clone()?)
        .stderr(output_file)
        .spawn()
        .with_context(|| format!("failed to run judge: {judge_cmd}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(prompt.as_bytes());
    }

    let status = child.wait()?;
    Ok(status.code().unwrap_or(1))
}

fn observe_changes(workdir: &Path, base_ref: &str, diff_file: &Path) -> Result<(bool, String)> {
    if base_ref.is_empty() {
        return Ok((false, String::new()));
    }

    let head_ref = match git_rev_parse(workdir, "HEAD") {
        Some(h) => h,
        None => return Ok((false, String::new())),
    };

    let wd = workdir.display().to_string();

    if head_ref != base_ref {
        // Committed changes
        let diff_range = format!("{base_ref}...{head_ref}");
        let diff_output = Command::new("git")
            .args(["-C", &wd, "diff", &diff_range])
            .output()?;
        fs::write(diff_file, &diff_output.stdout)?;

        let stat_output = Command::new("git")
            .args(["-C", &wd, "diff", "--stat", &diff_range])
            .output()?;
        let stat = String::from_utf8_lossy(&stat_output.stdout).to_string();
        return Ok((true, stat));
    }

    // Check for unstaged/staged changes
    let unstaged = Command::new("git").args(["-C", &wd, "diff"]).output()?;
    let staged = Command::new("git")
        .args(["-C", &wd, "diff", "--cached"])
        .output()?;

    let mut diff_content = unstaged.stdout;
    diff_content.extend_from_slice(&staged.stdout);

    if diff_content.is_empty() {
        return Ok((false, String::new()));
    }

    fs::write(diff_file, &diff_content)?;
    let stat_output = Command::new("git")
        .args(["-C", &wd, "diff", "--stat"])
        .output()?;
    let stat = String::from_utf8_lossy(&stat_output.stdout).to_string();
    Ok((true, stat))
}
