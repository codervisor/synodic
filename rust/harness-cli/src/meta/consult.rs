use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use super::{Diagnosis, MetaConfig, TestExecution, TestPlan};

/// AI consultant: analyzes the project and produces a testing spec.
///
/// Instead of generating test code directly, the consultant:
/// 1. Analyzes the project and changes
/// 2. Produces a tiered TestPlan (the "what")
/// 3. Recommends which Synodic pipeline to run (factory, fractal, adversarial)
///
/// The actual test implementation is done by the chosen pipeline — leveraging
/// Synodic's existing BUILD → INSPECT, fractal decomposition, or adversarial
/// hardening patterns rather than reimplementing them.
pub fn analyze(config: &MetaConfig, run_dir: &Path) -> Result<TestPlan> {
    let context = gather_project_context(config)?;
    let prompt = build_consult_prompt(config, &context);

    fs::write(run_dir.join("meta-consult-prompt.md"), &prompt)?;

    let response = invoke_agent(&config.agent_cmd, &prompt, run_dir, "meta-consult")?;
    parse_test_plan(&response)
}

/// Re-consult after a failed attempt. The AI gets the failure context
/// and proposes a fundamentally different approach.
pub fn reconsult(
    config: &MetaConfig,
    diagnosis: &Diagnosis,
    failed_exec: &TestExecution,
    run_dir: &Path,
) -> Result<TestPlan> {
    let context = gather_project_context(config)?;
    let prompt = build_reconsult_prompt(&context, diagnosis, failed_exec);

    fs::write(run_dir.join("meta-reconsult-prompt.md"), &prompt)?;

    let response = invoke_agent(&config.agent_cmd, &prompt, run_dir, "meta-reconsult")?;
    parse_test_plan(&response)
}

/// Diagnose why tests failed to run and propose fixes.
pub fn diagnose(
    config: &MetaConfig,
    plan: &TestPlan,
    execution: &TestExecution,
    run_dir: &Path,
    iteration: u32,
) -> Result<Diagnosis> {
    let prompt = build_diagnose_prompt(plan, execution);

    fs::write(
        run_dir.join(format!("meta-diagnose-{iteration}-prompt.md")),
        &prompt,
    )?;

    let response = invoke_agent(
        &config.agent_cmd,
        &prompt,
        run_dir,
        &format!("meta-diagnose-{iteration}"),
    )?;
    parse_diagnosis(&response)
}

// ── Prompt construction ─────────────────────────────────────────────

struct ProjectContext {
    file_listing: String,
    existing_tests: String,
    config_files: String,
    changes: String,
    spec: String,
}

fn gather_project_context(config: &MetaConfig) -> Result<ProjectContext> {
    let workdir = &config.workdir;
    Ok(ProjectContext {
        file_listing: list_project_files(workdir),
        existing_tests: find_existing_tests(workdir),
        config_files: read_config_files(workdir),
        changes: config.diff.clone().unwrap_or_else(|| {
            get_git_diff(workdir).unwrap_or_default()
        }),
        spec: config.spec.clone().unwrap_or_default(),
    })
}

