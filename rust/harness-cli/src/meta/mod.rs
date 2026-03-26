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
    /// Maximum rework cycles for test infrastructure debugging.
    pub max_rework: u32,
    /// Minimal output.
    pub quiet: bool,
    /// Machine-readable JSON output.
    pub json_output: bool,
    /// Dry run — show plan without executing.
    pub dry_run: bool,
}

// ── Test Plan ───────────────────────────────────────────────────────

/// A test plan produced by the AI consultant.
///
/// The plan is tiered: the AI proposes a layered strategy from fast/cheap
/// (smoke/unit) to slow/expensive (integration/e2e), each tier building
/// confidence. This mirrors real-world testing where you don't start with
/// full e2e — you build up.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestPlan {
    /// High-level summary of the testing approach.
    pub strategy: String,
    /// Detected or recommended test framework(s).
    pub frameworks: Vec<String>,
    /// Rationale for the chosen approach.
    pub rationale: String,
    /// Tiered test layers, from cheapest to most expensive.
    pub tiers: Vec<TestTier>,
    /// Infrastructure requirements (databases, containers, services).
    pub infrastructure: Vec<InfraRequirement>,
    /// Teardown commands to clean up after all testing.
    pub teardown_commands: Vec<String>,
    /// Known risks and mitigations the AI identified.
    pub risks: Vec<String>,
}

/// A testing tier — a layer in the testing pyramid.
///
/// Each tier is self-contained: it has its own setup, tests, and run commands.
/// Tiers execute in order. If an earlier tier fails, later tiers may be skipped
/// (why run e2e if unit tests don't pass?).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestTier {
    /// Tier name: "smoke", "unit", "integration", "e2e", "property", etc.
    pub name: String,
    /// Why this tier exists and what confidence it provides.
    pub purpose: String,
    /// Setup commands specific to this tier.
    pub setup_commands: Vec<String>,
    /// Run commands to execute this tier's tests.
    pub run_commands: Vec<String>,
    /// Test files to write for this tier.
    pub tests: Vec<TestProposal>,
    /// Whether to continue to the next tier if this one fails.
    pub continue_on_failure: bool,
}

/// A single test that the AI proposes to write.
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

/// An infrastructure requirement for testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfraRequirement {
    /// What is needed: "postgres", "redis", "docker", "mock-server", etc.
    pub name: String,
    /// How to provision it.
    pub setup_command: String,
    /// How to verify it's ready.
    pub health_check: String,
    /// How to tear it down.
    pub teardown_command: String,
}

// ── Execution ───────────────────────────────────────────────────────

/// Result of executing a single test tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierExecution {
    /// Which tier was executed.
    pub tier_name: String,
    /// Setup output.
    pub setup_output: String,
    /// Whether setup succeeded.
    pub setup_ok: bool,
    /// Raw test output.
    pub test_output: String,
    /// Exit code.
    pub exit_code: i32,
    /// Parsed pass count.
    pub passed: usize,
    /// Parsed fail count.
    pub failed: usize,
}

/// Result of executing the full test plan across all tiers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestExecution {
    /// The plan that was executed.
    pub plan: TestPlan,
    /// Per-tier results.
    pub tiers: Vec<TierExecution>,
    /// Infrastructure setup output.
    pub infra_output: String,
    /// Whether infrastructure was provisioned successfully.
    pub infra_ok: bool,
    /// Total passed across all tiers.
    pub total_passed: usize,
    /// Total failed across all tiers.
    pub total_failed: usize,
    /// How many rework iterations were needed to get tests running.
    pub rework_iterations: u32,
}

// ── Diagnosis (rework loop) ─────────────────────────────────────────

/// Diagnosis from the AI after a failed execution attempt.
///
/// This is the key to handling real-world complexity. When tests fail to
/// run (not fail assertions — fail to even execute), the AI diagnoses the
/// root cause and proposes fixes to the test plan itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnosis {
    /// What went wrong.
    pub root_cause: String,
    /// Classification of the failure.
    pub failure_kind: FailureKind,
    /// Proposed fixes to the test plan.
    pub fixes: Vec<PlanFix>,
    /// Whether the AI believes the plan is fundamentally sound (just needs
    /// infrastructure fixes) or needs a complete rethink.
    pub salvageable: bool,
}

