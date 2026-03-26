use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

use super::{MetaConfig, TestExecution, TestPlan, TierExecution};

/// Execute a test plan tier-by-tier with infrastructure lifecycle.
///
/// The execution model mirrors real-world testing:
/// 1. Provision infrastructure (databases, containers, services)
/// 2. Health-check infrastructure
/// 3. Execute tiers in order (smoke → unit → integration → e2e)
/// 4. Earlier tier failure can short-circuit later tiers
/// 5. Always run teardown (even on failure)
///
/// This is the "implement and run" phase. For complex test scenarios,
/// the meta pipeline can delegate to a Synodic pipeline (factory, fractal)
/// instead of calling this directly — but this provides the raw execution
/// capability that any approach needs.
pub fn run_plan(
    config: &MetaConfig,
    plan: &TestPlan,
    run_dir: &Path,
    iteration: u32,
) -> Result<TestExecution> {
    let workdir = &config.workdir;
    let iter_dir = run_dir.join(format!("iteration-{iteration}"));
    fs::create_dir_all(&iter_dir)?;

    // Phase 1: Write all test files across all tiers
    for tier in &plan.tiers {
        for test in &tier.tests {
            write_test_file(workdir, test)?;
        }
    }

    // Phase 2: Provision infrastructure
    let (infra_ok, infra_output) =
        provision_infrastructure(workdir, &plan.infrastructure, &iter_dir)?;

    if !infra_ok {
        // Infrastructure failed — return early so the rework loop can diagnose
        return Ok(TestExecution {
            plan: plan.clone(),
            tiers: vec![],
            infra_output,
            infra_ok: false,
            total_passed: 0,
            total_failed: 0,
            rework_iterations: iteration,
        });
    }

    // Phase 3: Execute tiers in order
    let mut tier_results = Vec::new();
    let mut total_passed = 0usize;
    let mut total_failed = 0usize;
    let mut short_circuit = false;

    for tier in &plan.tiers {
        if short_circuit {
            tier_results.push(TierExecution {
                tier_name: tier.name.clone(),
                setup_output: String::new(),
                setup_ok: false,
                test_output: "Skipped: previous tier failed".into(),
                exit_code: -1,
                passed: 0,
                failed: 0,
            });
            continue;
        }

        let tier_exec = execute_tier(workdir, tier, &iter_dir)?;
        total_passed += tier_exec.passed;
        total_failed += tier_exec.failed;

        let tier_failed = tier_exec.exit_code != 0;
        let should_continue = tier.continue_on_failure || !tier_failed;

        tier_results.push(tier_exec);

        if !should_continue {
            short_circuit = true;
        }
    }

    // Phase 4: Teardown (always, even on failure)
    run_teardown(
        workdir,
        &plan.teardown_commands,
        &plan.infrastructure,
        &iter_dir,
    );

    Ok(TestExecution {
        plan: plan.clone(),
        tiers: tier_results,
        infra_output,
        infra_ok: true,
        total_passed,
        total_failed,
        rework_iterations: iteration,
    })
}

/// Write a test file to disk, creating parent directories as needed.
fn write_test_file(workdir: &Path, test: &super::TestProposal) -> Result<()> {
    let target = workdir.join(&test.file_path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create dir for {}", test.file_path))?;
    }
    fs::write(&target, &test.code)
        .with_context(|| format!("write test file {}", test.file_path))?;
    Ok(())
}

