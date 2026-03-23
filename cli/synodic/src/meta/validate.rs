use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use super::{MetaConfig, TestExecution, ValidationReport};

/// AI validator: reviews test results for reliability.
///
/// After tests have been executed, we ask the AI to assess whether the
/// results actually mean what they appear to mean. This catches:
///
/// - **False positives**: Tests that pass but don't actually validate
///   the intended behavior (vacuous assertions, wrong code path, etc.)
/// - **False negatives**: Tests that fail due to environment issues,
///   missing fixtures, or flaky conditions rather than real bugs
/// - **Misleading results**: Tests that technically pass/fail but whose
///   result doesn't inform us about the feature under test
///
/// The AI sees the test plan, the test code, AND the execution output,
/// and reasons about whether we can trust the results.
pub fn assess(
    config: &MetaConfig,
    execution: &TestExecution,
    run_dir: &Path,
) -> Result<ValidationReport> {
    let prompt = build_validation_prompt(execution);

    // Save prompt
    let prompt_path = run_dir.join("meta-validate-prompt.md");
    fs::write(&prompt_path, &prompt)?;

    // Invoke the AI
    let response = invoke_agent(&config.agent_cmd, &prompt, run_dir, "meta-validate")?;

    // Parse the response
    parse_validation_report(&response)
}

fn build_validation_prompt(execution: &TestExecution) -> String {
    let mut prompt = String::new();

    prompt.push_str(
        "You are a test reliability analyst. You have been given test results and your job\n\
         is to assess whether they are trustworthy. Specifically:\n\n\
         1. Do passing tests actually validate the intended behavior? Or do they pass vacuously?\n\
         2. Do failing tests represent real bugs? Or are they caused by environment/setup issues?\n\
         3. Can we trust the overall result? What is your confidence level?\n\n"
    );

    prompt.push_str("## Testing Strategy\n\n");
    prompt.push_str(&execution.plan.strategy);
    prompt.push_str("\n\n");
    prompt.push_str("Rationale: ");
    prompt.push_str(&execution.plan.rationale);
    prompt.push_str("\n\n");

    prompt.push_str("## Tests Executed\n\n");
    for (i, test) in execution.plan.tests.iter().enumerate() {
        prompt.push_str(&format!(
            "### Test {} — {} [{}]\n\nPass criteria: {}\n\n```\n{}\n```\n\n",
            i + 1,
            test.description,
            test.kind,
            test.pass_criteria,
            test.code
        ));
    }

    prompt.push_str("## Setup Result\n\n");
    prompt.push_str(&format!(
        "Setup {}\n\n",
        if execution.setup_ok { "succeeded" } else { "FAILED" }
    ));
    if !execution.setup_output.is_empty() {
        let capped: String = execution.setup_output.lines().take(100).collect::<Vec<_>>().join("\n");
        prompt.push_str(&format!("```\n{capped}\n```\n\n"));
    }

    prompt.push_str("## Test Output\n\n");
    prompt.push_str(&format!(
        "Exit code: {} | Passed: {} | Failed: {}\n\n",
        execution.exit_code, execution.passed, execution.failed
    ));
    // Cap test output to keep prompt manageable
    let capped_output: String = execution.test_output.lines().take(500).collect::<Vec<_>>().join("\n");
    prompt.push_str(&format!("```\n{capped_output}\n```\n\n"));

    prompt.push_str(
        "## Output Format\n\n\
         Respond with a JSON object (and ONLY the JSON, no markdown fences) with this structure:\n\n\
         {\n\
         \x20 \"confidence\": 0.85,\n\
         \x20 \"summary\": \"Brief overall assessment of result reliability\",\n\
         \x20 \"assessments\": [\n\
         \x20   {\n\
         \x20     \"test_description\": \"Which test this refers to\",\n\
         \x20     \"reliable\": true,\n\
         \x20     \"concern\": null,\n\
         \x20     \"reasoning\": \"Why you believe this result is or isn't trustworthy\"\n\
         \x20   }\n\
         \x20 ],\n\
         \x20 \"next_actions\": [\"Recommended follow-up steps if any\"]\n\
         }\n\n\
         For 'concern', use one of: \"false_positive\", \"false_negative\", \"flaky\", \n\
         \"environment\", \"vacuous\", or null if the result is reliable.\n\n\
         confidence should be 0.0 (no trust) to 1.0 (fully trustworthy).\n\
         Be rigorous. A test that passes with 'assert True' is a false positive.\n\
         A test that fails with ImportError is a false negative (environment issue).\n"
    );

    prompt
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
    let combined = format!("{stdout}\n{stderr}");

    fs::write(&output_path, &combined)?;

    if !output.status.success() {
        bail!(
            "AI validation agent failed (exit {}). Output saved to {}",
            output.status.code().unwrap_or(-1),
            output_path.display()
        );
    }

    Ok(stdout)
}