fn build_consult_prompt(_config: &MetaConfig, ctx: &ProjectContext) -> String {
    let mut p = String::new();

    p.push_str(
        "You are a senior testing architect. Analyze this project and propose a testing strategy.\n\n\
         Think about real-world challenges:\n\
         - What infrastructure is needed? (databases, Docker, mock services)\n\
         - What tiers of testing make sense? (smoke → unit → integration → e2e)\n\
         - What could go wrong with the test environment?\n\
         - What would a false positive look like? A false negative?\n\
         - Which Synodic pipeline pattern fits best?\n\
           * factory: straightforward BUILD → INSPECT for clear test requirements\n\
           * fractal: decompose complex multi-component testing into sub-problems\n\
           * adversarial: generator-critic loop for hardening test quality\n\n"
    );

    append_project_context(&mut p, ctx);

    p.push_str(
        "## Output Format\n\n\
         Respond with ONLY a JSON object:\n\n\
         {\n\
         \x20 \"strategy\": \"Overall approach summary\",\n\
         \x20 \"frameworks\": [\"pytest\", \"docker-compose\"],\n\
         \x20 \"rationale\": \"Why this approach fits this project and these changes\",\n\
         \x20 \"recommended_pipeline\": \"factory|fractal|adversarial\",\n\
         \x20 \"tiers\": [\n\
         \x20   {\n\
         \x20     \"name\": \"smoke|unit|integration|e2e|property\",\n\
         \x20     \"purpose\": \"What confidence this tier provides\",\n\
         \x20     \"setup_commands\": [\"commands to prepare this tier\"],\n\
         \x20     \"run_commands\": [\"commands to execute this tier\"],\n\
         \x20     \"tests\": [{\n\
         \x20       \"description\": \"What this test validates\",\n\
         \x20       \"file_path\": \"path/to/test.ext\",\n\
         \x20       \"code\": \"complete, runnable test code\",\n\
         \x20       \"kind\": \"unit|integration|e2e|property|smoke\",\n\
         \x20       \"pass_criteria\": \"What pass/fail means\"\n\
         \x20     }],\n\
         \x20     \"continue_on_failure\": false\n\
         \x20   }\n\
         \x20 ],\n\
         \x20 \"infrastructure\": [{\n\
         \x20   \"name\": \"postgres|redis|docker|mock-server\",\n\
         \x20   \"setup_command\": \"how to provision it\",\n\
         \x20   \"health_check\": \"how to verify it's ready\",\n\
         \x20   \"teardown_command\": \"how to clean up\"\n\
         \x20 }],\n\
         \x20 \"teardown_commands\": [\"cleanup after all testing\"],\n\
         \x20 \"risks\": [\"known risks and mitigations\"]\n\
         }\n\n\
         Guidelines:\n\
         - Every test must be complete and runnable\n\
         - Match existing project conventions\n\
         - Infrastructure requirements must be explicit\n\
         - Risks should be honest — what might not work\n"
    );

    p
}

fn build_reconsult_prompt(
    ctx: &ProjectContext,
    diagnosis: &Diagnosis,
    failed_exec: &TestExecution,
) -> String {
    let mut p = String::new();

    p.push_str(
        "You are a senior testing architect. A previous testing attempt FAILED and the \n\
         approach was deemed unsalvageable. You must propose a fundamentally different strategy.\n\n"
    );

    p.push_str("## What Failed\n\n");
    p.push_str(&format!("Root cause: {}\n", diagnosis.root_cause));
    p.push_str(&format!("Failure kind: {:?}\n\n", diagnosis.failure_kind));

    p.push_str("## Previous Approach (DO NOT REPEAT)\n\n");
    p.push_str(&format!("Strategy: {}\n", failed_exec.plan.strategy));
    p.push_str(&format!("Frameworks: {}\n", failed_exec.plan.frameworks.join(", ")));
    if !failed_exec.infra_output.is_empty() {
        let capped: String = failed_exec.infra_output.lines().take(50).collect::<Vec<_>>().join("\n");
        p.push_str(&format!("\nInfra output:\n```\n{capped}\n```\n\n"));
    }
    for tier in &failed_exec.tiers {
        if !tier.test_output.is_empty() {
            let capped: String = tier.test_output.lines().take(50).collect::<Vec<_>>().join("\n");
            p.push_str(&format!("Tier '{}' output:\n```\n{capped}\n```\n\n", tier.tier_name));
        }
    }

    append_project_context(&mut p, ctx);

    // Same output format as consult
    p.push_str(
        "## Output Format\n\n\
         Same JSON format as before. Propose a DIFFERENT approach that avoids the failure mode above.\n\
         Consider simpler alternatives — maybe skip Docker, use SQLite instead of Postgres, \n\
         mock external services, or test at a different granularity.\n"
    );

    p
}