/// Provision infrastructure: run setup commands, then health checks.
fn provision_infrastructure(
    workdir: &Path,
    infra: &[super::InfraRequirement],
    iter_dir: &Path,
) -> Result<(bool, String)> {
    if infra.is_empty() {
        return Ok((true, String::new()));
    }

    let mut output = String::new();

    // Run setup commands
    for req in infra {
        if req.setup_command.is_empty() {
            continue;
        }
        output.push_str(&format!(
            "[infra:{}] setup: {}\n",
            req.name, req.setup_command
        ));
        let result = run_shell_command(workdir, &req.setup_command)?;
        output.push_str(&result.output);

        if !result.success {
            output.push_str(&format!("[infra:{}] setup FAILED\n", req.name));
            let _ = fs::write(iter_dir.join("infra-output.txt"), &output);
            return Ok((false, output));
        }
    }

    // Run health checks (with retries)
    for req in infra {
        if req.health_check.is_empty() {
            continue;
        }
        output.push_str(&format!(
            "[infra:{}] health check: {}\n",
            req.name, req.health_check
        ));

        let mut healthy = false;
        for attempt in 1..=5 {
            let result = run_shell_command(workdir, &req.health_check)?;
            if result.success {
                output.push_str(&format!(
                    "[infra:{}] healthy (attempt {})\n",
                    req.name, attempt
                ));
                healthy = true;
                break;
            }
            if attempt < 5 {
                output.push_str(&format!("[infra:{}] not ready, waiting 2s...\n", req.name));
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }

        if !healthy {
            output.push_str(&format!(
                "[infra:{}] health check FAILED after 5 attempts\n",
                req.name
            ));
            let _ = fs::write(iter_dir.join("infra-output.txt"), &output);
            return Ok((false, output));
        }
    }

    let _ = fs::write(iter_dir.join("infra-output.txt"), &output);
    Ok((true, output))
}

/// Execute a single test tier.
fn execute_tier(workdir: &Path, tier: &super::TestTier, iter_dir: &Path) -> Result<TierExecution> {
    // Run tier setup
    let mut setup_output = String::new();
    for cmd in &tier.setup_commands {
        let result = run_shell_command(workdir, cmd)?;
        setup_output.push_str(&format!("$ {cmd}\n{}\n", result.output));
        if !result.success {
            let _ = fs::write(
                iter_dir.join(format!("tier-{}-setup.txt", tier.name)),
                &setup_output,
            );
            return Ok(TierExecution {
                tier_name: tier.name.clone(),
                setup_output,
                setup_ok: false,
                test_output: String::new(),
                exit_code: 1,
                passed: 0,
                failed: 0,
            });
        }
    }

    // Run tests
    let mut test_output = String::new();
    let mut last_exit = 0i32;
    for cmd in &tier.run_commands {
        let result = run_shell_command(workdir, cmd)?;
        test_output.push_str(&format!("$ {cmd}\n{}\n", result.output));
        last_exit = result.exit_code;
    }

    let (passed, failed) = parse_test_results(&test_output);

    let _ = fs::write(
        iter_dir.join(format!("tier-{}-output.txt", tier.name)),
        &test_output,
    );

    Ok(TierExecution {
        tier_name: tier.name.clone(),
        setup_output,
        setup_ok: true,
        test_output,
        exit_code: last_exit,
        passed,
        failed,
    })
}

/// Run teardown commands (best-effort, never fails the pipeline).
fn run_teardown(
    workdir: &Path,
    teardown_commands: &[String],
    infra: &[super::InfraRequirement],
    iter_dir: &Path,
) {
    let mut output = String::new();

    for cmd in teardown_commands {
        output.push_str(&format!("$ {cmd}\n"));
        if let Ok(result) = run_shell_command(workdir, cmd) {
            output.push_str(&result.output);
        }
    }

    for req in infra {
        if !req.teardown_command.is_empty() {
            output.push_str(&format!(
                "[infra:{}] teardown: {}\n",
                req.name, req.teardown_command
            ));
            if let Ok(result) = run_shell_command(workdir, &req.teardown_command) {
                output.push_str(&result.output);
            }
        }
    }

    let _ = fs::write(iter_dir.join("teardown-output.txt"), &output);
}

// ── Shell execution ─────────────────────────────────────────────────

struct ShellOutput {
    output: String,
    success: bool,
    exit_code: i32,
}

fn run_shell_command(workdir: &Path, cmd: &str) -> Result<ShellOutput> {
    let output = Command::new("sh")
        .args(["-c", cmd])
        .current_dir(workdir)
        .output()
        .with_context(|| format!("failed to run: {cmd}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    Ok(ShellOutput {
        output: combined,
        success: output.status.success(),
        exit_code: output.status.code().unwrap_or(1),
    })
}

/// Parse test pass/fail counts from common framework output formats.
fn parse_test_results(output: &str) -> (usize, usize) {
    let mut passed = 0usize;
    let mut failed = 0usize;

    for line in output.lines() {
        if let Some(n) = extract_number_before(line, " passed") {
            passed += n;
        }
        if let Some(n) = extract_number_before(line, " failed") {
            failed += n;
        }
    }

    (passed, failed)
}

fn extract_number_before(line: &str, keyword: &str) -> Option<usize> {
    let idx = line.find(keyword)?;
    let before = line[..idx].trim_end();
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
        let output = "========================= 5 passed in 0.12s ==========================";
        let (passed, failed) = parse_test_results(output);
        assert_eq!(passed, 5);
        assert_eq!(failed, 0);
    }

    #[test]
    fn test_parse_mixed_output() {
        let output =
            "========================= 3 passed, 2 failed in 0.15s ==========================";
        let (passed, failed) = parse_test_results(output);
        assert_eq!(passed, 3);
        assert_eq!(failed, 2);
    }

    #[test]
    fn test_parse_cargo_output() {
        let output = "test result: ok. 31 passed; 0 failed; 0 ignored;";
        let (passed, failed) = parse_test_results(output);
        assert_eq!(passed, 31);
        assert_eq!(failed, 0);
    }

    #[test]
    fn test_parse_empty() {
        let (passed, failed) = parse_test_results("");
        assert_eq!(passed, 0);
        assert_eq!(failed, 0);
    }

    #[test]
    fn test_extract_number_before() {
        assert_eq!(extract_number_before("5 passed", " passed"), Some(5));
        assert_eq!(
            extract_number_before("31 passed; 0 failed", " passed"),
            Some(31)
        );
        assert_eq!(extract_number_before("no match", " passed"), None);
    }

    #[test]
    fn test_write_test_file() {
        let dir = std::env::temp_dir().join("meta-test-write-v2");
        let _ = fs::create_dir_all(&dir);

        let proposal = super::super::TestProposal {
            description: "Example".into(),
            file_path: "tests/nested/test_example.py".into(),
            code: "def test_one(): assert True".into(),
            kind: "unit".into(),
            pass_criteria: "Passes".into(),
        };

        write_test_file(&dir, &proposal).unwrap();
        let written = fs::read_to_string(dir.join("tests/nested/test_example.py")).unwrap();
        assert_eq!(written, "def test_one(): assert True");

        let _ = fs::remove_dir_all(&dir);
    }
}
