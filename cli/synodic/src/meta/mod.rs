pub mod consult;
pub mod execute;
pub mod validate;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Configuration for the meta-testing pipeline.
#[allow(dead_code)]
pub struct MetaConfig {
    /// Working directory (project root).
    pub workdir: PathBuf,
    /// Git diff or description of changes to test.
    pub diff: Option<String>,
    /// Spec or feature description (if available).
    pub spec: Option<String>,
    /// AI agent command for meta-testing consultations.
    pub agent_cmd: String,
    /// Maximum rework cycles for test validation.
    pub max_rework: u32,
    /// Minimal output.
    pub quiet: bool,
    /// Machine-readable JSON output.
    pub json_output: bool,
    /// Dry run — show plan without executing.
    pub dry_run: bool,
}

/// A test plan produced by the AI consultant.
///
/// The AI analyzes the project, the feature requirements, and the available
/// tools/frameworks to produce a structured plan that can be executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestPlan {
    /// High-level summary of the testing approach.
    pub strategy: String,
    /// Detected or recommended test framework(s).
    pub frameworks: Vec<String>,
    /// Individual test proposals.
    pub tests: Vec<TestProposal>,
    /// Rationale for the chosen approach.
    pub rationale: String,
    /// Setup commands needed before running tests.
    pub setup_commands: Vec<String>,
    /// Run commands to execute the test suite.
    pub run_commands: Vec<String>,
}

/// A single test that the AI proposes to write or use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestProposal {
    /// What this test validates.
    pub description: String,
    /// File path where the test should live.
    pub file_path: String,
    /// The actual test code to write.
    pub code: String,
    /// What kind of test: unit, integration, e2e, property, etc.
    pub kind: String,
    /// What a pass means / what a failure means.
    pub pass_criteria: String,
}

/// Result of executing a test plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestExecution {
    /// The plan that was executed.
    pub plan: TestPlan,
    /// Setup phase output.
    pub setup_output: String,
    /// Whether setup succeeded.
    pub setup_ok: bool,
    /// Raw test output (stdout + stderr).
    pub test_output: String,
    /// Exit code of the test run.
    pub exit_code: i32,
    /// Number of tests that passed.
    pub passed: usize,
    /// Number of tests that failed.
    pub failed: usize,
}

/// AI validation of test results — assessing reliability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Overall confidence that results reflect real behavior (0.0 to 1.0).
    pub confidence: f64,
    /// Per-test assessments.
    pub assessments: Vec<TestAssessment>,
    /// Summary judgment from the AI.
    pub summary: String,
    /// Recommended next actions.
    pub next_actions: Vec<String>,
}

/// AI assessment of an individual test's reliability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestAssessment {
    /// The test being assessed.
    pub test_description: String,
    /// Whether the AI considers the result reliable.
    pub reliable: bool,
    /// Classification if unreliable.
    pub concern: Option<String>,
    /// AI's reasoning.
    pub reasoning: String,
}

/// Full result of the meta-testing pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaResult {
    pub plan: TestPlan,
    pub execution: Option<TestExecution>,
    pub validation: Option<ValidationReport>,
    pub status: String,
}

