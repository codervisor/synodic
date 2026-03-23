use std::path::Path;

use crate::score::parser;

use super::{GranularityBreakdown, RiskFactor, Severity, TestGranularity, TestStrategy};

/// Analyze the testing strategy for a benchmark evaluation.
///
/// Examines test lists, detects framework, classifies test granularity,
/// and identifies risk factors that could affect test reliability.
pub fn analyze(
    benchmark: &str,
    testbed_dir: &Path,
) -> TestStrategy {
    let task_dir = testbed_dir.join(format!(".{}", benchmark));

    // Load test lists
    let f2p_tests = load_tests(&task_dir.join("fail_to_pass.json"));
    let p2p_tests = load_tests(&task_dir.join("pass_to_pass.json"));

    let all_tests: Vec<String> = f2p_tests
        .iter()
        .chain(p2p_tests.iter())
        .cloned()
        .collect();

    // Detect framework
    let framework = if all_tests.is_empty() {
        match benchmark {
            "synodic" => "cargo".to_string(),
            _ => "unknown".to_string(),
        }
    } else {
        let detected = parser::detect_test_format(&all_tests);
        format!("{:?}", detected)
    };

    // Classify test granularity
    let granularity = classify_granularity(&all_tests);

    // Identify risk factors
    let risk_factors = identify_risks(benchmark, &f2p_tests, &p2p_tests, &granularity);

    // Generate recommendations
    let recommendations = generate_recommendations(benchmark, &f2p_tests, &p2p_tests, &risk_factors);

    TestStrategy {
        framework,
        f2p_count: f2p_tests.len(),
        p2p_count: p2p_tests.len(),
        granularity,
        risk_factors,
        recommendations,
    }
}

fn load_tests(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|content| parser::parse_test_list(&content))
        .unwrap_or_default()
}

/// Classify each test name into a granularity bucket.
///
/// Heuristics:
/// - Tests with "integration" or that cross module boundaries → Integration
/// - Tests with "e2e", "end_to_end", "functional", "acceptance" → EndToEnd
/// - Tests in a single class/module with specific method names → Unit
/// - Everything else → Unknown
fn classify_granularity(tests: &[String]) -> GranularityBreakdown {
    let mut unit = 0;
    let mut integration = 0;
    let mut end_to_end = 0;
    let mut unknown = 0;

    for test in tests {
        let granularity = classify_single_test(test);
        match granularity {
            TestGranularity::Unit => unit += 1,
            TestGranularity::Integration => integration += 1,
            TestGranularity::EndToEnd => end_to_end += 1,
            TestGranularity::Unknown => unknown += 1,
        }
    }

    GranularityBreakdown {
        unit,
        integration,
        end_to_end,
        unknown,
    }
}

fn classify_single_test(test_name: &str) -> TestGranularity {
    let lower = test_name.to_lowercase();

    // E2E / functional test indicators
    if lower.contains("e2e")
        || lower.contains("end_to_end")
        || lower.contains("end2end")
        || lower.contains("functional")
        || lower.contains("acceptance")
        || lower.contains("selenium")
        || lower.contains("browser")
    {
        return TestGranularity::EndToEnd;
    }

    // Integration test indicators
    if lower.contains("integration")
        || lower.contains("integ_")
        || lower.contains("_integ")
    {
        return TestGranularity::Integration;
    }

    // Pytest-style path-based tests: multiple "::" separators suggest module scoping
    if test_name.contains("::") {
        let parts: Vec<&str> = test_name.split("::").collect();
        if parts.len() >= 3 {
            // tests/path.py::Class::test_method — likely unit
            return TestGranularity::Unit;
        }
        if parts.len() == 2 {
            // tests/path.py::test_function — could be either
            let path = parts[0].to_lowercase();
            if path.contains("integration") {
                return TestGranularity::Integration;
            }
            return TestGranularity::Unit;
        }
    }

    // Django-style: test_method (module.Class) — typically unit
    if lower.contains('(') && lower.contains(')') {
        return TestGranularity::Unit;
    }

    // Single test function name — likely unit
    if lower.starts_with("test_") {
        return TestGranularity::Unit;
    }

    TestGranularity::Unknown
}

fn identify_risks(
    benchmark: &str,
    f2p_tests: &[String],
    p2p_tests: &[String],
    granularity: &GranularityBreakdown,
) -> Vec<RiskFactor> {
    let mut risks = Vec::new();

    // No F2P tests — nothing to validate
    if f2p_tests.is_empty() && benchmark != "synodic" {
        risks.push(RiskFactor {
            name: "no_f2p_tests".into(),
            severity: Severity::Error,
            description: "No fail-to-pass tests defined. Cannot verify if the fix is correct.".into(),
        });
    }

    // Very few F2P tests — weak signal
    if f2p_tests.len() == 1 {
        risks.push(RiskFactor {
            name: "single_f2p_test".into(),
            severity: Severity::Warning,
            description: "Only 1 F2P test — single point of failure for correctness validation. A vacuous pass is more likely.".into(),
        });
    }

    // No P2P tests — no regression safety net
    if p2p_tests.is_empty() && benchmark != "synodic" {
        risks.push(RiskFactor {
            name: "no_p2p_tests".into(),
            severity: Severity::Warning,
            description: "No pass-to-pass tests. Regressions in unrelated code won't be caught.".into(),
        });
    }

    // Disproportionate P2P to F2P ratio
    if !f2p_tests.is_empty() && p2p_tests.len() > f2p_tests.len() * 50 {
        risks.push(RiskFactor {
            name: "p2p_heavy".into(),
            severity: Severity::Info,
            description: format!(
                "P2P/F2P ratio is {}:1 — large P2P suite may dominate execution time",
                p2p_tests.len() / f2p_tests.len()
            ),
        });
    }

    // High unknown granularity — we can't reason about the tests
    let total = granularity.unit + granularity.integration + granularity.end_to_end + granularity.unknown;
    if total > 0 && granularity.unknown as f64 / total as f64 > 0.5 {
        risks.push(RiskFactor {
            name: "unclassifiable_tests".into(),
            severity: Severity::Info,
            description: format!(
                "{}/{} tests could not be classified by granularity — test naming may be non-standard",
                granularity.unknown,
                total
            ),
        });
    }

    // F2P and P2P test overlap — same test in both lists
    let f2p_set: std::collections::HashSet<&str> =
        f2p_tests.iter().map(|s| s.as_str()).collect();
    let overlap: Vec<&str> = p2p_tests
        .iter()
        .filter(|t| f2p_set.contains(t.as_str()))
        .map(|t| t.as_str())
        .collect();
    if !overlap.is_empty() {
        risks.push(RiskFactor {
            name: "f2p_p2p_overlap".into(),
            severity: Severity::Error,
            description: format!(
                "{} test(s) appear in both F2P and P2P lists — contradictory expectations will produce false results",
                overlap.len()
            ),
        });
    }

    risks
}

