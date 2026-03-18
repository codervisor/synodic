use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde_json::json;

use super::score;

/// Options for a single eval run.
pub struct RunOptions {
    pub alias: String,
    pub skill: String,
    pub testbed_dir: Option<String>,
    pub skip_setup: bool,
    pub skip_agent: bool,
    pub agent_cmd: String,
    pub output: Option<String>,
    pub dry_run: bool,
    pub split: String,
    pub repo_root: PathBuf,
}

/// Resolved benchmark target.
#[derive(Debug, Clone)]
pub struct Target {
    pub benchmark: String,
    pub instance_id: String,
}

/// Resolve a benchmark alias (e.g. "fb:mlflow-tracing") to a Target.
pub fn resolve_target(raw: &str) -> Result<Target> {
    let mut benchmark = String::new();
    let mut raw = raw.to_string();

    // Check for benchmark prefix
    if let Some(rest) = raw.strip_prefix("fb:") {
        benchmark = "featurebench".into();
        raw = rest.to_string();
    } else if let Some(rest) = raw.strip_prefix("swe:") {
        benchmark = "swebench".into();
        raw = rest.to_string();
    } else if let Some(rest) = raw.strip_prefix("dev:") {
        benchmark = "devbench".into();
        raw = rest.to_string();
    } else if let Some(rest) = raw.strip_prefix("syn:") {
        benchmark = "synodic".into();
        raw = rest.to_string();
    }

    // Resolve aliases
    let mut aliases: HashMap<&str, (&str, &str)> = HashMap::new();
    // FeatureBench aliases
    aliases.insert("mlflow-tracing", ("featurebench", "mlflow__mlflow.93dab383.test_trace.17fde8b0.lv1"));
    aliases.insert("mlflow", ("featurebench", "mlflow__mlflow.93dab383.test_trace.17fde8b0.lv1"));
    aliases.insert("sympy-nullspace", ("featurebench", "sympy__sympy.c1097516.test_nullspace.f14fc970.lv1"));
    aliases.insert("sympy", ("featurebench", "sympy__sympy.c1097516.test_nullspace.f14fc970.lv1"));
    aliases.insert("seaborn-regr", ("featurebench", "mwaskom__seaborn.7001ebe7.test_regression.ce8c62e2.lv1"));
    aliases.insert("seaborn", ("featurebench", "mwaskom__seaborn.7001ebe7.test_regression.ce8c62e2.lv1"));
    // SWE-bench Verified aliases
    aliases.insert("django-10097", ("swebench", "django__django-10097"));
    // SWE-bench Pro aliases
    aliases.insert("qutebrowser-f91ace", ("swebench", "instance_qutebrowser__qutebrowser-f91ace96223cac8161c16dd061907e138fe85111-v059c6fdc75567943479b23ebca7c07b5e9a7f34c"));
    aliases.insert("ansible-f327e6", ("swebench", "instance_ansible__ansible-f327e65d11bb905ed9f15996024f857a95592629-vba6da65a0f3baefda7a058ebbd0a8dcafb8512f5"));
    aliases.insert("teleport-3fa690", ("swebench", "instance_gravitational__teleport-3fa6904377c006497169945428e8197158667910-v626ec2a48416b10a88641359a169d99e935ff037"));

    let instance_id = if let Some((default_bench, id)) = aliases.get(raw.as_str()) {
        if benchmark.is_empty() {
            benchmark = default_bench.to_string();
        }
        id.to_string()
    } else {
        raw.clone()
    };

    // Auto-detect benchmark from instance ID format if not specified
    if benchmark.is_empty() {
        if instance_id.starts_with("instance_") {
            benchmark = "swebench".into();
        } else if instance_id.matches('.').count() >= 4 {
            benchmark = "featurebench".into();
        } else if instance_id.contains("__") && instance_id.contains('-') {
            benchmark = "swebench".into();
        } else {
            benchmark = "devbench".into();
        }
    }

    Ok(Target {
        benchmark,
        instance_id,
    })
}

