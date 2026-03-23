use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

use super::{MetaConfig, TestPlan};

/// AI consultant: analyzes the project and proposes a testing strategy.
///
/// This is the core of the meta-testing approach. Instead of applying static
/// heuristics, we ask an AI agent to reason about:
///
/// 1. The nature of the project (language, framework, architecture)
/// 2. The feature or changes being validated (from diff or spec)
/// 3. Available testing tools and frameworks in the ecosystem
/// 4. What testing methodology is appropriate (unit, integration, property, etc.)
/// 5. The actual test code to implement
///
/// The AI returns a structured TestPlan that Phase 2 can execute.
pub fn analyze(config: &MetaConfig, run_dir: &Path) -> Result<TestPlan> {
    // Gather project context for the AI
    let context = gather_project_context(config)?;

    // Build the consultation prompt
    let prompt = build_consult_prompt(config, &context);

    // Save prompt for auditability
    let prompt_path = run_dir.join("meta-consult-prompt.md");
    fs::write(&prompt_path, &prompt)?;

    // Invoke the AI agent
    let response = invoke_agent(&config.agent_cmd, &prompt, run_dir, "meta-consult")?;

    // Parse the structured response into a TestPlan
    parse_test_plan(&response)
}

/// Gather context about the project that the AI needs to reason about.
fn gather_project_context(config: &MetaConfig) -> Result<ProjectContext> {
    let workdir = &config.workdir;

    // Detect project structure
    let file_listing = list_project_files(workdir);

    // Read existing test infrastructure
    let existing_tests = find_existing_tests(workdir);

    // Read config files that indicate tooling
    let config_files = read_config_files(workdir);

    // Get the diff or spec
    let changes = if let Some(ref diff) = config.diff {
        diff.clone()
    } else {
        // Try to get uncommitted changes
        get_git_diff(workdir).unwrap_or_default()
    };

    let spec = config.spec.clone().unwrap_or_default();

    Ok(ProjectContext {
        file_listing,
        existing_tests,
        config_files,
        changes,
        spec,
    })
}

struct ProjectContext {
    file_listing: String,
    existing_tests: String,
    config_files: String,
    changes: String,
    spec: String,
}

