use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use super::{MetaConfig, TestExecution, ValidationReport};

/// AI validator: reviews test results for reliability.
///
/// After tests execute, we ask the AI to assess whether results actually
/// mean what they appear to mean. The validator sees:
/// - The test plan (strategy, rationale, infrastructure)
/// - The test code itself
/// - The raw execution output per tier
/// - How many rework iterations were needed
///
/// This mirrors the INSPECT phase of the factory pipeline — but specialized
/// for test quality. When the adversarial pipeline is used, this validation
/// is the "critic" that the generator-critic loop builds on.
pub fn assess(
    config: &MetaConfig,
    execution: &TestExecution,
    run_dir: &Path,
) -> Result<ValidationReport> {
    let prompt = build_validation_prompt(execution);

    fs::write(run_dir.join("meta-validate-prompt.md"), &prompt)?;

    let response = invoke_agent(&config.agent_cmd, &prompt, run_dir, "meta-validate")?;
    parse_validation_report(&response)
}

fn build_validation_prompt(execution: &TestExecution) -> String {
    let mut p = String::new();

    p.push_str(
        "You are a test reliability analyst. Assess whether these test results are trustworthy.\n\n\
         Consider:\n\
         1. Do passing tests actually validate intended behavior, or pass vacuously?\n\
         2. Do failing tests represent real bugs, or environment/setup issues?\n\
         3. Did the test infrastructure require rework? What does that tell us?\n\
         4. Are there gaps — things that should be tested but aren't?\n\n"
    );

    p.push_str("## Testing Strategy\n\n");
    p.push_str(&format!("Strategy: {}\n", execution.plan.strategy));
    p.push_str(&format!("Rationale: {}\n", execution.plan.rationale));
    p.push_str(&format!("Frameworks: {}\n", execution.plan.frameworks.join(", ")));
    p.push_str(&format!("Rework iterations needed: {}\n\n", execution.rework_iterations));

    if !execution.plan.risks.is_empty() {
        p.push_str("Known risks:\n");
        for risk in &execution.plan.risks {
            p.push_str(&format!("  - {risk}\n"));
        }
        p.push_str("\n");
    }

    // Per-tier details
    for (i, tier) in execution.plan.tiers.iter().enumerate() {
        p.push_str(&format!("## Tier {}: {} [{}]\n\n", i + 1, tier.name, tier.purpose));

        for test in &tier.tests {
            p.push_str(&format!(
                "### {} [{}]\nPass criteria: {}\n```\n{}\n```\n\n",
                test.description, test.kind, test.pass_criteria, test.code
            ));
        }

        // Tier execution results
        if let Some(tier_exec) = execution.tiers.get(i) {
            let status = if !tier_exec.setup_ok {
                "SETUP FAILED"
            } else if tier_exec.exit_code == 0 {
                "PASSED"
            } else {
                "FAILED"
            };
            p.push_str(&format!(
                "Result: {} (exit {}, {} passed, {} failed)\n",
                status, tier_exec.exit_code, tier_exec.passed, tier_exec.failed
            ));
            if !tier_exec.test_output.is_empty() {
                let capped: String = tier_exec.test_output.lines().take(200).collect::<Vec<_>>().join("\n");
                p.push_str(&format!("```\n{capped}\n```\n\n"));
            }
        }
    }

    p.push_str(
        "## Output Format\n\n\
         Respond with ONLY a JSON object:\n\n\
         {\n\
         \x20 \"confidence\": 0.85,\n\
         \x20 \"summary\": \"Brief overall assessment\",\n\
         \x20 \"assessments\": [\n\
         \x20   {\n\
         \x20     \"test_description\": \"Which test\",\n\
         \x20     \"reliable\": true,\n\
         \x20     \"concern\": null,\n\
         \x20     \"reasoning\": \"Why trustworthy or not\"\n\
         \x20   }\n\
         \x20 ],\n\
         \x20 \"next_actions\": [\"Follow-up steps if any\"]\n\
         }\n\n\
         concern: \"false_positive\"|\"false_negative\"|\"flaky\"|\"environment\"|\"vacuous\"|null\n\
         confidence: 0.0 (no trust) to 1.0 (fully trustworthy)\n"
    );

    p
}

