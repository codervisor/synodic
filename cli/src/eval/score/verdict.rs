use std::path::Path;

use anyhow::Result;

use super::parser;
use super::runner;
use super::{
    EvalVerdict, GroupVerdict, ScoreResult, TestFramework, TestGroup, TestResult, TestStatus,
};

/// Detect benchmark type from testbed directory contents.
pub fn detect_bench_type(testbed_dir: &Path) -> Result<(String, std::path::PathBuf)> {
    if testbed_dir.join(".swebench").is_dir() {
        Ok(("swebench".into(), testbed_dir.join(".swebench")))
    } else if testbed_dir.join(".featurebench").is_dir() {
        Ok(("featurebench".into(), testbed_dir.join(".featurebench")))
    } else if testbed_dir.join(".devbench").is_dir() {
        Ok(("devbench".into(), testbed_dir.join(".devbench")))
    } else {
        // Fallback: guess from path
        let path_str = testbed_dir.to_string_lossy();
        if path_str.contains("swebench") {
            Ok(("swebench".into(), testbed_dir.join(".swebench")))
        } else {
            Ok(("featurebench".into(), testbed_dir.join(".featurebench")))
        }
    }
}

/// Load test list from a JSON file, handling double-encoding.
fn load_test_list(path: &Path) -> Vec<String> {
    match std::fs::read_to_string(path) {
        Ok(content) => parser::parse_test_list(&content),
        Err(_) => Vec::new(),
    }
}

/// Run test group (F2P or P2P) and produce a GroupVerdict.
fn run_group(
    tests: &[String],
    repo_dir: &Path,
    python: &str,
    framework: &TestFramework,
    group: TestGroup,
) -> Result<GroupVerdict> {
    let label = match group {
        TestGroup::F2P => "f2p",
        TestGroup::P2P => "p2p",
    };

    let results: Vec<TestResult> = match framework {
        TestFramework::Django => runner::run_django_tests(tests, repo_dir, python, label)?,
        TestFramework::Pytest => runner::run_pytest_tests(tests, repo_dir, python, label)?,
    };

    let score = ScoreResult::from_results(&results);

    Ok(GroupVerdict {
        group,
        expected: tests.to_vec(),
        results,
        score,
    })
}

/// Score a completed benchmark run.
///
/// This replaces score.sh + score_runner.py.
pub fn score(
    instance_id: &str,
    testbed_dir: &Path,
    output_path: Option<&Path>,
) -> Result<EvalVerdict> {
    let (bench_type, task_dir) = detect_bench_type(testbed_dir)?;
    let repo_dir = testbed_dir.join("repo");
    let python = runner::resolve_python(testbed_dir);

    let bench_label = match bench_type.as_str() {
        "swebench" => "SWE-bench",
        "featurebench" => "FeatureBench",
        _ => &bench_type,
    };

    eprintln!("=== {} Scoring ===", bench_label);
    eprintln!("Instance: {}", instance_id);
    eprintln!("Testbed:  {}", testbed_dir.display());
    eprintln!();

    // Load test lists
    let f2p_tests = load_test_list(&task_dir.join("fail_to_pass.json"));
    let p2p_tests = load_test_list(&task_dir.join("pass_to_pass.json"));

    eprintln!("F2P tests: {}", f2p_tests.len());
    eprintln!("P2P tests: {}", p2p_tests.len());
    eprintln!();

    // Detect test format
    let all_tests: Vec<String> = f2p_tests
        .iter()
        .chain(p2p_tests.iter())
        .cloned()
        .collect();
    let framework = parser::detect_test_format(&all_tests);
    eprintln!("Test format: {:?}", framework);
    eprintln!();

    // Run F2P tests
    eprintln!("[1/2] Running F2P tests (fail-to-pass)...");
    eprintln!("  These tests must PASS after your implementation.");
    eprintln!();
    let f2p = run_group(&f2p_tests, &repo_dir, &python, &framework, TestGroup::F2P)?;
    eprintln!(
        "  F2P: {}/{} passed, {} failed, {} errors",
        f2p.score.passed,
        f2p.expected.len(),
        f2p.score.failed,
        f2p.score.errors
    );
    eprintln!();

    // Run P2P tests
    eprintln!("[2/2] Running P2P tests (pass-to-pass)...");
    eprintln!("  These tests must STILL PASS after your implementation.");
    eprintln!();
    let p2p = run_group(&p2p_tests, &repo_dir, &python, &framework, TestGroup::P2P)?;
    eprintln!(
        "  P2P: {}/{} passed, {} failed, {} errors",
        p2p.score.passed,
        p2p.expected.len(),
        p2p.score.failed,
        p2p.score.errors
    );
    eprintln!();

    // Compute verdict
    let f2p_all_pass = f2p.score.passed == f2p.expected.len() && !f2p.expected.is_empty();
    let p2p_all_pass = p2p.score.passed == p2p.expected.len();
    let resolved = f2p_all_pass && p2p_all_pass;

    let verdict = EvalVerdict {
        instance_id: instance_id.to_string(),
        f2p,
        p2p,
        resolved,
    };

    // Write report
    let output_path = output_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| task_dir.join("score_report.json"));

    super::report::write_score_report(&verdict, &output_path)?;

    // Print verdict
    eprintln!("=== Final Verdict ===");
    if resolved {
        eprintln!("RESOLVED — All F2P and P2P tests pass.");
    } else {
        eprintln!("FAILED");
        if !f2p_all_pass {
            eprintln!(
                "  F2P: {}/{} (need all to pass)",
                verdict.f2p.score.passed,
                verdict.f2p.expected.len()
            );
        }
        if !p2p_all_pass {
            eprintln!(
                "  P2P: {}/{} (regressions detected)",
                verdict.p2p.score.passed,
                verdict.p2p.expected.len()
            );
        }
    }

    Ok(verdict)
}