fn build_diagnose_prompt(plan: &TestPlan, execution: &TestExecution) -> String {
    let mut p = String::new();

    p.push_str(
        "You are a test infrastructure debugger. Tests failed to run properly.\n\
         Analyze the error output and determine the root cause.\n\n"
    );

    p.push_str("## Plan That Was Executed\n\n");
    p.push_str(&format!("Strategy: {}\n", plan.strategy));
    p.push_str(&format!("Frameworks: {}\n\n", plan.frameworks.join(", ")));

    if !execution.infra_ok {
        p.push_str("## Infrastructure Setup FAILED\n\n```\n");
        let capped: String = execution.infra_output.lines().take(100).collect::<Vec<_>>().join("\n");
        p.push_str(&capped);
        p.push_str("\n```\n\n");
    }

    for tier in &execution.tiers {
        if !tier.setup_ok || (tier.passed == 0 && tier.failed == 0 && tier.exit_code != 0) {
            p.push_str(&format!("## Tier '{}' — ", tier.tier_name));
            if !tier.setup_ok {
                p.push_str("SETUP FAILED\n\n");
                let capped: String = tier.setup_output.lines().take(80).collect::<Vec<_>>().join("\n");
                p.push_str(&format!("Setup output:\n```\n{capped}\n```\n\n"));
            } else {
                p.push_str("TESTS DID NOT RUN\n\n");
            }
            if !tier.test_output.is_empty() {
                let capped: String = tier.test_output.lines().take(100).collect::<Vec<_>>().join("\n");
                p.push_str(&format!("Test output:\n```\n{capped}\n```\n\n"));
            }
        }
    }

    p.push_str(
        "## Output Format\n\n\
         Respond with ONLY a JSON object:\n\n\
         {\n\
         \x20 \"root_cause\": \"What specifically went wrong\",\n\
         \x20 \"failure_kind\": \"MissingDependency|InfrastructureUnavailable|VersionMismatch|TestCodeError|EnvironmentConfig|WrongApproach\",\n\
         \x20 \"salvageable\": true,\n\
         \x20 \"fixes\": [{\n\
         \x20   \"description\": \"What to fix\",\n\
         \x20   \"target\": \"tier-name|infrastructure|global\",\n\
         \x20   \"action\": { \"SetupCommand\": { \"command\": \"pip install missing-dep\" } }\n\
         \x20 }]\n\
         }\n\n\
         For action, use one of:\n\
         - {\"SetupCommand\": {\"command\": \"...\"}}\n\
         - {\"ReplaceTest\": {\"file_path\": \"...\", \"new_code\": \"...\"}}\n\
         - {\"AddInfra\": {\"name\": \"...\", \"setup_command\": \"...\", \"health_check\": \"...\", \"teardown_command\": \"...\"}}\n\
         - {\"ReplaceRunCommand\": {\"tier\": \"...\", \"command\": \"...\"}}\n\
         - {\"RemoveTest\": {\"file_path\": \"...\", \"reason\": \"...\"}}\n\n\
         Set salvageable=false if the entire approach is wrong.\n"
    );

    p
}

fn append_project_context(p: &mut String, ctx: &ProjectContext) {
    p.push_str("## Project Structure\n\n```\n");
    p.push_str(&ctx.file_listing);
    p.push_str("\n```\n\n");

    if !ctx.existing_tests.is_empty() {
        p.push_str("## Existing Test Infrastructure\n\n");
        p.push_str(&ctx.existing_tests);
        p.push_str("\n\n");
    }

    if !ctx.config_files.is_empty() {
        p.push_str("## Project Configuration\n\n");
        p.push_str(&ctx.config_files);
        p.push_str("\n\n");
    }

    if !ctx.changes.is_empty() {
        p.push_str("## Changes to Test\n\n```diff\n");
        let capped: String = ctx.changes.lines().take(3000).collect::<Vec<_>>().join("\n");
        p.push_str(&capped);
        p.push_str("\n```\n\n");
    }

    if !ctx.spec.is_empty() {
        p.push_str("## Feature Specification\n\n");
        p.push_str(&ctx.spec);
        p.push_str("\n\n");
    }
}

// ── AI invocation ───────────────────────────────────────────────────

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
            "AI agent failed (exit {}). Output saved to {}",
            output.status.code().unwrap_or(-1),
            output_path.display()
        );
    }

    Ok(stdout)
}

// ── Response parsing ────────────────────────────────────────────────