/// Execute a full eval run: setup → agent → score.
///
/// This replaces run.sh.
pub fn execute(opts: RunOptions) -> Result<()> {
    let target = resolve_target(&opts.alias)?;

    let testbed_dir = opts
        .testbed_dir
        .clone()
        .unwrap_or_else(|| {
            format!(
                "/tmp/{}-testbed/{}",
                target.benchmark, target.instance_id
            )
        });
    let testbed_path = PathBuf::from(&testbed_dir);
    let task_dir = testbed_path.join(format!(".{}", target.benchmark));
    let repo_dir = testbed_path.join("repo");

    // Header
    let bench_label = match target.benchmark.as_str() {
        "featurebench" => "FeatureBench".to_string(),
        "swebench" => format!("SWE-bench ({})", opts.split),
        "devbench" => "DevBench".to_string(),
        other => other.to_string(),
    };
    let skill_label = match opts.skill.as_str() {
        "fractal" => "Fractal Decomposition",
        "factory" => "Factory (BUILD → INSPECT)",
        "baseline" => "Baseline (no skill)",
        other => other,
    };

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║          E2E Eval — Synodic Harness                        ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("Benchmark: {}", bench_label);
    println!("Skill:     {}", skill_label);
    println!("Instance:  {}", target.instance_id);
    println!("Testbed:   {}", testbed_dir);
    println!();

    let run_start = std::time::Instant::now();

    // --- Phase 1: Setup ---
    if !opts.skip_setup {
        println!("━━━ Phase 1: Testbed Setup ━━━");
        super::setup::run_setup(
            &target.benchmark,
            &target.instance_id,
            &testbed_dir,
            &opts.skill,
            &opts.split,
            &opts.repo_root,
        )?;
    } else {
        println!("━━━ Phase 1: Testbed Setup (skipped) ━━━");
        if !task_dir.exists() {
            bail!(
                "Testbed not found at {}. Run without --skip-setup first.",
                testbed_dir
            );
        }
    }
    println!();

    // --- Phase 2: Agent Invocation ---
    let prompt_file = task_dir.join("agent_prompt.md");

    if opts.dry_run {
        println!("━━━ Phase 2: Agent Prompt (dry run) ━━━");
        println!();
        let prompt = std::fs::read_to_string(&prompt_file)
            .context("read agent prompt")?;
        println!("--- BEGIN PROMPT ({} chars) ---", prompt.len());
        println!("{}", prompt);
        println!("--- END PROMPT ---");
        println!();
        println!("To run manually:");
        println!("  cd {}", repo_dir.display());
        println!(
            "  cat {} | {} --print -",
            prompt_file.display(),
            opts.agent_cmd
        );
        return Ok(());
    }

    if !opts.skip_agent {
        println!("━━━ Phase 2: Agent Invocation ━━━");
        println!();
        println!("Starting agent in testbed repo...");
        println!("  Agent command: {}", opts.agent_cmd);
        println!("  Skill:         {}", skill_label);
        println!("  Working dir:   {}", repo_dir.display());

        if prompt_file.exists() {
            let prompt_size = std::fs::metadata(&prompt_file)
                .map(|m| m.len())
                .unwrap_or(0);
            println!("  Prompt:        {} ({} chars)", prompt_file.display(), prompt_size);
        }
        println!();

        let start = std::time::Instant::now();

        let prompt_content = std::fs::read_to_string(&prompt_file)
            .context("read agent prompt")?;

        let agent_log = task_dir.join("agent_output.log");

        let status = Command::new(&opts.agent_cmd)
            .args(["--print", "--allowedTools", "Edit Write Bash Read Glob Grep Agent"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(&repo_dir)
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(ref mut stdin) = child.stdin {
                    let _ = stdin.write_all(prompt_content.as_bytes());
                }
                child.wait_with_output()
            });

        match status {
            Ok(output) => {
                let combined = format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                println!("{}", combined);
                let _ = std::fs::write(&agent_log, &combined);
            }
            Err(e) => {
                eprintln!("WARNING: Agent command '{}' failed: {}", opts.agent_cmd, e);
                eprintln!("Please run the agent manually:");
                eprintln!();
                eprintln!("  cd {}", repo_dir.display());
                eprintln!("  cat {} | claude", prompt_file.display());
                eprintln!();
                eprintln!("Then re-run with --skip-agent to score.");
                bail!("Agent invocation failed");
            }
        }

        let elapsed = start.elapsed().as_secs();
        println!();
        println!("Agent completed in {}s", elapsed);
        println!();
    } else {
        println!("━━━ Phase 2: Agent Invocation (skipped) ━━━");
    }
    println!();

    // --- Phase 3: Scoring ---
    println!("━━━ Phase 3: Scoring ━━━");
    println!();

    let verdict = match target.benchmark.as_str() {
        "featurebench" | "swebench" => {
            let output_path = opts.output.as_ref().map(PathBuf::from);
            Some(score::verdict::score(
                &target.instance_id,
                &testbed_path,
                output_path.as_deref(),
            )?)
        }
        "devbench" => {
            // DevBench scoring uses a separate script for now
            let score_script = opts.repo_root.join("evals/score-devbench.sh");
            if score_script.exists() {
                let mut args = vec![target.instance_id.clone(), "--testbed-dir".into(), testbed_dir.clone()];
                if let Some(ref output) = opts.output {
                    args.push("--output".into());
                    args.push(output.clone());
                }
                crate::util::exec_script(&score_script, &args)?;
            } else {
                bail!("DevBench scoring not yet ported to Rust; evals/score-devbench.sh not found");
            }
            None
        }
        "synodic" => {
            Some(score::verdict::score_synodic(
                &target.instance_id,
                &testbed_path,
                opts.output.as_ref().map(PathBuf::from).as_deref(),
            )?)
        }
        other => bail!("Unknown benchmark type: {}", other),
    };

    // --- Governance Log ---
    let duration_s = run_start.elapsed().as_secs();
    append_governance_log(
        &opts.repo_root,
        &target,
        &opts.skill,
        &opts.split,
        duration_s,
        verdict.as_ref(),
    );

    println!();
    println!("━━━ Done ━━━");
    Ok(())
}

