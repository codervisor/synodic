use std::path::Path;

use anyhow::Result;

use super::parser;
use super::runner;
use super::{
    EvalVerdict, GroupVerdict, ScoreResult, TestFramework, TestGroup, TestResult,
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
