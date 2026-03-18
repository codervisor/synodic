use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize)]
struct BatchManifest {
    batch_id: String,
    started: String,
    config: BatchConfig,
    runs: Vec<BatchRun>,
    #[serde(skip_serializing_if = "Option::is_none")]
    completed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<BatchSummary>,
}

#[derive(Serialize, Deserialize)]
struct BatchConfig {
    tasks: Vec<String>,
    skills: Vec<String>,
    swe_split: String,
    agent_cmd: String,
}

#[derive(Serialize, Deserialize)]
struct BatchRun {
    run_id: String,
    task: String,
    skill: String,
    resolved: bool,
    exit_code: i32,
    duration_s: i64,
    result_file: String,
    log_file: String,
    timestamp: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BatchSummary {
    pub total_runs: usize,
    pub resolved: usize,
    pub failed: usize,
    pub errors: usize,
    pub per_skill: serde_json::Map<String, Value>,
    pub skill_deltas: serde_json::Map<String, Value>,
}

pub struct BatchOptions {
    pub tasks: Option<String>,
    pub skills: String,
    pub benchmark: Option<String>,
    pub split: String,
    pub output_dir: Option<String>,
    pub agent_cmd: String,
    pub dry_run: bool,
    pub skip_setup: bool,
    pub resume: bool,
}

/// Run batch evaluation across task×skill matrix.
pub fn execute(repo_root: &Path, opts: BatchOptions) -> Result<()> {
    let evals_dir = repo_root.join("evals");

    // Resolve task list
    let task_list = resolve_tasks(&evals_dir, opts.tasks.as_deref(), opts.benchmark.as_deref())?;
    let skill_list: Vec<String> = opts.skills.split(',').map(|s| s.trim().to_string()).collect();

    if task_list.is_empty() {
        bail!("No tasks matched. Check --tasks or --benchmark filter.");
    }

    let total_runs = task_list.len() * skill_list.len();

    // Output directory
    let output_dir = match &opts.output_dir {
        Some(d) => PathBuf::from(d),
        None => {
            let ts = Utc::now().format("%Y%m%d-%H%M").to_string();
            evals_dir.join(format!("results/batch-{ts}"))
        }
    };
    fs::create_dir_all(&output_dir)?;

    // Header
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║          Batch Eval                                          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Tasks:      {}", task_list.len());
    println!("Skills:     {}", skill_list.join(", "));
    println!("Total runs: {total_runs}");
    println!("Output:     {}", output_dir.display());
    println!("Agent:      {}", opts.agent_cmd);
    println!();

    // Print run matrix
    println!("┌─────────────────────────────────────────┬──────────┬──────────┐");
    println!("│ {:<39} │ {:<8} │ {:<8} │", "Task", "Skill", "Status");
    println!("├─────────────────────────────────────────┼──────────┼──────────┤");
    for task in &task_list {
        for skill in &skill_list {
            let result_file = output_dir.join(format!("{}_{}.json", sanitize(task), skill));
            let status = if opts.resume && result_file.exists() {
                "done"
            } else {
                "pending"
            };
            println!("│ {:<39} │ {:<8} │ {:<8} │", task, skill, status);
        }
    }
    println!("└─────────────────────────────────────────┴──────────┴──────────┘");
    println!();

    if opts.dry_run {
        println!("DRY RUN — commands that would execute:");
        println!();
        for task in &task_list {
            for skill in &skill_list {
                let mut extra = String::new();
                if task.starts_with("swe:") {
                    extra.push_str(&format!(" --split {}", opts.split));
                }
                if opts.skip_setup {
                    extra.push_str(" --skip-setup");
                }
                println!(
                    "  synodic-eval run {task} --skill {skill} --agent-cmd {}{extra}",
                    opts.agent_cmd
                );
            }
        }
        println!();
        println!("To execute: re-run without --dry-run");
        return Ok(());
    }

    // Initialize manifest
    let manifest_path = output_dir.join("manifest.json");
    let mut manifest = BatchManifest {
        batch_id: output_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        started: Utc::now().to_rfc3339(),
        config: BatchConfig {
            tasks: task_list.clone(),
            skills: skill_list.clone(),
            swe_split: opts.split.clone(),
            agent_cmd: opts.agent_cmd.clone(),
        },
        runs: Vec::new(),
        completed: None,
        summary: None,
    };
    save_manifest(&manifest, &manifest_path)?;

    // Run matrix
    let mut run_index = 0u32;
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;
    let mut errors = 0u32;

    let run_script = evals_dir.join("run.sh");

    for task in &task_list {
        for skill in &skill_list {
            run_index += 1;
            let run_id = format!("{}_{}", sanitize(task), skill);
            let result_file = output_dir.join(format!("{run_id}.json"));
            let log_file = output_dir.join(format!("{run_id}.log"));

            println!("━━━ Run {run_index}/{total_runs}: {task} × {skill} ━━━");

            if opts.resume && result_file.exists() {
                println!("  SKIP (already completed)");
                skipped += 1;
                println!();
                continue;
            }

            let start = Utc::now();

            let mut args = vec![
                task.clone(),
                "--skill".to_string(),
                skill.clone(),
                "--agent-cmd".to_string(),
                opts.agent_cmd.clone(),
                "--output".to_string(),
                result_file.display().to_string(),
            ];

            if task.starts_with("swe:") {
                args.push("--split".to_string());
                args.push(opts.split.clone());
            }
            if opts.skip_setup {
                args.push("--skip-setup".to_string());
            }

            // Run via run.sh, capture output to log file
            let log_f = fs::File::create(&log_file)?;
            let exit_code = std::process::Command::new(&run_script)
                .args(&args)
                .stdout(log_f.try_clone()?)
                .stderr(log_f)
                .status()
                .map(|s| s.code().unwrap_or(1))
                .unwrap_or(1);

            let duration = (Utc::now() - start).num_seconds();

            // Extract resolved status
            let resolved = result_file
                .exists()
                .then(|| {
                    fs::read_to_string(&result_file)
                        .ok()
                        .and_then(|c| serde_json::from_str::<Value>(&c).ok())
                        .and_then(|v| v.get("resolved").and_then(|r| r.as_bool()))
                        .unwrap_or(false)
                })
                .unwrap_or(false);

            manifest.runs.push(BatchRun {
                run_id,
                task: task.clone(),
                skill: skill.clone(),
                resolved,
                exit_code,
                duration_s: duration,
                result_file: result_file.display().to_string(),
                log_file: log_file.display().to_string(),
                timestamp: Utc::now().to_rfc3339(),
            });
            save_manifest(&manifest, &manifest_path)?;

            if exit_code == 0 {
                if resolved {
                    println!("  RESOLVED");
                    passed += 1;
                } else {
                    println!("  NOT RESOLVED");
                    failed += 1;
                }
            } else {
                println!("  ERROR (exit code {exit_code})");
                errors += 1;
            }
            println!();
        }
    }

    // Finalize with summary
    manifest.completed = Some(Utc::now().to_rfc3339());
    manifest.summary = Some(compute_summary(&manifest.runs));
    save_manifest(&manifest, &manifest_path)?;

    // Print summary
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║          Batch Results                                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Total: {total_runs}  Passed: {passed}  Failed: {failed}  Errors: {errors}  Skipped: {skipped}");
    println!();

    if let Some(summary) = &manifest.summary {
        print_skill_table(summary);
    }

    println!();
    println!("Full results: {}/", output_dir.display());
    println!("Manifest:     {}", manifest_path.display());
    println!();
    println!("To generate a report:");
    println!("  synodic-eval report {}", output_dir.display());

    Ok(())
}

fn resolve_tasks(evals_dir: &Path, tasks: Option<&str>, benchmark: Option<&str>) -> Result<Vec<String>> {
    if let Some(tasks_str) = tasks {
        return Ok(tasks_str.split(',').map(|s| s.trim().to_string()).collect());
    }

    let evals_path = evals_dir.join("evals.json");
    let content = fs::read_to_string(&evals_path)?;
    let registry: Value = serde_json::from_str(&content)?;
    let evals = registry
        .get("evals")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("invalid evals.json"))?;

