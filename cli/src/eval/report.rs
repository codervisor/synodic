use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde_json::Value;

pub enum ReportMode {
    Single(PathBuf),
    Latest,
    All,
    Compare(String, String),
}

pub enum ReportFormat {
    Table,
    Json,
    Csv,
}

/// Generate a report from batch eval results.
pub fn generate(results_dir: &Path, mode: ReportMode, format: ReportFormat) -> Result<()> {
    match mode {
        ReportMode::Single(batch_dir) => {
            let dir = resolve_batch_dir(&batch_dir, results_dir)?;
            let manifest = load_manifest(&dir)?;
            render(&manifest, &format)
        }
        ReportMode::Latest => {
            let dir = find_latest_batch(results_dir)?;
            let manifest = load_manifest(&dir)?;
            render(&manifest, &format)
        }
        ReportMode::All => {
            list_all_batches(results_dir)
        }
        ReportMode::Compare(a, b) => {
            let dir_a = resolve_batch_dir(&PathBuf::from(&a), results_dir)?;
            let dir_b = resolve_batch_dir(&PathBuf::from(&b), results_dir)?;
            let manifest_a = load_manifest(&dir_a)?;
            let manifest_b = load_manifest(&dir_b)?;
            render_compare(&manifest_a, &manifest_b)
        }
    }
}

fn resolve_batch_dir(path: &Path, results_dir: &Path) -> Result<PathBuf> {
    if path.is_dir() && path.join("manifest.json").exists() {
        return Ok(path.to_path_buf());
    }
    // Try as relative name under results_dir
    let candidate = results_dir.join(path);
    if candidate.is_dir() && candidate.join("manifest.json").exists() {
        return Ok(candidate);
    }
    bail!(
        "No manifest.json found in {} or {}",
        path.display(),
        candidate.display()
    );
}

fn find_latest_batch(results_dir: &Path) -> Result<PathBuf> {
    let mut batches: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(results_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("batch-") && entry.path().is_dir() {
                batches.push(entry.path());
            }
        }
    }
    batches.sort();
    batches
        .last()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("No batch results found in {}", results_dir.display()))
}

fn load_manifest(dir: &Path) -> Result<Value> {
    let path = dir.join("manifest.json");
    let content = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&content)?)
}

fn render(manifest: &Value, format: &ReportFormat) -> Result<()> {
    match format {
        ReportFormat::Table => render_table(manifest),
        ReportFormat::Json => {
            let summary = manifest.get("summary").unwrap_or(manifest);
            println!("{}", serde_json::to_string_pretty(summary)?);
            Ok(())
        }
        ReportFormat::Csv => render_csv(manifest),
    }
}