/// Run the full meta-testing pipeline: consult → implement → execute → validate.
pub fn run(config: &MetaConfig, run_dir: &Path) -> anyhow::Result<MetaResult> {
    log_info(config, "━━━ Meta-Testing: AI Consultant ━━━");
    log_info(config, "");

    // Phase 1: AI consults on the project and produces a test plan
    log_info(config, "Phase 1: Analyzing project and proposing test strategy...");
    let plan = consult::analyze(config, run_dir)?;

    log_info(config, &format!("  Strategy: {}", plan.strategy));
    log_info(config, &format!("  Frameworks: {}", plan.frameworks.join(", ")));
    log_info(config, &format!("  Tests proposed: {}", plan.tests.len()));
    for (i, test) in plan.tests.iter().enumerate() {
        log_info(config, &format!("    {}. [{}] {}", i + 1, test.kind, test.description));
    }
    log_info(config, "");

    if config.dry_run {
        log_info(config, "DRY RUN — would execute:");
        log_info(config, &format!("  Setup: {} command(s)", plan.setup_commands.len()));
        log_info(config, &format!("  Tests: {} file(s) to write", plan.tests.len()));
        log_info(config, &format!("  Run: {} command(s)", plan.run_commands.len()));
        return Ok(MetaResult {
            plan,
            execution: None,
            validation: None,
            status: "dry_run".into(),
        });
    }

    // Phase 2: Write test files and execute them
    log_info(config, "Phase 2: Implementing and executing tests...");
    let execution = execute::run_plan(config, &plan, run_dir)?;

    log_info(config, &format!("  Setup: {}", if execution.setup_ok { "OK" } else { "FAILED" }));
    log_info(config, &format!("  Results: {} passed, {} failed (exit {})",
        execution.passed, execution.failed, execution.exit_code));
    log_info(config, "");

    // Phase 3: AI validates the results
    log_info(config, "Phase 3: Validating test result reliability...");
    let validation = validate::assess(config, &execution, run_dir)?;

    log_info(config, &format!("  Confidence: {:.0}%", validation.confidence * 100.0));
    log_info(config, &format!("  Summary: {}", validation.summary));
    for assessment in &validation.assessments {
        if !assessment.reliable {
            let concern = assessment.concern.as_deref().unwrap_or("unknown");
            log_info(config, &format!("  ⚠ {} — {}", concern, assessment.test_description));
        }
    }
    if !validation.next_actions.is_empty() {
        log_info(config, "");
        log_info(config, "  Recommended next actions:");
        for action in &validation.next_actions {
            log_info(config, &format!("    - {}", action));
        }
    }
    log_info(config, "");

    let status = if validation.confidence >= 0.7 && execution.exit_code == 0 {
        "passed"
    } else if validation.confidence < 0.5 {
        "unreliable"
    } else {
        "failed"
    };

    Ok(MetaResult {
        plan,
        execution: Some(execution),
        validation: Some(validation),
        status: status.into(),
    })
}

fn log_info(config: &MetaConfig, msg: &str) {
    if !config.quiet {
        if msg.is_empty() {
            eprintln!();
        } else {
            eprintln!("meta: {msg}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_plan_serialization() {
        let plan = TestPlan {
            strategy: "Unit testing with pytest".into(),
            frameworks: vec!["pytest".into()],
            tests: vec![TestProposal {
                description: "Test that parse_config handles empty input".into(),
                file_path: "tests/test_config.py".into(),
                code: "def test_empty(): assert parse_config('') == {}".into(),
                kind: "unit".into(),
                pass_criteria: "Returns empty dict for empty string".into(),
            }],
            rationale: "Project uses pytest for existing tests".into(),
            setup_commands: vec!["pip install pytest".into()],
            run_commands: vec!["pytest tests/".into()],
        };
        let json = serde_json::to_string(&plan).unwrap();
        let parsed: TestPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tests.len(), 1);
        assert_eq!(parsed.frameworks[0], "pytest");
    }

    #[test]
    fn test_meta_result_status() {
        let result = MetaResult {
            plan: TestPlan {
                strategy: "test".into(),
                frameworks: vec![],
                tests: vec![],
                rationale: "test".into(),
                setup_commands: vec![],
                run_commands: vec![],
            },
            execution: None,
            validation: None,
            status: "dry_run".into(),
        };
        assert_eq!(result.status, "dry_run");
    }

    #[test]
    fn test_validation_report_serialization() {
        let report = ValidationReport {
            confidence: 0.85,
            assessments: vec![TestAssessment {
                test_description: "test_parse".into(),
                reliable: true,
                concern: None,
                reasoning: "Tests actual parsing logic".into(),
            }],
            summary: "Tests appear reliable".into(),
            next_actions: vec![],
        };
        let json = serde_json::to_string(&report).unwrap();
        let parsed: ValidationReport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.confidence, 0.85);
        assert!(parsed.assessments[0].reliable);
    }
}
