use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

use chrono::Utc;
use serde_json::json;
use synodic_eval::run::EvalResult;
use synodic_eval::score;

/// Record an eval result to the governance log at `.harness/eval.governance.jsonl`.
///
/// Best-effort — failures are logged but do not abort.
pub fn record_eval_result(repo_root: &Path, result: &EvalResult) {
    let harness_dir = repo_root.join(".harness");
    if std::fs::create_dir_all(&harness_dir).is_err() {
        return;
    }

    let (findings, category) = match &result.verdict {
        Some(v) => extract_findings(v),
        None => (vec![], "unknown"),
    };

    let record = json!({
        "work_id": format!("eval-{}-{}-{}", result.target.instance_id, result.skill, Utc::now().timestamp()),
        "source": "eval",
        "timestamp": Utc::now().to_rfc3339(),
        "status": if result.resolved { "resolved" } else { category },
        "instance_id": result.target.instance_id,
        "benchmark": result.target.benchmark,
        "split": result.split,
        "skill": result.skill,
        "resolved": result.resolved,
        "findings": findings,
        "metrics": {
            "duration_s": result.duration_s,
        }
    });

    let gov_log = harness_dir.join("eval.governance.jsonl");
    let write_result = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&gov_log)
        .and_then(|mut f| writeln!(f, "{}", serde_json::to_string(&record).unwrap_or_default()));

    match write_result {
        Ok(_) => eprintln!("Governance log: {}", gov_log.display()),
        Err(e) => eprintln!("WARNING: Could not write governance log: {}", e),
    }
}

/// Extract categorized, synthesized findings from a verdict.
///
/// Groups failures by pattern and produces actionable summaries.
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

        let summary = if ratio > 0.8 {
            format!(
                "{}/{} P2P tests failed — likely environment/setup issue, not selective regression",
                p2p_failed.len(),
                p2p_total
            )
        } else {
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