fn render_table(manifest: &Value) -> Result<()> {
    let summary = match manifest.get("summary") {
        Some(s) if !s.is_null() => s,
        _ => {
            println!("ERROR: Batch has no summary (still running?)");
            return Ok(());
        }
    };

    let batch_id = manifest
        .get("batch_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let started = manifest
        .get("started")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let completed = manifest
        .get("completed")
        .and_then(|v| v.as_str())
        .unwrap_or("?");

    println!("Batch:     {batch_id}");
    println!("Started:   {started}");
    println!("Completed: {completed}");
    println!();

    let total = summary.get("total_runs").and_then(|v| v.as_u64()).unwrap_or(0);
    let resolved = summary.get("resolved").and_then(|v| v.as_u64()).unwrap_or(0);
    let failed = summary.get("failed").and_then(|v| v.as_u64()).unwrap_or(0);
    let errors = summary.get("errors").and_then(|v| v.as_u64()).unwrap_or(0);

    println!("Total runs:  {total}");
    println!("Resolved:    {resolved}");
    println!("Failed:      {failed}");
    println!("Errors:      {errors}");
    println!();

    // Per-skill table
    if let Some(skills) = summary.get("per_skill").and_then(|v| v.as_object()) {
        let hdr = format!(
            "{:<10} {:>8} {:>6} {:>8} {:>10}",
            "Skill", "Resolved", "Total", "Rate", "Avg Time"
        );
        println!("{hdr}");
        println!("{}", "─".repeat(hdr.len()));

        for (skill, stats) in skills {
            let res = stats.get("resolved").and_then(|v| v.as_u64()).unwrap_or(0);
            let tot = stats.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
            let rate = stats
                .get("resolve_rate")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let avg = stats
                .get("avg_duration_s")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            println!(
                "{:<10} {:>8} {:>6} {:>7.1}% {:>9.0}s",
                skill,
                res,
                tot,
                rate * 100.0,
                avg
            );
        }
        println!();
    }

    // Per-task × skill matrix
    if let Some(runs) = manifest.get("runs").and_then(|v| v.as_array()) {
        let mut tasks: Vec<String> = runs
            .iter()
            .filter_map(|r| r.get("task").and_then(|v| v.as_str()))
            .map(|s| s.to_string())
            .collect();
        tasks.sort();
        tasks.dedup();

        let mut skill_names: Vec<String> = runs
            .iter()
            .filter_map(|r| r.get("skill").and_then(|v| v.as_str()))
            .map(|s| s.to_string())
            .collect();
        skill_names.sort();
        skill_names.dedup();

        let lookup: HashMap<(String, String), &Value> = runs
            .iter()
            .filter_map(|r| {
                let t = r.get("task")?.as_str()?.to_string();
                let s = r.get("skill")?.as_str()?.to_string();
                Some(((t, s), r))
            })
            .collect();

        let col_w = skill_names
            .iter()
            .map(|s| s.len())
            .max()
            .unwrap_or(8)
            .max(10)
            + 2;
        let task_w = tasks.iter().map(|t| t.len()).max().unwrap_or(20).max(30) + 2;

        let mut header = format!("{:<task_w$}", "Task");
        for s in &skill_names {
            header.push_str(&format!("{:>col_w$}", s));
        }
        println!("{header}");
        println!("{}", "─".repeat(header.len()));

        for task in &tasks {
            let mut row = format!("{:<task_w$}", task);
            for skill in &skill_names {
                let key = (task.clone(), skill.clone());
                let cell = match lookup.get(&key) {
                    None => "—".to_string(),
                    Some(r) => {
                        let resolved = r
                            .get("resolved")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let exit = r
                            .get("exit_code")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        if resolved {
                            let dur = r
                                .get("duration_s")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(0);
                            format!("OK ({dur}s)")
                        } else if exit != 0 {
                            "ERR".to_string()
                        } else {
                            "FAIL".to_string()
                        }
                    }
                };
                row.push_str(&format!("{:>col_w$}", cell));
            }
            println!("{row}");
        }
        println!();
    }

    // Skill deltas
    if let Some(deltas) = summary.get("skill_deltas").and_then(|v| v.as_object()) {
        if !deltas.is_empty() {
            println!("Harness Impact (vs baseline):");
            println!();
            for (skill, delta) in deltas {
                let net = delta
                    .get("net_delta")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let sign = if net >= 0 { "+" } else { "" };
                println!("  {skill}: {sign}{net} net");

                if let Some(uplift) = delta.get("uplift_tasks").and_then(|v| v.as_array()) {
                    if !uplift.is_empty() {
                        let tasks: Vec<&str> =
                            uplift.iter().filter_map(|v| v.as_str()).collect();
                        println!("    Uplift (+): {}", tasks.join(", "));
                    }
                }
                if let Some(regr) = delta.get("regression_tasks").and_then(|v| v.as_array()) {
                    if !regr.is_empty() {
                        let tasks: Vec<&str> =
                            regr.iter().filter_map(|v| v.as_str()).collect();
                        println!("    Regress (-): {}", tasks.join(", "));
                    }
                }
            }
            println!();
        }
    }

    Ok(())
}

fn render_csv(manifest: &Value) -> Result<()> {
    println!("task,skill,resolved,duration_s,exit_code");
    if let Some(runs) = manifest.get("runs").and_then(|v| v.as_array()) {
        for r in runs {
            let task = r.get("task").and_then(|v| v.as_str()).unwrap_or("");
            let skill = r.get("skill").and_then(|v| v.as_str()).unwrap_or("");
            let resolved = r
                .get("resolved")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let dur = r
                .get("duration_s")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let exit = r
                .get("exit_code")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            println!("{task},{skill},{resolved},{dur},{exit}");
        }
    }
    Ok(())
}

fn render_compare(manifest_a: &Value, manifest_b: &Value) -> Result<()> {
    let id_a = manifest_a
        .get("batch_id")
        .and_then(|v| v.as_str())
        .unwrap_or("A");
    let id_b = manifest_b
        .get("batch_id")
        .and_then(|v| v.as_str())
        .unwrap_or("B");

    println!("Comparing: {id_a} vs {id_b}");
    println!();

    let sum_a = manifest_a.get("summary").unwrap_or(&Value::Null);
    let sum_b = manifest_b.get("summary").unwrap_or(&Value::Null);

    let skills_a = sum_a
        .get("per_skill")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let skills_b = sum_b
        .get("per_skill")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    let mut all_skills: Vec<String> = skills_a
        .keys()
        .chain(skills_b.keys())
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    all_skills.sort();

    let hdr = format!(
        "{:<10} {:>10} {:>10} {:>8}",
        "Skill", "Rate (A)", "Rate (B)", "Delta"
    );
    println!("{hdr}");
    println!("{}", "─".repeat(hdr.len()));

    for skill in &all_skills {
        let rate_a = skills_a
            .get(skill)
            .and_then(|s| s.get("resolve_rate"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
            * 100.0;
        let rate_b = skills_b
            .get(skill)
            .and_then(|s| s.get("resolve_rate"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0)
            * 100.0;
        let delta = rate_b - rate_a;
        let sign = if delta >= 0.0 { "+" } else { "" };
        println!(
            "{:<10} {:>9.1}% {:>9.1}% {:>6.1}%",
            skill, rate_a, rate_b, delta
        );
        let _ = sign; // used in formatting concept
    }
    println!();

    // Per-task comparison
    let runs_a_map = build_run_lookup(manifest_a);
    let runs_b_map = build_run_lookup(manifest_b);

    let mut all_keys: Vec<(String, String)> = runs_a_map
        .keys()
        .chain(runs_b_map.keys())
        .cloned()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    all_keys.sort();

    let mut changes = Vec::new();
    for key in &all_keys {
        let ra = runs_a_map.get(key).copied().unwrap_or(false);
        let rb = runs_b_map.get(key).copied().unwrap_or(false);
        if ra != rb {
            let direction = if rb { "GAINED" } else { "LOST" };
            changes.push((&key.0, &key.1, direction));
        }
    }

    if !changes.is_empty() {
        println!("Changes:");
        for (task, skill, direction) in &changes {
            let marker = if *direction == "GAINED" { "+" } else { "-" };
            println!("  {marker} {task} ({skill}): {direction}");
        }
    } else {
        println!("No changes in resolve status between runs.");
    }
    println!();

    Ok(())
}

fn build_run_lookup(manifest: &Value) -> HashMap<(String, String), bool> {
    let mut map = HashMap::new();
    if let Some(runs) = manifest.get("runs").and_then(|v| v.as_array()) {
        for r in runs {
            if let (Some(task), Some(skill)) = (
                r.get("task").and_then(|v| v.as_str()),
                r.get("skill").and_then(|v| v.as_str()),
            ) {
                let resolved = r
                    .get("resolved")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                map.insert((task.to_string(), skill.to_string()), resolved);
            }
        }
    }
    map
}

fn list_all_batches(results_dir: &Path) -> Result<()> {
    let mut batches: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(results_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("batch-") && entry.path().is_dir() {
                let mf = entry.path().join("manifest.json");
                if mf.exists() {
                    batches.push(entry.path());
                }
            }
        }
    }
    batches.sort();

    if batches.is_empty() {
        println!("No batch results found in {}", results_dir.display());
        return Ok(());
    }

    for batch_dir in &batches {
        let manifest = load_manifest(batch_dir)?;
        let batch_id = manifest
            .get("batch_id")
            .and_then(|v| v.as_str())
            .unwrap_or("?");

        let rates: Vec<String> = manifest
            .get("summary")
            .and_then(|s| s.get("per_skill"))
            .and_then(|v| v.as_object())
            .map(|skills| {
                skills
                    .iter()
                    .map(|(name, stats)| {
                        let rate = stats
                            .get("resolve_rate")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);
                        format!("{name}: {:.0}%", rate * 100.0)
                    })
                    .collect()
            })
            .unwrap_or_default();

        println!("{:<30} {}", batch_id, rates.join(" | "));
    }
    println!();

    Ok(())
}
