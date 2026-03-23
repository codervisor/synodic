pub mod environment;
pub mod quality;
pub mod strategy;

use serde::{Deserialize, Serialize};


// ── Environment validation ──────────────────────────────────────────

/// Severity of an environment check finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Severity {
    /// Blocks test execution entirely.
    Error,
    /// May cause unreliable results.
    Warning,
    /// Informational only.
    Info,
}

/// A single environment check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvCheck {
    pub name: String,
    pub passed: bool,
    pub severity: Severity,
    pub message: String,
}

/// Overall environment readiness assessment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentReport {
    pub checks: Vec<EnvCheck>,
    pub ready: bool,
    pub blocking_count: usize,
    pub warning_count: usize,
}

impl EnvironmentReport {
    pub fn from_checks(checks: Vec<EnvCheck>) -> Self {
        let blocking_count = checks
            .iter()
            .filter(|c| !c.passed && c.severity == Severity::Error)
            .count();
        let warning_count = checks
            .iter()
            .filter(|c| !c.passed && c.severity == Severity::Warning)
            .count();
        let ready = blocking_count == 0;
        EnvironmentReport {
            checks,
            ready,
            blocking_count,
            warning_count,
        }
    }
}

// ── Testing strategy ────────────────────────────────────────────────

/// Granularity classification of a test.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TestGranularity {
    Unit,
    Integration,
    EndToEnd,
    Unknown,
}

/// A recommended testing strategy for an eval run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestStrategy {
    /// Detected test framework.
    pub framework: String,
    /// Total tests in F2P group.
    pub f2p_count: usize,
    /// Total tests in P2P group.
    pub p2p_count: usize,
    /// Granularity breakdown.
    pub granularity: GranularityBreakdown,
    /// Risk assessment for this test configuration.
    pub risk_factors: Vec<RiskFactor>,
    /// Recommended actions before running tests.
    pub recommendations: Vec<String>,
}

/// Breakdown of test granularity across the suite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GranularityBreakdown {
    pub unit: usize,
    pub integration: usize,
    pub end_to_end: usize,
    pub unknown: usize,
}

/// A risk factor that could affect test reliability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactor {
    pub name: String,
    pub severity: Severity,
    pub description: String,
}

// ── Test quality analysis ───────────────────────────────────────────

/// Classification of a test quality issue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QualityIssueKind {
    /// Test passes but likely doesn't validate anything meaningful.
    FalsePositive,
    /// Test fails but likely due to environment, not code.
    FalseNegative,
    /// Test shows signs of non-determinism.
    Flaky,
    /// Test result doesn't match the expected group behavior.
    Anomalous,
}

/// A single test quality issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityIssue {
    pub kind: QualityIssueKind,
    pub test_name: String,
    pub confidence: f64,
    pub evidence: String,
}

/// Overall test quality analysis report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityReport {
    pub issues: Vec<QualityIssue>,
    pub false_positive_count: usize,
    pub false_negative_count: usize,
    pub flaky_count: usize,
    pub confidence_score: f64,
    pub summary: String,
}

// ── Combined meta-testing report ────────────────────────────────────

/// Full meta-testing report combining all analyses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaReport {
    pub environment: Option<EnvironmentReport>,
    pub strategy: Option<TestStrategy>,
    pub quality: Option<QualityReport>,
    pub overall_confidence: f64,
    pub actionable_findings: Vec<String>,
}

