use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use serde::Serialize;

use super::EvalVerdict;

/// JSON score report matching the Python scorer's output format.
#[derive(Debug, Serialize)]
struct ScoreReport {
    instance_id: String,
    timestamp: String,
    resolved: bool,
    test_format: String,
    f2p: GroupReport,
    p2p: GroupReport,
}

#[derive(Debug, Serialize)]
struct GroupReport {
    total: usize,
    passed: usize,
    failed: usize,
    errors: usize,
    all_pass: bool,
    details: Vec<DetailEntry>,
}

#[derive(Debug, Serialize)]
struct DetailEntry {
    test: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

/// Write a JSON score report from an EvalVerdict.
pub fn write_score_report(verdict: &EvalVerdict, output_path: &Path) -> Result<()> {
    let test_format = if !verdict.f2p.expected.is_empty() {
        let sample = &verdict.f2p.expected[0];
        if sample.contains("::") {
            "pytest"
        } else {
            "django"
        }
    } else if !verdict.p2p.expected.is_empty() {
        let sample = &verdict.p2p.expected[0];
        if sample.contains("::") {
            "pytest"
        } else {
            "django"
        }
    } else {
        "pytest"
    };

    let f2p_all_pass =
        verdict.f2p.score.passed == verdict.f2p.expected.len() && !verdict.f2p.expected.is_empty();
    let p2p_all_pass = verdict.p2p.score.passed == verdict.p2p.expected.len();

    let report = ScoreReport {
        instance_id: verdict.instance_id.clone(),
        timestamp: Utc::now().to_rfc3339(),
        resolved: verdict.resolved,
        test_format: test_format.to_string(),
        f2p: GroupReport {
            total: verdict.f2p.expected.len(),
            passed: verdict.f2p.score.passed,
            failed: verdict.f2p.score.failed,
            errors: verdict.f2p.score.errors,
            all_pass: f2p_all_pass,
            details: verdict
                .f2p
                .results
                .iter()
                .map(|r| DetailEntry {
                    test: r.name.clone(),
                    status: r.status.as_str().to_string(),
                    reason: r.reason.clone(),
                })
                .collect(),
        },
        p2p: GroupReport {
            total: verdict.p2p.expected.len(),
            passed: verdict.p2p.score.passed,
            failed: verdict.p2p.score.failed,
            errors: verdict.p2p.score.errors,
            all_pass: p2p_all_pass,
            details: verdict
                .p2p
                .results
                .iter()
                .map(|r| DetailEntry {
                    test: r.name.clone(),
                    status: r.status.as_str().to_string(),
                    reason: r.reason.clone(),
                })
                .collect(),
        },
    };

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).context("create report directory")?;
    }

    let json = serde_json::to_string_pretty(&report).context("serialize score report")?;

    // Print to stdout for visibility
    println!("{}", json);
    println!();

    std::fs::write(output_path, &json).context("write score report")?;
    eprintln!("Score report: {}", output_path.display());

    Ok(())
}