fn build_consult_prompt(_config: &MetaConfig, ctx: &ProjectContext) -> String {
    let mut prompt = String::new();

    prompt.push_str(
        "You are a senior testing consultant. Your job is to analyze a software project,\n\
         understand the changes being made, and propose a comprehensive testing strategy\n\
         with actual test implementations.\n\n\
         You must reason about:\n\
         1. What the project is and what technologies/frameworks it uses\n\
         2. What changes are being made and what they affect\n\
         3. What testing tools and frameworks are appropriate for this project\n\
         4. What kinds of tests will catch real bugs (not produce false positives/negatives)\n\
         5. The actual test code needed\n\n"
    );

    // Project structure
    prompt.push_str("## Project Structure\n\n```\n");
    prompt.push_str(&ctx.file_listing);
    prompt.push_str("\n```\n\n");

    // Existing tests
    if !ctx.existing_tests.is_empty() {
        prompt.push_str("## Existing Test Infrastructure\n\n");
        prompt.push_str(&ctx.existing_tests);
        prompt.push_str("\n\n");
    }

    // Config files (package.json, Cargo.toml, pyproject.toml, etc.)
    if !ctx.config_files.is_empty() {
        prompt.push_str("## Project Configuration\n\n");
        prompt.push_str(&ctx.config_files);
        prompt.push_str("\n\n");
    }

    // Changes under test
    if !ctx.changes.is_empty() {
        prompt.push_str("## Changes to Test\n\n```diff\n");
        // Cap diff at 3000 lines to stay within reasonable context
        let capped: String = ctx.changes.lines().take(3000).collect::<Vec<_>>().join("\n");
        prompt.push_str(&capped);
        prompt.push_str("\n```\n\n");
    }

    // Spec if provided
    if !ctx.spec.is_empty() {
        prompt.push_str("## Feature Specification\n\n");
        prompt.push_str(&ctx.spec);
        prompt.push_str("\n\n");
    }

    // Output format
    prompt.push_str(
        "## Output Format\n\n\
         Respond with a JSON object (and ONLY the JSON, no markdown fences) with this structure:\n\n\
         {\n\
         \x20 \"strategy\": \"Brief description of overall testing approach\",\n\
         \x20 \"frameworks\": [\"framework1\", \"framework2\"],\n\
         \x20 \"rationale\": \"Why this approach is appropriate for this project and these changes\",\n\
         \x20 \"setup_commands\": [\"commands to prepare the test environment\"],\n\
         \x20 \"run_commands\": [\"commands to execute the tests\"],\n\
         \x20 \"tests\": [\n\
         \x20   {\n\
         \x20     \"description\": \"What this test validates\",\n\
         \x20     \"file_path\": \"path/to/test_file.ext\",\n\
         \x20     \"code\": \"full test code for this file\",\n\
         \x20     \"kind\": \"unit|integration|e2e|property|smoke\",\n\
         \x20     \"pass_criteria\": \"What a pass or failure means in context\"\n\
         \x20   }\n\
         \x20 ]\n\
         }\n\n\
         Important guidelines:\n\
         - Match the project's existing test conventions when possible\n\
         - Write tests that validate real behavior, not implementation details\n\
         - For each test, explain what a pass/fail actually tells us about the code\n\
         - Prefer the simplest testing approach that catches real bugs\n\
         - Include setup_commands only for dependencies not already installed\n\
         - Every test in 'code' must be complete and runnable\n"
    );

    prompt
}

/// Invoke the AI agent with a prompt and return its response.
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
            "AI agent failed (exit {}). Output saved to {}",
            output.status.code().unwrap_or(-1),
            output_path.display()
        );
    }

    Ok(stdout)
}

/// Parse the AI's response into a structured TestPlan.
///
/// The AI is instructed to output JSON, but may include markdown fences
/// or preamble text. We extract the JSON portion.
fn parse_test_plan(response: &str) -> Result<TestPlan> {
    // Try direct JSON parse first
    if let Ok(plan) = serde_json::from_str::<TestPlan>(response.trim()) {
        return Ok(plan);
    }

    // Try extracting JSON from markdown code fence
    let json_str = extract_json_block(response)
        .context("Could not find JSON in AI response. The AI may not have followed the output format.")?;

    serde_json::from_str::<TestPlan>(&json_str)
        .context("AI response contained JSON but it didn't match the expected TestPlan schema")
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

    // Try to find a bare JSON object
    let trimmed = text.trim();
    if let Some(start) = trimmed.find('{') {
        // Find the matching closing brace
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

// ── Project introspection helpers ───────────────────────────────────

fn list_project_files(workdir: &Path) -> String {
    // Use git ls-files if available, otherwise fall back to find
    let output = Command::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .current_dir(workdir)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let files = String::from_utf8_lossy(&o.stdout);
            // Cap at 200 lines to keep prompt reasonable
            files.lines().take(200).collect::<Vec<_>>().join("\n")
        }
        _ => {
            // Fallback: list top-level + one level deep
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

    // Look for test directories and files
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

    // Check for test runner configs
    let configs = [
        ("pytest.ini", "pytest"),
        ("pyproject.toml", "Python project"),
        ("jest.config.js", "Jest"),
        ("jest.config.ts", "Jest (TypeScript)"),
        ("vitest.config.ts", "Vitest"),
        (".mocharc.yml", "Mocha"),
        ("Cargo.toml", "Cargo/Rust"),
        ("go.mod", "Go"),
    ];

    for (file, name) in &configs {
        if workdir.join(file).exists() {
            info.push_str(&format!("Test runner config: {file} ({name})\n"));
        }
    }

    info
}

fn read_config_files(workdir: &Path) -> String {
    let mut info = String::new();

    let configs = [
        "package.json",
        "Cargo.toml",
        "pyproject.toml",
        "setup.py",
        "go.mod",
        "Gemfile",
        "pom.xml",
        "build.gradle",
    ];

    for name in &configs {
        let path = workdir.join(name);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                // Cap each file at 100 lines
                let capped: String = content.lines().take(100).collect::<Vec<_>>().join("\n");
                info.push_str(&format!("### {name}\n\n```\n{capped}\n```\n\n"));
            }
        }
    }

    info
}

