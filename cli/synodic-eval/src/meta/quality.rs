use crate::score::{EvalVerdict, GroupVerdict, TestResult, TestStatus};

use super::{QualityIssue, QualityIssueKind, QualityReport};

/// Analyze test results for false positives, false negatives, and flakiness.
///
/// This is a post-execution analysis that examines the verdict to identify
/// results that are likely unreliable. It uses structural heuristics rather
/// than re-executing tests, making it fast and deterministic.
pub fn analyze(verdict: &EvalVerdict) -> QualityReport {
    let mut issues = Vec::new();

    // Analyze F2P results
    analyze_f2p_quality(&verdict.f2p, &mut issues);

    // Analyze P2P results
    analyze_p2p_quality(&verdict.p2p, &mut issues);

    // Cross-group analysis
    analyze_cross_group(verdict, &mut issues);

    let false_positive_count = issues
        .iter()
        .filter(|i| i.kind == QualityIssueKind::FalsePositive)
        .count();
    let false_negative_count = issues
        .iter()
        .filter(|i| i.kind == QualityIssueKind::FalseNegative)
        .count();
    let flaky_count = issues
        .iter()
        .filter(|i| i.kind == QualityIssueKind::Flaky)
        .count();

    let confidence_score = compute_confidence(&issues, verdict);

    let summary = build_summary(
        &issues,
        false_positive_count,
        false_negative_count,
        flaky_count,
        confidence_score,
    );

    QualityReport {
        issues,
        false_positive_count,
        false_negative_count,
        flaky_count,
        confidence_score,
        summary,
    }
}

/// Analyze F2P (fail-to-pass) results for quality issues.
fn analyze_f2p_quality(f2p: &GroupVerdict, issues: &mut Vec<QualityIssue>) {
    if f2p.expected.is_empty() {
        return;
    }

    // Check for vacuous passes — all F2P tests pass with zero expected tests listed
    // (This shouldn't happen with well-formed data, but guards against it)

    // Check for "all pass immediately" pattern — suspicious if many F2P tests pass
    // with no failures. Could indicate tests aren't actually testing the right thing.
    if f2p.score.passed == f2p.expected.len() && f2p.expected.len() > 0 {
        // Look for signs that passes might be vacuous
        for result in &f2p.results {
            if result.status == TestStatus::Passed {
                if is_likely_vacuous_pass(result) {
                    issues.push(QualityIssue {
                        kind: QualityIssueKind::FalsePositive,
                        test_name: result.name.clone(),
                        confidence: 0.6,
                        evidence: "F2P test passes but name suggests it may not exercise the changed behavior".into(),
                    });
                }
            }
        }
    }

    // Check for environment-caused F2P failures
    for result in &f2p.results {
        if result.status == TestStatus::Error {
            if let Some(ref reason) = result.reason {
                if is_environment_error(reason) {
                    issues.push(QualityIssue {
                        kind: QualityIssueKind::FalseNegative,
                        test_name: result.name.clone(),
                        confidence: 0.8,
                        evidence: format!(
                            "F2P test errored with environment-related message: {}",
                            truncate(reason, 120)
                        ),
                    });
                }
            }
        }

        if result.status == TestStatus::Failed {
            if let Some(ref reason) = result.reason {
                if is_environment_error(reason) {
                    issues.push(QualityIssue {
                        kind: QualityIssueKind::FalseNegative,
                        test_name: result.name.clone(),
                        confidence: 0.7,
                        evidence: format!(
                            "F2P test failed with environment-related message: {}",
                            truncate(reason, 120)
                        ),
                    });
                }
            }
        }
    }
}

/// Analyze P2P (pass-to-pass) results for quality issues.
fn analyze_p2p_quality(p2p: &GroupVerdict, issues: &mut Vec<QualityIssue>) {
    if p2p.expected.is_empty() || p2p.results.is_empty() {
        return;
    }

    let total = p2p.expected.len();
    let failed_count = p2p.score.failed + p2p.score.errors;

    // Mass P2P failure (>80%) — likely environment issue, not real regressions
    if total > 0 && failed_count as f64 / total as f64 > 0.8 {
        for result in &p2p.results {
            if matches!(result.status, TestStatus::Failed | TestStatus::Error) {
                issues.push(QualityIssue {
                    kind: QualityIssueKind::FalseNegative,
                    test_name: result.name.clone(),
                    confidence: 0.85,
                    evidence: format!(
                        "P2P test failed as part of mass failure ({}/{} failed) — likely environment issue",
                        failed_count, total
                    ),
                });
            }
        }
        return;
    }

    // Individual P2P failures — check for environment patterns
    for result in &p2p.results {
        if matches!(result.status, TestStatus::Failed | TestStatus::Error) {
            if let Some(ref reason) = result.reason {
                if is_environment_error(reason) {
                    issues.push(QualityIssue {
                        kind: QualityIssueKind::FalseNegative,
                        test_name: result.name.clone(),
                        confidence: 0.75,
                        evidence: format!(
                            "P2P test failed with environment-related message: {}",
                            truncate(reason, 120)
                        ),
                    });
                }
            }
        }
    }
}