fn generate_recommendations(
    benchmark: &str,
    f2p_tests: &[String],
    p2p_tests: &[String],
    risks: &[RiskFactor],
) -> Vec<String> {
    let mut recs = Vec::new();

    for risk in risks {
        match risk.name.as_str() {
            "no_f2p_tests" => {
                recs.push(
                    "Add fail-to-pass tests that exercise the expected behavior change".into(),
                );
            }
            "single_f2p_test" => {
                recs.push(
                    "Consider adding more F2P tests to increase confidence in the fix".into(),
                );
            }
            "f2p_p2p_overlap" => {
                recs.push(
                    "Remove overlapping tests from one of the lists to avoid contradictory expectations".into(),
                );
            }
            _ => {}
        }
    }

    // Benchmark-specific recommendations
    if benchmark == "synodic" && f2p_tests.is_empty() && p2p_tests.is_empty() {
        recs.push("Synodic dogfood uses cargo test — ensure test targets are configured".into());
    }

    recs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_pytest_unit() {
        assert_eq!(
            classify_single_test("tests/test_parser.py::TestParser::test_parse_django"),
            TestGranularity::Unit
        );
    }

    #[test]
    fn test_classify_pytest_integration() {
        assert_eq!(
            classify_single_test("tests/integration/test_api.py::test_full_flow"),
            TestGranularity::Integration
        );
    }

    #[test]
    fn test_classify_e2e() {
        assert_eq!(
            classify_single_test("tests/e2e/test_browser_login.py::test_login_flow"),
            TestGranularity::EndToEnd
        );
    }

    #[test]
    fn test_classify_django_style() {
        assert_eq!(
            classify_single_test("test_create_user (auth.tests.UserTests)"),
            TestGranularity::Unit
        );
    }

    #[test]
    fn test_classify_unknown() {
        assert_eq!(
            classify_single_test("some_random_identifier"),
            TestGranularity::Unknown
        );
    }

    #[test]
    fn test_granularity_breakdown() {
        let tests = vec![
            "tests/test_a.py::TestA::test_method".into(),
            "tests/integration/test_b.py::test_flow".into(),
            "tests/e2e/test_c.py::test_browser".into(),
            "random_name".into(),
        ];
        let breakdown = classify_granularity(&tests);
        assert_eq!(breakdown.unit, 1);
        assert_eq!(breakdown.integration, 1);
        assert_eq!(breakdown.end_to_end, 1);
        assert_eq!(breakdown.unknown, 1);
    }

    #[test]
    fn test_identify_risk_no_f2p() {
        let risks = identify_risks(
            "swebench",
            &[],
            &["test_bar".into()],
            &GranularityBreakdown {
                unit: 0,
                integration: 0,
                end_to_end: 0,
                unknown: 1,
            },
        );
        assert!(risks.iter().any(|r| r.name == "no_f2p_tests"));
    }

    #[test]
    fn test_identify_risk_overlap() {
        let risks = identify_risks(
            "swebench",
            &["test_foo".into()],
            &["test_foo".into(), "test_bar".into()],
            &GranularityBreakdown {
                unit: 2,
                integration: 0,
                end_to_end: 0,
                unknown: 0,
            },
        );
        assert!(risks.iter().any(|r| r.name == "f2p_p2p_overlap"));
    }

    #[test]
    fn test_identify_risk_single_f2p() {
        let risks = identify_risks(
            "featurebench",
            &["test_only".into()],
            &["test_a".into(), "test_b".into()],
            &GranularityBreakdown {
                unit: 3,
                integration: 0,
                end_to_end: 0,
                unknown: 0,
            },
        );
        assert!(risks.iter().any(|r| r.name == "single_f2p_test"));
    }

    #[test]
    fn test_synodic_no_f2p_risk_suppressed() {
        let risks = identify_risks(
            "synodic",
            &[],
            &[],
            &GranularityBreakdown {
                unit: 0,
                integration: 0,
                end_to_end: 0,
                unknown: 0,
            },
        );
        // synodic doesn't require F2P/P2P lists
        assert!(!risks.iter().any(|r| r.name == "no_f2p_tests"));
        assert!(!risks.iter().any(|r| r.name == "no_p2p_tests"));
    }
}
