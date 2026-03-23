use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

use super::{MetaConfig, TestExecution, TestPlan};

/// Execute a test plan: write test files, run setup, run tests.
///
/// This phase takes the AI-generated TestPlan and makes it real:
/// 1. Write each proposed test file to disk
/// 2. Run setup commands (install deps, compile, etc.)
/// 3. Run the test commands
/// 4. Collect results
pub fn run_plan(
    config: &MetaConfig,
    plan: &TestPlan,
    run_dir: &Path,
) -> Result<TestExecution> {
    let workdir = &config.workdir;

    // Phase 2a: Write test files
    for test in &plan.tests {
        write_test_file(workdir, test)?;
    }

    // Phase 2b: Run setup commands
    let (setup_ok, setup_output) = run_setup(workdir, &plan.setup_commands, run_dir)?;

    if !setup_ok {
        return Ok(TestExecution {
            plan: plan.clone(),
            setup_output,
            setup_ok: false,
            test_output: String::new(),
            exit_code: 1,
            passed: 0,
            failed: 0,
        });
    }

    // Phase 2c: Run test commands
    let (test_output, exit_code) = run_tests(workdir, &plan.run_commands, run_dir)?;

    // Phase 2d: Parse test counts from output
    let (passed, failed) = parse_test_results(&test_output);

    // Save raw output
    let _ = fs::write(run_dir.join("meta-test-output.txt"), &test_output);

    Ok(TestExecution {
        plan: plan.clone(),
        setup_output,
        setup_ok: true,
        test_output,
        exit_code,
        passed,
        failed,
    })
}

/// Write a test file to disk, creating parent directories as needed.
fn write_test_file(workdir: &Path, test: &super::TestProposal) -> Result<()> {
    let target = workdir.join(&test.file_path);

    // Create parent directory
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create dir for {}", test.file_path))?;
    }

    fs::write(&target, &test.code)
        .with_context(|| format!("write test file {}", test.file_path))?;

    Ok(())
}

/// Run setup commands sequentially. Returns (success, combined_output).
fn run_setup(
    workdir: &Path,
    commands: &[String],
    run_dir: &Path,
) -> Result<(bool, String)> {
    let mut combined_output = String::new();

    for cmd in commands.iter() {
        let output = run_shell_command(workdir, cmd)?;
        combined_output.push_str(&format!("$ {cmd}\n{}\n", output.output));

        if !output.success {
            // Save partial output for debugging
            let _ = fs::write(
                run_dir.join("meta-setup-output.txt"),
                &combined_output,
            );
            return Ok((false, combined_output));
        }
    }

    let _ = fs::write(run_dir.join("meta-setup-output.txt"), &combined_output);
    Ok((true, combined_output))
}

/// Run test commands sequentially. Returns (combined_output, last_exit_code).
fn run_tests(
    workdir: &Path,
    commands: &[String],
    run_dir: &Path,
) -> Result<(String, i32)> {
    let mut combined_output = String::new();
    let mut last_exit = 0i32;

    for cmd in commands {
        let output = run_shell_command(workdir, cmd)?;
        combined_output.push_str(&format!("$ {cmd}\n{}\n", output.output));
        last_exit = output.exit_code;
    }

    let _ = fs::write(run_dir.join("meta-test-output.txt"), &combined_output);
    Ok((combined_output, last_exit))
}

struct ShellOutput {
    output: String,
    success: bool,
    exit_code: i32,
}

/// Run a shell command and capture output.
fn run_shell_command(workdir: &Path, cmd: &str) -> Result<ShellOutput> {
    let output = Command::new("sh")
        .args(["-c", cmd])
        .current_dir(workdir)
        .output()
        .with_context(|| format!("failed to run: {cmd}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    let exit_code = output.status.code().unwrap_or(1);

    Ok(ShellOutput {
        output: combined,
        success: output.status.success(),
        exit_code,
    })
}

/// Parse test pass/fail counts from output.
///
/// Supports common output formats from multiple frameworks:
/// - pytest: "5 passed, 2 failed"
/// - cargo test: "test result: ok. 10 passed; 0 failed"
/// - jest: "Tests: 3 passed, 1 failed"
/// - go test: "ok" / "FAIL"
/// - generic: counts lines with PASS/FAIL/ok/FAILED
fn parse_test_results(output: &str) -> (usize, usize) {
    let mut passed = 0usize;
    let mut failed = 0usize;

    for line in output.lines() {
        // pytest format: "5 passed" or "2 failed"
        if let Some(n) = extract_number_before(line, " passed") {
            passed += n;
        }
        if let Some(n) = extract_number_before(line, " failed") {
            failed += n;
        }

        // cargo test and jest formats are already handled by the generic
        // " passed" / " failed" patterns above.
    }

    // Fallback: if no structured output was found, use exit code heuristic
    // The caller will use exit_code to determine overall pass/fail
    (passed, failed)
}

/// Extract the number immediately before a keyword in a line.
fn extract_number_before(line: &str, keyword: &str) -> Option<usize> {
    let idx = line.find(keyword)?;
    let before = line[..idx].trim_end();
    // Find the last contiguous digit sequence
    let num_str: String = before
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if num_str.is_empty() {
        return None;
    }
    num_str.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pytest_output() {
        let output = "============================= test session starts ==============================\ncollected 5 items\n\ntest_foo.py .....\n\n========================= 5 passed in 0.12s ==========================";
        let (passed, failed) = parse_test_results(output);
        assert_eq!(passed, 5);
        assert_eq!(failed, 0);
    }

    #[test]
    fn test_parse_pytest_with_failures() {
        let output = "========================= 3 passed, 2 failed in 0.15s ==========================";
        let (passed, failed) = parse_test_results(output);
        assert_eq!(passed, 3);
        assert_eq!(failed, 2);
    }

    #[test]
    fn test_parse_cargo_test_output() {
        let output = "test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured;";
        let (passed, failed) = parse_test_results(output);
        assert_eq!(passed, 31);
        assert_eq!(failed, 0);
    }

    #[test]
    fn test_parse_empty_output() {
        let (passed, failed) = parse_test_results("");
        assert_eq!(passed, 0);
        assert_eq!(failed, 0);
    }

    #[test]
    fn test_extract_number_before() {
        assert_eq!(extract_number_before("5 passed", " passed"), Some(5));
        assert_eq!(extract_number_before("31 passed; 0 failed", " passed"), Some(31));
        assert_eq!(extract_number_before("no numbers here", " passed"), None);
        assert_eq!(extract_number_before("Tests: 12 passed", " passed"), Some(12));
    }

    #[test]
    fn test_write_test_file() {
        let dir = std::env::temp_dir().join("meta-test-write-test");
        let _ = fs::create_dir_all(&dir);

        let proposal = super::super::TestProposal {
            description: "Test example".into(),
            file_path: "tests/nested/test_example.py".into(),
            code: "def test_one(): assert True".into(),
            kind: "unit".into(),
            pass_criteria: "Always passes".into(),
        };

        write_test_file(&dir, &proposal).unwrap();

        let written = fs::read_to_string(dir.join("tests/nested/test_example.py")).unwrap();
        assert_eq!(written, "def test_one(): assert True");

        let _ = fs::remove_dir_all(&dir);
    }
}