/// Cross-group analysis: compare F2P and P2P patterns.
fn analyze_cross_group(verdict: &EvalVerdict, issues: &mut Vec<QualityIssue>) {
    // If all F2P pass but all P2P fail → suspicious, likely the "fix" broke the test
    // environment rather than actually fixing the issue
    let f2p_all_pass = verdict.f2p.score.all_pass() || verdict.f2p.expected.is_empty();
    let p2p_total = verdict.p2p.expected.len();
    let p2p_all_fail = p2p_total > 0
        && (verdict.p2p.score.failed + verdict.p2p.score.errors) == p2p_total;

    if f2p_all_pass && p2p_all_fail && p2p_total > 0 {
        issues.push(QualityIssue {
            kind: QualityIssueKind::Anomalous,
            test_name: "(cross-group)".into(),
            confidence: 0.9,
            evidence: format!(
                "All {} F2P tests pass but all {} P2P tests fail — the fix may have broken the test environment rather than solving the problem",
                verdict.f2p.expected.len(),
                p2p_total
            ),
        });
    }

    // If F2P has errors and P2P also has errors with the same pattern → infra issue
    let f2p_error_reasons: Vec<&str> = verdict
        .f2p
        .results
        .iter()
        .filter(|r| r.status == TestStatus::Error)
        .filter_map(|r| r.reason.as_deref())
        .collect();
    let p2p_error_reasons: Vec<&str> = verdict
        .p2p
        .results
        .iter()
        .filter(|r| r.status == TestStatus::Error)
        .filter_map(|r| r.reason.as_deref())
        .collect();

    if !f2p_error_reasons.is_empty() && !p2p_error_reasons.is_empty() {
        // Check for common patterns
        for f2p_reason in &f2p_error_reasons {
            for p2p_reason in &p2p_error_reasons {
                if reasons_share_root_cause(f2p_reason, p2p_reason) {
                    issues.push(QualityIssue {
                        kind: QualityIssueKind::FalseNegative,
                        test_name: "(cross-group)".into(),
                        confidence: 0.85,
                        evidence: format!(
                            "Same error pattern in both F2P and P2P groups — shared infrastructure failure: {}",
                            truncate(f2p_reason, 80)
                        ),
                    });
                    return; // One finding is enough
                }
            }
        }
    }
}

/// Check if a test result looks like a vacuous pass.
///
/// A vacuous pass is one where the test passes but doesn't actually validate
/// the expected behavior. Common patterns:
/// - Test name doesn't relate to the feature being tested
/// - Test is a setup/teardown helper that always passes
fn is_likely_vacuous_pass(result: &TestResult) -> bool {
    let lower = result.name.to_lowercase();

    // Setup/teardown helpers
    if lower.contains("setup") || lower.contains("teardown") || lower.contains("fixture") {
        return true;
    }

    // Tests named "test_init" or "test_default" that just check construction
    if lower.ends_with("test_init") || lower.ends_with("test_default") || lower.ends_with("test_noop") {
        return true;
    }

    false
}

/// Check if an error message indicates an environment/infrastructure issue
/// rather than a real test failure.
fn is_environment_error(reason: &str) -> bool {
    let lower = reason.to_lowercase();

    // Import errors
    if lower.contains("modulenotfounderror")
        || lower.contains("importerror")
        || lower.contains("no module named")
    {
        return true;
    }

    // Connection errors
    if lower.contains("connectionrefused")
        || lower.contains("connection refused")
        || lower.contains("could not connect")
        || lower.contains("timeout")
        || lower.contains("timed out")
    {
        return true;
    }

    // Permission errors
    if lower.contains("permissionerror") || lower.contains("permission denied") {
        return true;
    }

    // File system errors indicating missing setup
    if lower.contains("filenotfounderror")
        || lower.contains("no such file or directory")
        || lower.contains("oserror")
    {
        return true;
    }

    // Database setup errors
    if lower.contains("operationalerror")
        || lower.contains("database")
        || lower.contains("does not exist")
            && (lower.contains("table") || lower.contains("relation"))
    {
        return true;
    }

    // Process/resource errors
    if lower.contains("killed")
        || lower.contains("out of memory")
        || lower.contains("segfault")
        || lower.contains("signal")
    {
        return true;
    }

    false
}