fn parse_validation_report(response: &str) -> Result<ValidationReport> {
    // Try direct parse
    if let Ok(report) = serde_json::from_str::<ValidationReport>(response.trim()) {
        return Ok(report);
    }

    // Try extracting JSON
    let json_str = super::consult::extract_json_block(response)
        .context("Could not find JSON in AI validation response")?;

    serde_json::from_str::<ValidationReport>(&json_str)
        .context("AI validation response contained JSON but didn't match expected schema")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::{TestExecution, TestPlan, TestProposal};

    fn sample_execution() -> TestExecution {
        TestExecution {
            plan: TestPlan {
                strategy: "Unit testing".into(),
                frameworks: vec!["pytest".into()],
                rationale: "Python project".into(),
                setup_commands: vec![],
                run_commands: vec!["pytest".into()],
                tests: vec![TestProposal {
                    description: "Test parse function".into(),
                    file_path: "test_parse.py".into(),
                    code: "def test_parse(): assert parse('x') == 'x'".into(),
                    kind: "unit".into(),
                    pass_criteria: "Correct parsing".into(),
                }],
            },
            setup_output: String::new(),
            setup_ok: true,
            test_output: "1 passed in 0.01s".into(),
            exit_code: 0,
            passed: 1,
            failed: 0,
        }
    }

    #[test]
    fn test_build_validation_prompt_contains_key_sections() {
        let execution = sample_execution();
        let prompt = build_validation_prompt(&execution);

        assert!(prompt.contains("test reliability analyst"));
        assert!(prompt.contains("Unit testing"));
        assert!(prompt.contains("Test parse function"));
        assert!(prompt.contains("Exit code: 0"));
        assert!(prompt.contains("confidence"));
    }

    #[test]
    fn test_parse_validation_report_direct() {
        let json = r#"{
            "confidence": 0.9,
            "summary": "Tests appear reliable",
            "assessments": [{
                "test_description": "Test parse",
                "reliable": true,
                "concern": null,
                "reasoning": "Tests actual parsing logic"
            }],
            "next_actions": []
        }"#;
        let report = parse_validation_report(json).unwrap();
        assert_eq!(report.confidence, 0.9);
        assert!(report.assessments[0].reliable);
    }

    #[test]
    fn test_parse_validation_report_with_concerns() {
        let json = r#"{
            "confidence": 0.3,
            "summary": "Results unreliable",
            "assessments": [{
                "test_description": "Test import",
                "reliable": false,
                "concern": "false_negative",
                "reasoning": "Failed with ModuleNotFoundError"
            }],
            "next_actions": ["Install missing dependencies"]
        }"#;
        let report = parse_validation_report(json).unwrap();
        assert_eq!(report.confidence, 0.3);
        assert!(!report.assessments[0].reliable);
        assert_eq!(
            report.assessments[0].concern.as_deref(),
            Some("false_negative")
        );
    }

    #[test]
    fn test_parse_validation_report_wrapped() {
        let response = "Here's my assessment:\n\n```json\n{\"confidence\": 0.8, \"summary\": \"OK\", \"assessments\": [], \"next_actions\": []}\n```";
        let report = parse_validation_report(response).unwrap();
        assert_eq!(report.confidence, 0.8);
    }
}