/// Score a completed Synodic dogfood eval run using `cargo test`.
///
/// Synodic dogfood scoring is simpler than SWE-bench/FeatureBench:
/// - Run `cargo test` in `testbed/repo/<score_dir>`
/// - If exit code 0: resolved = true (all tests pass)
/// - If exit code non-zero: resolved = false (some tests fail)
pub fn score_synodic(
    instance_id: &str,
    testbed_dir: &Path,
    output_path: Option<&Path>,
) -> Result<EvalVerdict> {
    use std::process::Command;
    use anyhow::{bail, Context};

    let task_dir = testbed_dir.join(".synodic");
    let meta_file = task_dir.join("meta.json");

    if !meta_file.exists() {
        bail!(
            "Synodic testbed metadata not found: {}. Run setup first.",
            meta_file.display()
        );
    }

    let meta_content = std::fs::read_to_string(&meta_file).context("read meta.json")?;
    let meta: serde_json::Value =
        serde_json::from_str(&meta_content).context("parse meta.json")?;
    let score_dir = meta["score_dir"].as_str().context("meta.score_dir")?;

    let repo_dir = testbed_dir.join("repo");
    let cargo_dir = repo_dir.join(score_dir);

    eprintln!("=== Synodic Dogfood Scoring ===");
    eprintln!("Instance: {}", instance_id);
    eprintln!("Score dir: {}", cargo_dir.display());
    eprintln!();

    eprintln!("[1/1] Running cargo test...");
    eprintln!();

    let output = Command::new("cargo")
        .args(["test"])
        .current_dir(&cargo_dir)
        .output()
        .context("run cargo test")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{}\n{}", stdout, stderr);

    eprintln!("{}", combined);

    let resolved = output.status.success();

    // Parse test counts from cargo output
    let (passed, failed) = parse_cargo_test_counts(&combined);

    // Build a synthetic EvalVerdict for the synodic benchmark
    // (no F2P/P2P distinction — just "all tests pass")
    let score = ScoreResult {
        passed,
        failed,
        errors: 0,
        skipped: 0,
    };

    let f2p = GroupVerdict {
        group: TestGroup::F2P,
        expected: vec![],
        results: vec![TestResult {
            name: "cargo test (all)".into(),
            status: if resolved {
                TestStatus::Passed
            } else {
                TestStatus::Failed
            },
            reason: if resolved {
                None
            } else {
                Some(format!(
                    "cargo test failed with exit code {}",
                    output.status.code().unwrap_or(-1)
                ))
            },
        }],
        score: score.clone(),
    };

    let p2p = GroupVerdict {
        group: TestGroup::P2P,
        expected: vec![],
        results: vec![],
        score: ScoreResult {
            passed: 0,
            failed: 0,
            errors: 0,
            skipped: 0,
        },
    };

    let verdict = EvalVerdict {
        instance_id: instance_id.to_string(),
        f2p,
        p2p,
        resolved,
    };

    let output_path = output_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| task_dir.join("score_report.json"));

    super::report::write_score_report(&verdict, &output_path)?;

    eprintln!("=== Final Verdict ===");
    if resolved {
        eprintln!("RESOLVED — All cargo tests pass ({} passed).", passed);
    } else {
        eprintln!("FAILED — cargo test exited with errors ({} failed).", failed);
    }

    Ok(verdict)
}

/// Parse passed/failed counts from `cargo test` output.
///
/// Cargo test outputs lines like:
///   `test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; ...`
fn parse_cargo_test_counts(output: &str) -> (usize, usize) {
    let mut total_passed = 0usize;
    let mut total_failed = 0usize;

    for line in output.lines() {
        if line.contains("test result:") {
            // Parse "N passed" and "N failed"
            if let Some(p) = extract_count(line, "passed") {
                total_passed += p;
            }
            if let Some(f) = extract_count(line, "failed") {
                total_failed += f;
            }
        }
    }

    (total_passed, total_failed)
}

/// Extract a number before a keyword (e.g. "29 passed" → 29).
fn extract_count(line: &str, keyword: &str) -> Option<usize> {
    let idx = line.find(keyword)?;
    let before = line[..idx].trim_end();
    let num_str = before.rsplit(|c: char| !c.is_ascii_digit()).next()?;
    num_str.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cargo_test_counts_all_pass() {
        let output = "test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s";
        let (passed, failed) = parse_cargo_test_counts(output);
        assert_eq!(passed, 31);
        assert_eq!(failed, 0);
    }

    #[test]
    fn test_parse_cargo_test_counts_with_failures() {
        let output = "test result: FAILED. 28 passed; 3 failed; 0 ignored; 0 measured;";
        let (passed, failed) = parse_cargo_test_counts(output);
        assert_eq!(passed, 28);
        assert_eq!(failed, 3);
    }

    #[test]
    fn test_parse_cargo_test_counts_empty() {
        let (passed, failed) = parse_cargo_test_counts("");
        assert_eq!(passed, 0);
        assert_eq!(failed, 0);
    }

    #[test]
    fn test_parse_cargo_test_counts_multiple_suites() {
        let output = "test result: ok. 10 passed; 0 failed; 0 ignored;\ntest result: ok. 5 passed; 0 failed; 0 ignored;";
        let (passed, failed) = parse_cargo_test_counts(output);
        assert_eq!(passed, 15);
        assert_eq!(failed, 0);
    }
}