/// Check if two error reasons share a common root cause.
fn reasons_share_root_cause(a: &str, b: &str) -> bool {
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();

    // Same exception type
    let exception_types = [
        "modulenotfounderror",
        "importerror",
        "connectionrefused",
        "permissionerror",
        "filenotfounderror",
        "operationalerror",
        "oserror",
    ];

    for exc in &exception_types {
        if a_lower.contains(exc) && b_lower.contains(exc) {
            return true;
        }
    }

    // Both mention the same missing module
    if a_lower.contains("no module named") && b_lower.contains("no module named") {
        return true;
    }

    false
}

/// Compute overall confidence score (0.0 to 1.0).
fn compute_confidence(issues: &[QualityIssue], _verdict: &EvalVerdict) -> f64 {
    if issues.is_empty() {
        return 1.0;
    }

    let mut confidence = 1.0;

    for issue in issues {
        let penalty = match issue.kind {
            QualityIssueKind::FalsePositive => 0.15 * issue.confidence,
            QualityIssueKind::FalseNegative => 0.1 * issue.confidence,
            QualityIssueKind::Flaky => 0.05 * issue.confidence,
            QualityIssueKind::Anomalous => 0.2 * issue.confidence,
        };
        confidence -= penalty;
    }

    // Floor at 0.0
    if confidence < 0.0 {
        confidence = 0.0;
    }

    confidence
}

fn build_summary(
    issues: &[QualityIssue],
    false_positive_count: usize,
    false_negative_count: usize,
    flaky_count: usize,
    confidence: f64,
) -> String {
    if issues.is_empty() {
        return "No quality issues detected — test results appear reliable".into();
    }

    let mut parts = Vec::new();

    if false_positive_count > 0 {
        parts.push(format!(
            "{} suspected false positive(s)",
            false_positive_count
        ));
    }
    if false_negative_count > 0 {
        parts.push(format!(
            "{} suspected false negative(s)",
            false_negative_count
        ));
    }
    if flaky_count > 0 {
        parts.push(format!("{} suspected flaky test(s)", flaky_count));
    }
    let anomalous_count = issues
        .iter()
        .filter(|i| i.kind == QualityIssueKind::Anomalous)
        .count();
    if anomalous_count > 0 {
        parts.push(format!("{} anomalous pattern(s)", anomalous_count));
    }

    format!(
        "{}. Confidence: {:.0}%",
        parts.join(", "),
        confidence * 100.0
    )
}

fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        &s[..max_len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::*;

    fn make_verdict(
        f2p_results: Vec<TestResult>,
        f2p_expected: Vec<String>,
        p2p_results: Vec<TestResult>,
        p2p_expected: Vec<String>,
    ) -> EvalVerdict {
        let f2p_score = ScoreResult::from_results(&f2p_results);
        let p2p_score = ScoreResult::from_results(&p2p_results);
        let f2p_all_pass = f2p_score.all_pass() || f2p_expected.is_empty();
        let p2p_all_pass = p2p_score.passed == p2p_expected.len();
        EvalVerdict {
            instance_id: "test-instance".into(),
            f2p: GroupVerdict {
                group: TestGroup::F2P,
                expected: f2p_expected,
                results: f2p_results,
                score: f2p_score,
            },
            p2p: GroupVerdict {
                group: TestGroup::P2P,
                expected: p2p_expected,
                results: p2p_results,
                score: p2p_score,
            },
            resolved: f2p_all_pass && p2p_all_pass,
        }
    }

    #[test]
    fn test_clean_verdict_no_issues() {
        let verdict = make_verdict(
            vec![TestResult {
                name: "test_feature".into(),
                status: TestStatus::Passed,
                reason: None,
            }],
            vec!["test_feature".into()],
            vec![TestResult {
                name: "test_existing".into(),
                status: TestStatus::Passed,
                reason: None,
            }],
            vec!["test_existing".into()],
        );
        let report = analyze(&verdict);
        assert_eq!(report.false_positive_count, 0);
        assert_eq!(report.false_negative_count, 0);
        assert_eq!(report.confidence_score, 1.0);
    }

    #[test]
    fn test_f2p_environment_error_detected() {
        let verdict = make_verdict(
            vec![TestResult {
                name: "test_feature".into(),
                status: TestStatus::Error,
                reason: Some("ModuleNotFoundError: No module named 'numpy'".into()),
            }],
            vec!["test_feature".into()],
            vec![],
            vec![],
        );
        let report = analyze(&verdict);
        assert!(report.false_negative_count > 0);
        assert!(report.confidence_score < 1.0);
    }

    #[test]
    fn test_p2p_mass_failure_detected() {
        let p2p_results: Vec<TestResult> = (0..10)
            .map(|i| TestResult {
                name: format!("test_{}", i),
                status: TestStatus::Failed,
                reason: Some("AssertionError".into()),
            })
            .collect();
        let p2p_expected: Vec<String> = (0..10).map(|i| format!("test_{}", i)).collect();

        let verdict = make_verdict(
            vec![TestResult {
                name: "test_fix".into(),
                status: TestStatus::Passed,
                reason: None,
            }],
            vec!["test_fix".into()],
            p2p_results,
            p2p_expected,
        );
        let report = analyze(&verdict);
        // Mass P2P failure should be flagged as false negatives
        assert!(report.false_negative_count > 0);
    }

    #[test]
    fn test_cross_group_anomaly() {
        let verdict = make_verdict(
            vec![TestResult {
                name: "test_fix".into(),
                status: TestStatus::Passed,
                reason: None,
            }],
            vec!["test_fix".into()],
            vec![
                TestResult {
                    name: "test_a".into(),
                    status: TestStatus::Failed,
                    reason: None,
                },
                TestResult {
                    name: "test_b".into(),
                    status: TestStatus::Failed,
                    reason: None,
                },
            ],
            vec!["test_a".into(), "test_b".into()],
        );
        let report = analyze(&verdict);
        let anomalous = report
            .issues
            .iter()
            .filter(|i| i.kind == QualityIssueKind::Anomalous)
            .count();
        assert!(anomalous > 0, "Expected anomalous finding for F2P pass + P2P all fail");
    }

    #[test]
    fn test_vacuous_pass_detected() {
        let verdict = make_verdict(
            vec![TestResult {
                name: "test_setup".into(),
                status: TestStatus::Passed,
                reason: None,
            }],
            vec!["test_setup".into()],
            vec![],
            vec![],
        );
        let report = analyze(&verdict);
        assert!(report.false_positive_count > 0);
    }

    #[test]
    fn test_is_environment_error_patterns() {
        assert!(is_environment_error("ModuleNotFoundError: No module named 'foo'"));
        assert!(is_environment_error("ConnectionRefused: localhost:5432"));
        assert!(is_environment_error("PermissionError: [Errno 13]"));
        assert!(is_environment_error("FileNotFoundError: /etc/config"));
        assert!(is_environment_error("Process timed out after 600s"));
        assert!(!is_environment_error("AssertionError: expected 5 got 3"));
        assert!(!is_environment_error("ValueError: invalid input"));
    }

    #[test]
    fn test_shared_root_cause_detection() {
        assert!(reasons_share_root_cause(
            "ModuleNotFoundError: no module named 'foo'",
            "ModuleNotFoundError: no module named 'bar'"
        ));
        assert!(!reasons_share_root_cause(
            "AssertionError: expected 5",
            "ValueError: invalid"
        ));
    }

    #[test]
    fn test_confidence_degrades_with_issues() {
        let verdict = make_verdict(vec![], vec![], vec![], vec![]);
        let issues = vec![
            QualityIssue {
                kind: QualityIssueKind::FalsePositive,
                test_name: "t1".into(),
                confidence: 1.0,
                evidence: "test".into(),
            },
            QualityIssue {
                kind: QualityIssueKind::FalseNegative,
                test_name: "t2".into(),
                confidence: 1.0,
                evidence: "test".into(),
            },
        ];
        let score = compute_confidence(&issues, &verdict);
        assert!(score < 1.0);
        assert!(score > 0.0);
    }
}