/// Extract categorized, synthesized findings from a verdict.
///
/// Instead of dumping every failed test, this groups failures by pattern and
/// produces actionable summaries that feed cross-run learning (HARNESS.md §6–7).
fn extract_findings(verdict: &score::EvalVerdict) -> (Vec<serde_json::Value>, &'static str) {
    let mut findings: Vec<serde_json::Value> = Vec::new();
    let mut category = "resolved";

    // --- F2P analysis: did the agent solve the task? ---
    let f2p_failed: Vec<&score::TestResult> = verdict
        .f2p
        .results
        .iter()
        .filter(|r| matches!(r.status, score::TestStatus::Failed))
        .collect();
    let f2p_errors: Vec<&score::TestResult> = verdict
        .f2p
        .results
        .iter()
        .filter(|r| matches!(r.status, score::TestStatus::Error))
        .collect();

    if !f2p_failed.is_empty() {
        category = "correctness";
        // Collect distinct failure reasons
        let reasons: Vec<&str> = f2p_failed
            .iter()
            .filter_map(|r| r.reason.as_deref())
            .filter(|s| !s.is_empty())
            .collect();
        let unique_reasons: Vec<&str> = {
            let mut seen = std::collections::HashSet::new();
            reasons.into_iter().filter(|r| seen.insert(*r)).collect()
        };

        findings.push(json!({
            "category": "correctness",
            "summary": format!(
                "{}/{} F2P tests still failing — agent patch incomplete",
                f2p_failed.len(),
                verdict.f2p.expected.len()
            ),
            "tests": f2p_failed.iter().map(|r| &r.name).collect::<Vec<_>>(),
            "reasons": unique_reasons,
        }));
    }

    if !f2p_errors.is_empty() {
        if category == "resolved" {
            category = "infrastructure";
        }
        let reasons: Vec<&str> = f2p_errors
            .iter()
            .filter_map(|r| r.reason.as_deref())
            .filter(|s| !s.is_empty())
            .collect();

        findings.push(json!({
            "category": "infrastructure",
            "summary": format!(
                "{}/{} F2P tests errored — test environment broken",
                f2p_errors.len(),
                verdict.f2p.expected.len()
            ),
            "tests": f2p_errors.iter().map(|r| &r.name).collect::<Vec<_>>(),
            "reasons": reasons,
        }));
    }

    // --- P2P analysis: did the agent break existing behavior? ---
    let p2p_failed: Vec<&score::TestResult> = verdict
        .p2p
        .results
        .iter()
        .filter(|r| matches!(r.status, score::TestStatus::Failed))
        .collect();
    let p2p_errors: Vec<&score::TestResult> = verdict
        .p2p
        .results
        .iter()
        .filter(|r| matches!(r.status, score::TestStatus::Error))
        .collect();
    let p2p_total = verdict.p2p.expected.len();

    if !p2p_failed.is_empty() {
        if category != "correctness" {
            category = "regression";
        }
        let ratio = if p2p_total > 0 {
            p2p_failed.len() as f64 / p2p_total as f64
        } else {
            0.0
        };

        // Synthesize: bulk failure vs selective regression
        let summary = if ratio > 0.8 {
            format!(
                "{}/{} P2P tests failed — likely environment/setup issue, not selective regression",
                p2p_failed.len(),
                p2p_total
            )
        } else {
            // Group by test file to find the blast radius
            let mut by_file: HashMap<&str, usize> = HashMap::new();
            for r in &p2p_failed {
                let file = r.name.split("::").next().unwrap_or(&r.name);
                *by_file.entry(file).or_default() += 1;
            }
            let hotspots: Vec<String> = by_file
                .iter()
                .map(|(f, n)| format!("{} ({})", f, n))
                .collect();
            format!(
                "{}/{} P2P regressions in: {}",
                p2p_failed.len(),
                p2p_total,
                hotspots.join(", ")
            )
        };

        let reasons: Vec<&str> = p2p_failed
            .iter()
            .filter_map(|r| r.reason.as_deref())
            .filter(|s| !s.is_empty())
            .collect();
        let unique_reasons: Vec<&str> = {
            let mut seen = std::collections::HashSet::new();
            reasons.into_iter().filter(|r| seen.insert(*r)).take(5).collect()
        };

        findings.push(json!({
            "category": if ratio > 0.8 { "infrastructure" } else { "regression" },
            "summary": summary,
            "failed_count": p2p_failed.len(),
            "total_count": p2p_total,
            "reasons": unique_reasons,
        }));
    }

    if !p2p_errors.is_empty() {
        if category == "resolved" {
            category = "infrastructure";
        }
        let reasons: Vec<&str> = p2p_errors
            .iter()
            .filter_map(|r| r.reason.as_deref())
            .filter(|s| !s.is_empty())
            .collect();

        findings.push(json!({
            "category": "infrastructure",
            "summary": format!(
                "{}/{} P2P tests errored — test harness broken",
                p2p_errors.len(),
                p2p_total
            ),
            "reasons": reasons,
        }));
    }

    (findings, category)
}