fn parse_test_plan(response: &str) -> Result<TestPlan> {
    if let Ok(plan) = serde_json::from_str::<TestPlan>(response.trim()) {
        return Ok(plan);
    }
    let json_str = extract_json_block(response)
        .context("Could not find JSON in AI response")?;
    serde_json::from_str::<TestPlan>(&json_str)
        .context("AI response JSON didn't match TestPlan schema")
}

fn parse_diagnosis(response: &str) -> Result<Diagnosis> {
    if let Ok(d) = serde_json::from_str::<Diagnosis>(response.trim()) {
        return Ok(d);
    }
    let json_str = extract_json_block(response)
        .context("Could not find JSON in AI diagnosis response")?;
    serde_json::from_str::<Diagnosis>(&json_str)
        .context("AI diagnosis JSON didn't match Diagnosis schema")
}

/// Extract a JSON block from text that may include markdown fences or prose.
pub fn extract_json_block(text: &str) -> Option<String> {
    // Try ```json ... ``` fence
    if let Some(start) = text.find("```json") {
        let after_fence = &text[start + 7..];
        if let Some(end) = after_fence.find("```") {
            return Some(after_fence[..end].trim().to_string());
        }
    }

    // Try ``` ... ``` fence
    if let Some(start) = text.find("```") {
        let after_fence = &text[start + 3..];
        if let Some(end) = after_fence.find("```") {
            let block = after_fence[..end].trim();
            if block.starts_with('{') {
                return Some(block.to_string());
            }
        }
    }

    // Bare JSON object
    let trimmed = text.trim();
    if let Some(start) = trimmed.find('{') {
        let mut depth = 0;
        let mut in_string = false;
        let mut escape = false;
        for (i, c) in trimmed[start..].char_indices() {
            if escape {
                escape = false;
                continue;
            }
            match c {
                '\\' if in_string => escape = true,
                '"' => in_string = !in_string,
                '{' if !in_string => depth += 1,
                '}' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(trimmed[start..start + i + 1].to_string());
                    }
                }
                _ => {}
            }
        }
    }

    None
}

// ── Project introspection ───────────────────────────────────────────

fn list_project_files(workdir: &Path) -> String {
    let output = Command::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .current_dir(workdir)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let files = String::from_utf8_lossy(&o.stdout);
            files.lines().take(200).collect::<Vec<_>>().join("\n")
        }
        _ => {
            let output = Command::new("find")
                .args([".", "-maxdepth", "2", "-type", "f", "-not", "-path", "./.git/*"])
                .current_dir(workdir)
                .output();
            match output {
                Ok(o) => {
                    let files = String::from_utf8_lossy(&o.stdout);
                    files.lines().take(200).collect::<Vec<_>>().join("\n")
                }
                Err(_) => "(could not list files)".into(),
            }
        }
    }
}

fn find_existing_tests(workdir: &Path) -> String {
    let mut info = String::new();

    let output = Command::new("find")
        .args([
            ".", "-maxdepth", "4", "-type", "f",
            "(", "-name", "test_*", "-o", "-name", "*_test.*", "-o",
            "-name", "*.test.*", "-o", "-name", "*.spec.*", ")",
            "-not", "-path", "./.git/*", "-not", "-path", "*/node_modules/*",
        ])
        .current_dir(workdir)
        .output();

    if let Ok(o) = output {
        let files = String::from_utf8_lossy(&o.stdout);
        let test_files: Vec<&str> = files.lines().take(50).collect();
        if !test_files.is_empty() {
            info.push_str(&format!("Found {} test file(s):\n", test_files.len()));
            for f in &test_files {
                info.push_str(&format!("  {f}\n"));
            }
        }
    }

    let configs = [
        ("pytest.ini", "pytest"), ("pyproject.toml", "Python"),
        ("jest.config.js", "Jest"), ("jest.config.ts", "Jest (TS)"),
        ("vitest.config.ts", "Vitest"), (".mocharc.yml", "Mocha"),
        ("Cargo.toml", "Cargo"), ("go.mod", "Go"),
    ];

    for (file, name) in &configs {
        if workdir.join(file).exists() {
            info.push_str(&format!("Config: {file} ({name})\n"));
        }
    }

    info
}