fn invoke_agent(
    agent_cmd: &str,
    prompt: &str,
    run_dir: &Path,
    phase: &str,
) -> Result<String> {
    let output_path = run_dir.join(format!("{phase}-response.txt"));

    let child = Command::new(agent_cmd)
        .args(["--print", "-p", prompt])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to run AI agent: {agent_cmd}"))?;

    let output = child.wait_with_output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    fs::write(&output_path, format!("{stdout}\n{stderr}"))?;

    if !output.status.success() {
        bail!(
            "AI validation agent failed (exit {}). Output: {}",
            output.status.code().unwrap_or(-1),
            output_path.display()
        );
    }

    Ok(stdout)
}

fn parse_validation_report(response: &str) -> Result<ValidationReport> {
    if let Ok(report) = serde_json::from_str::<ValidationReport>(response.trim()) {
        return Ok(report);
    }
    let json_str = super::consult::extract_json_block(response)
        .context("Could not find JSON in AI validation response")?;
    serde_json::from_str::<ValidationReport>(&json_str)
        .context("AI validation JSON didn't match expected schema")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::{TestExecution, TestPlan, TestProposal, TestTier, TierExecution};

    fn sample_execution() -> TestExecution {
        TestExecution {
            plan: TestPlan {
                strategy: "Tiered testing".into(),
                frameworks: vec!["pytest".into()],
                rationale: "Python project".into(),
                tiers: vec![TestTier {
                    name: "unit".into(),
                    purpose: "Fast correctness".into(),
                    setup_commands: vec![],
                    run_commands: vec!["pytest".into()],
                    tests: vec![TestProposal {
                        description: "Test parse".into(),
                        file_path: "test_parse.py".into(),
                        code: "def test_parse(): assert parse('x') == 'x'".into(),
                        kind: "unit".into(),
                        pass_criteria: "Correct parsing".into(),
                    }],
                    continue_on_failure: false,
                }],
                infrastructure: vec![],
                teardown_commands: vec![],
                risks: vec![],
            },
            tiers: vec![TierExecution {
                tier_name: "unit".into(),
                setup_output: String::new(),
                setup_ok: true,
                test_output: "1 passed in 0.01s".into(),
                exit_code: 0,
                passed: 1,
                failed: 0,
            }],
            infra_output: String::new(),
            infra_ok: true,
            total_passed: 1,
            total_failed: 0,
            rework_iterations: 1,
        }
    }

    #[test]
    fn test_validation_prompt_structure() {
        let exec = sample_execution();
        let prompt = build_validation_prompt(&exec);
        assert!(prompt.contains("reliability analyst"));
        assert!(prompt.contains("Tiered testing"));
        assert!(prompt.contains("Test parse"));
        assert!(prompt.contains("PASSED"));
    }

    #[test]
    fn test_parse_report_direct() {
        let json = r#"{"confidence": 0.9, "summary": "Reliable", "assessments": [{"test_description": "Test parse", "reliable": true, "concern": null, "reasoning": "Tests real logic"}], "next_actions": []}"#;
        let report = parse_validation_report(json).unwrap();
        assert_eq!(report.confidence, 0.9);
        assert!(report.assessments[0].reliable);
    }

    #[test]
    fn test_parse_report_with_concerns() {
        let json = r#"{"confidence": 0.3, "summary": "Unreliable", "assessments": [{"test_description": "Test import", "reliable": false, "concern": "false_negative", "reasoning": "ModuleNotFoundError"}], "next_actions": ["Install deps"]}"#;
        let report = parse_validation_report(json).unwrap();
        assert!(!report.assessments[0].reliable);
        assert_eq!(report.assessments[0].concern.as_deref(), Some("false_negative"));
    }

    #[test]
    fn test_parse_report_wrapped() {
        let response = "Assessment:\n```json\n{\"confidence\": 0.8, \"summary\": \"OK\", \"assessments\": [], \"next_actions\": []}\n```";
        let report = parse_validation_report(response).unwrap();
        assert_eq!(report.confidence, 0.8);
    }
}