/// Append a governance record to `.harness/eval.governance.jsonl`.
///
/// Best-effort — failures are logged but do not abort the eval.
/// Unlike score_report.json (raw test data), this captures categorized findings
/// that feed the cross-run learning substrate (HARNESS.md §6–7).
fn append_governance_log(
    repo_root: &PathBuf,
    target: &Target,
    skill: &str,
    split: &str,
    duration_s: u64,
    verdict: Option<&score::EvalVerdict>,
) {
    let harness_dir = repo_root.join(".harness");
    if std::fs::create_dir_all(&harness_dir).is_err() {
        return;
    }

    let (findings, category, resolved) = match verdict {
        Some(v) => {
            let (findings, cat) = extract_findings(v);
            (findings, cat, v.resolved)
        }
        None => (vec![], "unknown", false),
    };

    let record = json!({
        "work_id": format!("eval-{}-{}-{}", target.instance_id, skill, Utc::now().timestamp()),
        "source": "eval",
        "timestamp": Utc::now().to_rfc3339(),
        "status": if resolved { "resolved" } else { category },
        "instance_id": target.instance_id,
        "benchmark": target.benchmark,
        "split": split,
        "skill": skill,
        "resolved": resolved,
        "findings": findings,
        "metrics": {
            "duration_s": duration_s,
        }
    });

    let gov_log = harness_dir.join("eval.governance.jsonl");
    let result = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&gov_log)
        .and_then(|mut f| writeln!(f, "{}", serde_json::to_string(&record).unwrap_or_default()));

    match result {
        Ok(_) => eprintln!("Governance log: {}", gov_log.display()),
        Err(e) => eprintln!("WARNING: Could not write governance log: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_featurebench_alias() {
        let target = resolve_target("fb:mlflow-tracing").unwrap();
        assert_eq!(target.benchmark, "featurebench");
        assert_eq!(
            target.instance_id,
            "mlflow__mlflow.93dab383.test_trace.17fde8b0.lv1"
        );
    }

    #[test]
    fn test_resolve_swebench_alias() {
        let target = resolve_target("swe:django-10097").unwrap();
        assert_eq!(target.benchmark, "swebench");
        assert_eq!(target.instance_id, "django__django-10097");
    }

    #[test]
    fn test_resolve_devbench_alias() {
        let target = resolve_target("dev:TextCNN").unwrap();
        assert_eq!(target.benchmark, "devbench");
        assert_eq!(target.instance_id, "TextCNN");
    }

    #[test]
    fn test_resolve_bare_alias() {
        let target = resolve_target("mlflow-tracing").unwrap();
        assert_eq!(target.benchmark, "featurebench");
        assert_eq!(
            target.instance_id,
            "mlflow__mlflow.93dab383.test_trace.17fde8b0.lv1"
        );
    }

    #[test]
    fn test_resolve_literal_swebench_id() {
        let target = resolve_target("swe:django__django-16379").unwrap();
        assert_eq!(target.benchmark, "swebench");
        assert_eq!(target.instance_id, "django__django-16379");
    }

    #[test]
    fn test_resolve_auto_detect_featurebench() {
        let target = resolve_target("org__repo.commit.test.hash.lv1").unwrap();
        assert_eq!(target.benchmark, "featurebench");
    }

    #[test]
    fn test_resolve_auto_detect_swebench_pro() {
        let target = resolve_target("instance_org__repo-abc123-v456").unwrap();
        assert_eq!(target.benchmark, "swebench");
    }

    #[test]
    fn test_resolve_auto_detect_devbench() {
        let target = resolve_target("TextCNN").unwrap();
        assert_eq!(target.benchmark, "devbench");
    }

    #[test]
    fn test_resolve_synodic_alias() {
        let target = resolve_target("syn:dogfood-syn-support").unwrap();
        assert_eq!(target.benchmark, "synodic");
        assert_eq!(target.instance_id, "dogfood-syn-support");
    }

    #[test]
    fn test_resolve_synodic_prefix_arbitrary() {
        let target = resolve_target("syn:some-instance").unwrap();
        assert_eq!(target.benchmark, "synodic");
        assert_eq!(target.instance_id, "some-instance");
    }
}