impl MetaReport {
    /// Compute overall confidence from sub-reports.
    pub fn compute(
        environment: Option<EnvironmentReport>,
        strategy: Option<TestStrategy>,
        quality: Option<QualityReport>,
    ) -> Self {
        let mut confidence = 1.0;
        let mut findings = Vec::new();

        if let Some(ref env) = environment {
            if !env.ready {
                confidence *= 0.0;
                findings.push(format!(
                    "Environment not ready: {} blocking issue(s)",
                    env.blocking_count
                ));
            } else if env.warning_count > 0 {
                confidence *= 0.8;
                findings.push(format!(
                    "Environment has {} warning(s) that may affect reliability",
                    env.warning_count
                ));
            }
        }

        if let Some(ref strat) = strategy {
            for risk in &strat.risk_factors {
                match risk.severity {
                    Severity::Error => {
                        confidence *= 0.5;
                        findings.push(format!("High risk: {}", risk.description));
                    }
                    Severity::Warning => {
                        confidence *= 0.85;
                        findings.push(format!("Moderate risk: {}", risk.description));
                    }
                    Severity::Info => {}
                }
            }
            findings.extend(strat.recommendations.clone());
        }

        if let Some(ref qual) = quality {
            confidence *= qual.confidence_score;
            if qual.false_positive_count > 0 {
                findings.push(format!(
                    "{} suspected false positive(s) — tests may pass without validating correctness",
                    qual.false_positive_count
                ));
            }
            if qual.false_negative_count > 0 {
                findings.push(format!(
                    "{} suspected false negative(s) — failures may be environmental, not code issues",
                    qual.false_negative_count
                ));
            }
        }

        MetaReport {
            environment,
            strategy,
            quality,
            overall_confidence: confidence,
            actionable_findings: findings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_report_all_pass() {
        let checks = vec![
            EnvCheck {
                name: "python".into(),
                passed: true,
                severity: Severity::Error,
                message: "Python 3.10 found".into(),
            },
            EnvCheck {
                name: "testbed".into(),
                passed: true,
                severity: Severity::Error,
                message: "Testbed exists".into(),
            },
        ];
        let report = EnvironmentReport::from_checks(checks);
        assert!(report.ready);
        assert_eq!(report.blocking_count, 0);
    }

    #[test]
    fn test_environment_report_blocking_failure() {
        let checks = vec![
            EnvCheck {
                name: "python".into(),
                passed: false,
                severity: Severity::Error,
                message: "Python not found".into(),
            },
        ];
        let report = EnvironmentReport::from_checks(checks);
        assert!(!report.ready);
        assert_eq!(report.blocking_count, 1);
    }

    #[test]
    fn test_environment_report_warning_only() {
        let checks = vec![
            EnvCheck {
                name: "python".into(),
                passed: true,
                severity: Severity::Error,
                message: "OK".into(),
            },
            EnvCheck {
                name: "venv".into(),
                passed: false,
                severity: Severity::Warning,
                message: "No virtualenv found".into(),
            },
        ];
        let report = EnvironmentReport::from_checks(checks);
        assert!(report.ready);
        assert_eq!(report.warning_count, 1);
    }

    #[test]
    fn test_meta_report_compute_all_good() {
        let env = EnvironmentReport {
            checks: vec![],
            ready: true,
            blocking_count: 0,
            warning_count: 0,
        };
        let report = MetaReport::compute(Some(env), None, None);
        assert_eq!(report.overall_confidence, 1.0);
        assert!(report.actionable_findings.is_empty());
    }

    #[test]
    fn test_meta_report_compute_env_not_ready() {
        let env = EnvironmentReport {
            checks: vec![],
            ready: false,
            blocking_count: 2,
            warning_count: 0,
        };
        let report = MetaReport::compute(Some(env), None, None);
        assert_eq!(report.overall_confidence, 0.0);
        assert!(!report.actionable_findings.is_empty());
    }

    #[test]
    fn test_meta_report_compute_with_quality_issues() {
        let quality = QualityReport {
            issues: vec![],
            false_positive_count: 2,
            false_negative_count: 1,
            flaky_count: 0,
            confidence_score: 0.7,
            summary: "Some issues detected".into(),
        };
        let report = MetaReport::compute(None, None, Some(quality));
        assert!((report.overall_confidence - 0.7).abs() < 0.001);
        assert!(report.actionable_findings.len() >= 2);
    }
}