/// Classification of why tests failed to run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FailureKind {
    /// Missing dependency or tool.
    MissingDependency,
    /// Infrastructure not available (database, service, container).
    InfrastructureUnavailable,
    /// Wrong framework or tool version.
    VersionMismatch,
    /// Test code has errors (syntax, import paths, etc.).
    TestCodeError,
    /// Environment misconfiguration (paths, env vars, permissions).
    EnvironmentConfig,
    /// The approach itself is wrong — need to rethink.
    WrongApproach,
}

/// A proposed fix to the test plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanFix {
    /// What to fix.
    pub description: String,
    /// Which tier this applies to (or "infrastructure" / "global").
    pub target: String,
    /// The fix: new commands, modified test code, etc.
    pub action: FixAction,
}

/// What kind of fix to apply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FixAction {
    /// Add or replace a setup command.
    SetupCommand { command: String },
    /// Replace a test file's code.
    ReplaceTest { file_path: String, new_code: String },
    /// Add an infrastructure requirement.
    AddInfra(InfraRequirement),
    /// Replace the run command for a tier.
    ReplaceRunCommand { tier: String, command: String },
    /// Remove a test that can't work in this environment.
    RemoveTest { file_path: String, reason: String },
}

// ── Validation ──────────────────────────────────────────────────────

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

// ── Full result ─────────────────────────────────────────────────────

/// Full result of the meta-testing pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaResult {
    pub plan: TestPlan,
    pub execution: Option<TestExecution>,
    pub validation: Option<ValidationReport>,
    pub status: String,
    /// Diagnosis history from rework iterations.
    pub diagnosis_history: Vec<Diagnosis>,
}

// ── Pipeline orchestration ──────────────────────────────────────────