fn get_git_diff(workdir: &Path) -> Option<String> {
    // Try committed changes first
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

    // Fall back to staged + unstaged
    let staged = Command::new("git")
        .args(["diff", "--cached"])
        .current_dir(workdir)
        .output()
        .ok()?;
    let unstaged = Command::new("git")
        .args(["diff"])
        .current_dir(workdir)
        .output()
        .ok()?;

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&staged.stdout),
        String::from_utf8_lossy(&unstaged.stdout)
    );

    if combined.trim().is_empty() {
        None
    } else {
        Some(combined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_from_fenced_block() {
        let text = r#"Here is my analysis:

```json
{
  "strategy": "Unit testing",
  "frameworks": ["pytest"],
  "rationale": "Because",
  "setup_commands": [],
  "run_commands": ["pytest"],
  "tests": []
}
```

That should work."#;
        let json = extract_json_block(text).unwrap();
        let plan: TestPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(plan.strategy, "Unit testing");
    }

    #[test]
    fn test_extract_json_bare_object() {
        let text = r#"My recommendation:
{
  "strategy": "Integration tests",
  "frameworks": ["cargo"],
  "rationale": "Rust project",
  "setup_commands": [],
  "run_commands": ["cargo test"],
  "tests": []
}
Done."#;
        let json = extract_json_block(text).unwrap();
        let plan: TestPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(plan.strategy, "Integration tests");
    }

    #[test]
    fn test_extract_json_no_json() {
        let text = "This response has no JSON at all.";
        assert!(extract_json_block(text).is_none());
    }

    #[test]
    fn test_parse_test_plan_direct() {
        let json = r#"{
            "strategy": "Smoke tests",
            "frameworks": ["jest"],
            "rationale": "Node.js project",
            "setup_commands": ["npm install"],
            "run_commands": ["npm test"],
            "tests": [{
                "description": "API returns 200",
                "file_path": "tests/api.test.js",
                "code": "test('health', async () => { expect(200).toBe(200); });",
                "kind": "smoke",
                "pass_criteria": "Server responds"
            }]
        }"#;
        let plan = parse_test_plan(json).unwrap();
        assert_eq!(plan.tests.len(), 1);
        assert_eq!(plan.tests[0].kind, "smoke");
    }

    #[test]
    fn test_parse_test_plan_with_wrapper() {
        let response = "Sure! Here's the test plan:\n\n```json\n{\n  \"strategy\": \"Unit\",\n  \"frameworks\": [],\n  \"rationale\": \"r\",\n  \"setup_commands\": [],\n  \"run_commands\": [],\n  \"tests\": []\n}\n```";
        let plan = parse_test_plan(response).unwrap();
        assert_eq!(plan.strategy, "Unit");
    }

    #[test]
    fn test_extract_json_nested_braces() {
        let text = r#"{"strategy": "test", "frameworks": [], "rationale": "r", "setup_commands": [], "run_commands": [], "tests": [{"description": "d", "file_path": "f", "code": "if (x) { y(); }", "kind": "unit", "pass_criteria": "p"}]}"#;
        let json = extract_json_block(text).unwrap();
        let plan: TestPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(plan.tests.len(), 1);
        assert!(plan.tests[0].code.contains('{'));
    }
}