    let mut result = Vec::new();
    for ev in evals {
        let e2e = match ev.get("e2e") {
            Some(e) => e,
            None => continue,
        };
        let alias = match e2e.get("alias").and_then(|v| v.as_str()) {
            Some(a) if !a.is_empty() => a,
            _ => continue,
        };

        if let Some(bench) = benchmark {
            let tags = ev
                .get("tags")
                .and_then(|t| t.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                .unwrap_or_default();

            let matches = match bench {
                "swebench" => tags.contains(&"swebench"),
                "featurebench" => tags.contains(&"featurebench"),
                "devbench" => tags.contains(&"devbench"),
                _ => tags.iter().any(|t| t == &bench),
            };
            if !matches {
                continue;
            }
        }

        result.push(alias.to_string());
    }

    Ok(result)
}

fn sanitize(s: &str) -> String {
    s.replace([':', '/'], "_")
}

fn save_manifest(manifest: &BatchManifest, path: &Path) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(manifest)?)?;
    Ok(())
}

fn compute_summary(runs: &[BatchRun]) -> BatchSummary {
    let mut per_skill = serde_json::Map::new();
    let mut skills: Vec<String> = runs.iter().map(|r| r.skill.clone()).collect();
    skills.sort();
    skills.dedup();

    let baseline_resolved: std::collections::HashSet<String> = runs
        .iter()
        .filter(|r| r.skill == "baseline" && r.resolved)
        .map(|r| r.task.clone())
        .collect();

    for skill in &skills {
        let skill_runs: Vec<&BatchRun> = runs.iter().filter(|r| &r.skill == skill).collect();
        let resolved = skill_runs.iter().filter(|r| r.resolved).count();
        let total = skill_runs.len();
        let avg_dur = if total > 0 {
            skill_runs.iter().map(|r| r.duration_s).sum::<i64>() as f64 / total as f64
        } else {
            0.0
        };

        per_skill.insert(
            skill.clone(),
            serde_json::json!({
                "resolved": resolved,
                "total": total,
                "resolve_rate": if total > 0 { resolved as f64 / total as f64 } else { 0.0 },
                "avg_duration_s": (avg_dur * 10.0).round() / 10.0
            }),
        );
    }

    // Skill deltas vs baseline
    let mut skill_deltas = serde_json::Map::new();
    for skill in &skills {
        if skill == "baseline" {
            continue;
        }
        let skill_resolved: std::collections::HashSet<String> = runs
            .iter()
            .filter(|r| &r.skill == skill && r.resolved)
            .map(|r| r.task.clone())
            .collect();

        let uplift: Vec<String> = skill_resolved.difference(&baseline_resolved).cloned().collect();
        let regressions: Vec<String> = baseline_resolved.difference(&skill_resolved).cloned().collect();
        let net = uplift.len() as i64 - regressions.len() as i64;

        skill_deltas.insert(
            skill.clone(),
            serde_json::json!({
                "uplift_tasks": uplift,
                "regression_tasks": regressions,
                "net_delta": net
            }),
        );
    }

    BatchSummary {
        total_runs: runs.len(),
        resolved: runs.iter().filter(|r| r.resolved).count(),
        failed: runs
            .iter()
            .filter(|r| !r.resolved && r.exit_code == 0)
            .count(),
        errors: runs.iter().filter(|r| r.exit_code != 0).count(),
        per_skill,
        skill_deltas,
    }
}

fn print_skill_table(summary: &BatchSummary) {
    println!("┌──────────┬──────────┬──────────┬───────────┬──────────────┐");
    println!(
        "│ {:<8} │ {:<8} │ {:<8} │ {:<9} │ {:<12} │",
        "Skill", "Resolved", "Total", "Rate", "Avg Time (s)"
    );
    println!("├──────────┼──────────┼──────────┼───────────┼──────────────┤");

    for (skill, stats) in &summary.per_skill {
        let resolved = stats.get("resolved").and_then(|v| v.as_u64()).unwrap_or(0);
        let total = stats.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
        let rate = stats
            .get("resolve_rate")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let avg = stats
            .get("avg_duration_s")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        println!(
            "│ {:<8} │ {:>8} │ {:>8} │ {:>8.1}% │ {:>12.1} │",
            skill,
            resolved,
            total,
            rate * 100.0,
            avg
        );
    }
    println!("└──────────┴──────────┴──────────┴───────────┴──────────────┘");
}