fn read_config_files(workdir: &Path) -> String {
    let mut info = String::new();
    let configs = [
        "package.json", "Cargo.toml", "pyproject.toml", "setup.py",
        "go.mod", "Gemfile", "pom.xml", "build.gradle",
    ];

    for name in &configs {
        let path = workdir.join(name);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                let capped: String = content.lines().take(100).collect::<Vec<_>>().join("\n");
                info.push_str(&format!("### {name}\n\n```\n{capped}\n```\n\n"));
            }
        }
    }
    info
}

fn get_git_diff(workdir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["diff", "HEAD~1..HEAD"])
        .current_dir(workdir)
        .output()
        .ok()?;

    if output.status.success() {
        let diff = String::from_utf8_lossy(&output.stdout).to_string();
        if !diff.trim().is_empty() {
            return Some(diff);
        }
    }

    let staged = Command::new("git").args(["diff", "--cached"]).current_dir(workdir).output().ok()?;
    let unstaged = Command::new("git").args(["diff"]).current_dir(workdir).output().ok()?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&staged.stdout),
        String::from_utf8_lossy(&unstaged.stdout)
    );
    if combined.trim().is_empty() { None } else { Some(combined) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_from_fenced_block() {
        let text = "Analysis:\n\n```json\n{\"strategy\": \"Unit testing\", \"frameworks\": [\"pytest\"], \"rationale\": \"r\", \"tiers\": [], \"infrastructure\": [], \"teardown_commands\": [], \"risks\": []}\n```\n\nDone.";
        let json = extract_json_block(text).unwrap();
        let plan: TestPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(plan.strategy, "Unit testing");
    }

    #[test]
    fn test_extract_json_bare_object() {
        let text = "Recommendation:\n{\"strategy\": \"Integration\", \"frameworks\": [], \"rationale\": \"r\", \"tiers\": [], \"infrastructure\": [], \"teardown_commands\": [], \"risks\": []}\nDone.";
        let json = extract_json_block(text).unwrap();
        let plan: TestPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(plan.strategy, "Integration");
    }

    #[test]
    fn test_extract_json_no_json() {
        assert!(extract_json_block("no json here").is_none());
    }

    #[test]
    fn test_parse_test_plan_direct() {
        let json = r#"{"strategy": "Smoke", "frameworks": ["jest"], "rationale": "Node", "tiers": [{"name": "smoke", "purpose": "Basic", "setup_commands": [], "run_commands": ["npm test"], "tests": [{"description": "health", "file_path": "t.js", "code": "test('ok', () => {})", "kind": "smoke", "pass_criteria": "runs"}], "continue_on_failure": false}], "infrastructure": [], "teardown_commands": [], "risks": []}"#;
        let plan = parse_test_plan(json).unwrap();
        assert_eq!(plan.tiers.len(), 1);
        assert_eq!(plan.tiers[0].tests[0].kind, "smoke");
    }

    #[test]
    fn test_parse_diagnosis() {
        let json = r#"{"root_cause": "numpy missing", "failure_kind": "MissingDependency", "salvageable": true, "fixes": [{"description": "install numpy", "target": "unit", "action": {"SetupCommand": {"command": "pip install numpy"}}}]}"#;
        let diag = parse_diagnosis(json).unwrap();
        assert_eq!(diag.failure_kind, super::super::FailureKind::MissingDependency);
        assert!(diag.salvageable);
    }

    #[test]
    fn test_extract_json_nested_braces() {
        let text = r#"{"strategy": "t", "frameworks": [], "rationale": "r", "tiers": [{"name": "u", "purpose": "p", "setup_commands": [], "run_commands": [], "tests": [{"description": "d", "file_path": "f", "code": "if (x) { y(); }", "kind": "unit", "pass_criteria": "p"}], "continue_on_failure": false}], "infrastructure": [], "teardown_commands": [], "risks": []}"#;
        let json = extract_json_block(text).unwrap();
        let plan: TestPlan = serde_json::from_str(&json).unwrap();
        assert!(plan.tiers[0].tests[0].code.contains('{'));
    }
}