/// Run the full meta-testing pipeline with iterative rework.
///
/// The pipeline is NOT linear. It's a loop:
///
/// ```text
/// consult → implement → execute ─┬→ validate → done
///                                 │
///                            (failure?)
///                                 │
///                            diagnose → revise plan → re-execute
///                                 │
///                            (max rework?)
///                                 │
///                            escalate with partial results
/// ```
pub fn run(config: &MetaConfig, run_dir: &Path) -> anyhow::Result<MetaResult> {
    log_info(config, "━━━ Meta-Testing: AI Consultant ━━━");
    log_info(config, "");

    // Phase 1: AI consults on the project and produces a test plan
    log_info(
        config,
        "Phase 1: Analyzing project and proposing test strategy...",
    );
    let mut plan = consult::analyze(config, run_dir)?;

    log_info(config, &format!("  Strategy: {}", plan.strategy));
    log_info(
        config,
        &format!("  Frameworks: {}", plan.frameworks.join(", ")),
    );
    log_info(config, &format!("  Tiers: {}", plan.tiers.len()));
    for (i, tier) in plan.tiers.iter().enumerate() {
        let test_count: usize = tier.tests.len();
        log_info(
            config,
            &format!(
                "    {}. {} — {} test(s) [{}]",
                i + 1,
                tier.name,
                test_count,
                tier.purpose
            ),
        );
    }
    if !plan.infrastructure.is_empty() {
        log_info(
            config,
            &format!(
                "  Infrastructure: {}",
                plan.infrastructure
                    .iter()
                    .map(|i| i.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        );
    }
    if !plan.risks.is_empty() {
        log_info(config, "  Risks:");
        for risk in &plan.risks {
            log_info(config, &format!("    - {risk}"));
        }
    }
    log_info(config, "");

    if config.dry_run {
        log_info(config, "DRY RUN — would execute:");
        for tier in &plan.tiers {
            log_info(
                config,
                &format!(
                    "  Tier '{}': {} setup cmd(s), {} test(s), {} run cmd(s)",
                    tier.name,
                    tier.setup_commands.len(),
                    tier.tests.len(),
                    tier.run_commands.len()
                ),
            );
        }
        return Ok(MetaResult {
            plan,
            execution: None,
            validation: None,
            status: "dry_run".into(),
            diagnosis_history: vec![],
        });
    }

    // Phase 2+3: Execute with rework loop
    let mut diagnosis_history = Vec::new();
    let mut iteration = 0u32;
    let max_iterations = config.max_rework + 1;

    let execution = loop {
        iteration += 1;
        log_info(
            config,
            &format!(
                "Phase 2: Implementing and executing tests (attempt {}/{})",
                iteration, max_iterations
            ),
        );

        let exec = execute::run_plan(config, &plan, run_dir, iteration)?;

        // Log per-tier results
        for tier_exec in &exec.tiers {
            let status = if !tier_exec.setup_ok {
                "SETUP FAILED"
            } else if tier_exec.exit_code == 0 {
                "PASS"
            } else {
                "FAIL"
            };
            log_info(
                config,
                &format!(
                    "  Tier '{}': {} ({} passed, {} failed)",
                    tier_exec.tier_name, status, tier_exec.passed, tier_exec.failed
                ),
            );
        }
        log_info(config, "");

        // Decide: are the results good enough to validate, or do we need rework?
        let needs_rework = execution_needs_rework(&exec);

        if !needs_rework || iteration >= max_iterations {
            if needs_rework && iteration >= max_iterations {
                log_info(
                    config,
                    &format!(
                        "  Rework limit reached ({} attempts). Proceeding with partial results.",
                        max_iterations
                    ),
                );
            }
            break exec;
        }

        // Phase 2.5: Diagnose and revise
        log_info(config, "  Tests failed to run properly. Diagnosing...");
        let diagnosis = consult::diagnose(config, &plan, &exec, run_dir, iteration)?;

        log_info(
            config,
            &format!(
                "  Root cause: {} ({:?})",
                diagnosis.root_cause, diagnosis.failure_kind
            ),
        );
        log_info(config, &format!("  Salvageable: {}", diagnosis.salvageable));
        for fix in &diagnosis.fixes {
            log_info(
                config,
                &format!("  Fix: {} [{}]", fix.description, fix.target),
            );
        }
        log_info(config, "");

        if !diagnosis.salvageable {
            log_info(
                config,
                "  AI determined the approach needs fundamental rethinking.",
            );
            log_info(config, "  Re-consulting with failure context...");
            log_info(config, "");

            // Full re-consult with the failure as context
            plan = consult::reconsult(config, &diagnosis, &exec, run_dir)?;
        } else {
            // Apply incremental fixes to the plan
            plan = apply_fixes(plan, &diagnosis.fixes);
        }

        diagnosis_history.push(diagnosis);
    };

    // Phase 3: Validate results
    log_info(config, "Phase 3: Validating test result reliability...");
    let validation = validate::assess(config, &execution, run_dir)?;

    log_info(
        config,
        &format!("  Confidence: {:.0}%", validation.confidence * 100.0),
    );
    log_info(config, &format!("  Summary: {}", validation.summary));
    for assessment in &validation.assessments {
        if !assessment.reliable {
            let concern = assessment.concern.as_deref().unwrap_or("unknown");
            log_info(
                config,
                &format!("    [{concern}] {}", assessment.test_description),
            );
        }
    }
    if !validation.next_actions.is_empty() {
        log_info(config, "  Recommended next actions:");
        for action in &validation.next_actions {
            log_info(config, &format!("    - {action}"));
        }
    }
    log_info(config, "");

    let status = if validation.confidence >= 0.7 && execution.total_failed == 0 {
        "passed"
    } else if validation.confidence < 0.5 {
        "unreliable"
    } else {
        "failed"
    };

    if !diagnosis_history.is_empty() {
        log_info(
            config,
            &format!(
                "  ({} rework iteration(s) were needed to stabilize test infrastructure)",
                diagnosis_history.len()
            ),
        );
    }

    Ok(MetaResult {
        plan,
        execution: Some(execution),
        validation: Some(validation),
        status: status.into(),
        diagnosis_history,
    })
}

/// Determine if an execution result needs rework (infrastructure/setup failure)
/// vs. legitimate test failures that should go to validation.
fn execution_needs_rework(exec: &TestExecution) -> bool {
    // Infrastructure didn't come up
    if !exec.infra_ok {
        return true;
    }

    // Any tier failed to even set up
    for tier in &exec.tiers {
        if !tier.setup_ok {
            return true;
        }
    }

    // All tiers have zero passes and non-zero exits — tests likely didn't run at all
    let any_ran = exec.tiers.iter().any(|t| t.passed > 0 || t.failed > 0);
    let all_errored = exec.tiers.iter().all(|t| t.exit_code != 0);
    if !any_ran && all_errored && !exec.tiers.is_empty() {
        return true;
    }

    false
}

/// Apply incremental fixes to a test plan.
fn apply_fixes(mut plan: TestPlan, fixes: &[PlanFix]) -> TestPlan {
    for fix in fixes {
        match &fix.action {
            FixAction::SetupCommand { command } => {
                // Add to the targeted tier or globally
                if fix.target == "infrastructure" || fix.target == "global" {
                    // Add as first infra setup
                    plan.infrastructure.push(InfraRequirement {
                        name: fix.description.clone(),
                        setup_command: command.clone(),
                        health_check: String::new(),
                        teardown_command: String::new(),
                    });
                } else {
                    for tier in &mut plan.tiers {
                        if tier.name == fix.target {
                            tier.setup_commands.push(command.clone());
                        }
                    }
                }
            }
            FixAction::ReplaceTest {
                file_path,
                new_code,
            } => {
                for tier in &mut plan.tiers {
                    for test in &mut tier.tests {
                        if test.file_path == *file_path {
                            test.code = new_code.clone();
                        }
                    }
                }
            }
            FixAction::AddInfra(infra) => {
                plan.infrastructure.push(infra.clone());
            }
            FixAction::ReplaceRunCommand { tier, command } => {
                for t in &mut plan.tiers {
                    if t.name == *tier {
                        t.run_commands = vec![command.clone()];
                    }
                }
            }
            FixAction::RemoveTest { file_path, .. } => {
                for tier in &mut plan.tiers {
                    tier.tests.retain(|t| t.file_path != *file_path);
                }
            }
        }
    }
    plan
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

    fn sample_plan() -> TestPlan {
        TestPlan {
            strategy: "Tiered testing".into(),
            frameworks: vec!["pytest".into()],
            rationale: "Python project".into(),
            tiers: vec![
                TestTier {
                    name: "unit".into(),
                    purpose: "Fast correctness checks".into(),
                    setup_commands: vec![],
                    run_commands: vec!["pytest tests/unit/".into()],
                    tests: vec![TestProposal {
                        description: "Test parse".into(),
                        file_path: "tests/unit/test_parse.py".into(),
                        code: "def test_parse(): assert True".into(),
                        kind: "unit".into(),
                        pass_criteria: "Parses correctly".into(),
                    }],
                    continue_on_failure: false,
                },
                TestTier {
                    name: "integration".into(),
                    purpose: "Cross-component validation".into(),
                    setup_commands: vec!["docker-compose up -d".into()],
                    run_commands: vec!["pytest tests/integration/".into()],
                    tests: vec![],
                    continue_on_failure: false,
                },
            ],
            infrastructure: vec![InfraRequirement {
                name: "postgres".into(),
                setup_command: "docker-compose up -d db".into(),
                health_check: "pg_isready -h localhost".into(),
                teardown_command: "docker-compose down".into(),
            }],
            teardown_commands: vec!["docker-compose down".into()],
            risks: vec!["Docker must be available".into()],
        }
    }

    #[test]
    fn test_plan_serialization_roundtrip() {
        let plan = sample_plan();
        let json = serde_json::to_string(&plan).unwrap();
        let parsed: TestPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tiers.len(), 2);
        assert_eq!(parsed.tiers[0].name, "unit");
        assert_eq!(parsed.tiers[1].name, "integration");
        assert_eq!(parsed.infrastructure.len(), 1);
    }

    #[test]
    fn test_diagnosis_serialization() {
        let diag = Diagnosis {
            root_cause: "postgres not running".into(),
            failure_kind: FailureKind::InfrastructureUnavailable,
            fixes: vec![PlanFix {
                description: "Start postgres".into(),
                target: "infrastructure".into(),
                action: FixAction::SetupCommand {
                    command: "docker run -d -p 5432:5432 postgres".into(),
                },
            }],
            salvageable: true,
        };
        let json = serde_json::to_string(&diag).unwrap();
        let parsed: Diagnosis = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.failure_kind, FailureKind::InfrastructureUnavailable);
        assert!(parsed.salvageable);
    }

    #[test]
    fn test_apply_fixes_setup_command() {
        let mut plan = sample_plan();
        let fixes = vec![PlanFix {
            description: "Install missing dep".into(),
            target: "unit".into(),
            action: FixAction::SetupCommand {
                command: "pip install numpy".into(),
            },
        }];
        plan = apply_fixes(plan, &fixes);
        assert!(plan.tiers[0]
            .setup_commands
            .contains(&"pip install numpy".to_string()));
    }

    #[test]
    fn test_apply_fixes_replace_test() {
        let mut plan = sample_plan();
        let fixes = vec![PlanFix {
            description: "Fix import".into(),
            target: "unit".into(),
            action: FixAction::ReplaceTest {
                file_path: "tests/unit/test_parse.py".into(),
                new_code: "from mylib import parse\ndef test_parse(): assert parse('x')".into(),
            },
        }];
        plan = apply_fixes(plan, &fixes);
        assert!(plan.tiers[0].tests[0]
            .code
            .contains("from mylib import parse"));
    }

    #[test]
    fn test_apply_fixes_remove_test() {
        let mut plan = sample_plan();
        assert_eq!(plan.tiers[0].tests.len(), 1);
        let fixes = vec![PlanFix {
            description: "Remove broken test".into(),
            target: "unit".into(),
            action: FixAction::RemoveTest {
                file_path: "tests/unit/test_parse.py".into(),
                reason: "Cannot run without GPU".into(),
            },
        }];
        plan = apply_fixes(plan, &fixes);
        assert_eq!(plan.tiers[0].tests.len(), 0);
    }

    #[test]
    fn test_execution_needs_rework_infra_failed() {
        let exec = TestExecution {
            plan: sample_plan(),
            tiers: vec![],
            infra_output: "connection refused".into(),
            infra_ok: false,
            total_passed: 0,
            total_failed: 0,
            rework_iterations: 0,
        };
        assert!(execution_needs_rework(&exec));
    }

    #[test]
    fn test_execution_needs_rework_setup_failed() {
        let exec = TestExecution {
            plan: sample_plan(),
            tiers: vec![TierExecution {
                tier_name: "unit".into(),
                setup_output: "pip not found".into(),
                setup_ok: false,
                test_output: String::new(),
                exit_code: 1,
                passed: 0,
                failed: 0,
            }],
            infra_output: String::new(),
            infra_ok: true,
            total_passed: 0,
            total_failed: 0,
            rework_iterations: 0,
        };
        assert!(execution_needs_rework(&exec));
    }

    #[test]
    fn test_execution_no_rework_legitimate_failure() {
        let exec = TestExecution {
            plan: sample_plan(),
            tiers: vec![TierExecution {
                tier_name: "unit".into(),
                setup_output: String::new(),
                setup_ok: true,
                test_output: "1 passed, 2 failed".into(),
                exit_code: 1,
                passed: 1,
                failed: 2,
            }],
            infra_output: String::new(),
            infra_ok: true,
            total_passed: 1,
            total_failed: 2,
            rework_iterations: 0,
        };
        assert!(!execution_needs_rework(&exec));
    }

    #[test]
    fn test_execution_rework_zero_tests_ran() {
        let exec = TestExecution {
            plan: sample_plan(),
            tiers: vec![TierExecution {
                tier_name: "unit".into(),
                setup_output: String::new(),
                setup_ok: true,
                test_output: "ERROR: no tests collected".into(),
                exit_code: 5,
                passed: 0,
                failed: 0,
            }],
            infra_output: String::new(),
            infra_ok: true,
            total_passed: 0,
            total_failed: 0,
            rework_iterations: 0,
        };
        assert!(execution_needs_rework(&exec));
    }

    #[test]
    fn test_meta_result_with_diagnosis_history() {
        let result = MetaResult {
            plan: sample_plan(),
            execution: None,
            validation: None,
            status: "dry_run".into(),
            diagnosis_history: vec![Diagnosis {
                root_cause: "missing dep".into(),
                failure_kind: FailureKind::MissingDependency,
                fixes: vec![],
                salvageable: true,
            }],
        };
        assert_eq!(result.diagnosis_history.len(), 1);
    }
}
