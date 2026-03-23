use std::path::Path;
use std::process::Command;

use super::{EnvCheck, EnvironmentReport, Severity};

/// Validate environment readiness for a benchmark evaluation.
///
/// Runs a battery of checks appropriate for the benchmark type:
/// - Test runner availability (python, pytest, cargo)
/// - Testbed directory structure
/// - Required files (test lists, patches)
/// - Dependency installation status
pub fn validate(
    benchmark: &str,
    testbed_dir: &Path,
) -> EnvironmentReport {
    let mut checks = Vec::new();

    // Common checks
    checks.push(check_testbed_exists(testbed_dir));
    checks.push(check_testbed_structure(benchmark, testbed_dir));

    // Benchmark-specific checks
    match benchmark {
        "swebench" | "featurebench" => {
            checks.push(check_python_available(testbed_dir));
            checks.push(check_test_lists_exist(benchmark, testbed_dir));
            checks.push(check_repo_dir(testbed_dir));
            checks.push(check_python_imports(testbed_dir));
        }
        "devbench" => {
            checks.push(check_python_available(testbed_dir));
            checks.push(check_repo_dir(testbed_dir));
        }
        "synodic" => {
            checks.push(check_cargo_available());
            checks.push(check_repo_dir(testbed_dir));
            checks.push(check_synodic_meta(testbed_dir));
        }
        _ => {
            checks.push(check_repo_dir(testbed_dir));
        }
    }

    EnvironmentReport::from_checks(checks)
}

fn check_testbed_exists(testbed_dir: &Path) -> EnvCheck {
    let exists = testbed_dir.is_dir();
    EnvCheck {
        name: "testbed_exists".into(),
        passed: exists,
        severity: Severity::Error,
        message: if exists {
            format!("Testbed directory exists: {}", testbed_dir.display())
        } else {
            format!(
                "Testbed directory not found: {}. Run setup first.",
                testbed_dir.display()
            )
        },
    }
}

fn check_testbed_structure(benchmark: &str, testbed_dir: &Path) -> EnvCheck {
    let marker_dir = testbed_dir.join(format!(".{}", benchmark));
    let exists = marker_dir.is_dir();
    EnvCheck {
        name: "testbed_structure".into(),
        passed: exists,
        severity: Severity::Error,
        message: if exists {
            format!("Benchmark marker directory found: .{}", benchmark)
        } else {
            format!(
                "Missing benchmark marker directory .{} — testbed may not be set up for this benchmark",
                benchmark
            )
        },
    }
}

fn check_python_available(testbed_dir: &Path) -> EnvCheck {
    // Prefer venv python, fall back to system
    let venv_python = testbed_dir.join("repo/venv/bin/python");
    if venv_python.exists() {
        return EnvCheck {
            name: "python_available".into(),
            passed: true,
            severity: Severity::Error,
            message: format!("Virtualenv python found: {}", venv_python.display()),
        };
    }

    let result = Command::new("python3").arg("--version").output();
    match result {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            EnvCheck {
                name: "python_available".into(),
                passed: true,
                severity: Severity::Error,
                message: format!("System python3 found: {}", version.trim()),
            }
        }
        _ => EnvCheck {
            name: "python_available".into(),
            passed: false,
            severity: Severity::Error,
            message: "Python 3 not found. Required for Django/Pytest test execution.".into(),
        },
    }
}

fn check_cargo_available() -> EnvCheck {
    let result = Command::new("cargo").arg("--version").output();
    match result {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            EnvCheck {
                name: "cargo_available".into(),
                passed: true,
                severity: Severity::Error,
                message: format!("Cargo found: {}", version.trim()),
            }
        }
        _ => EnvCheck {
            name: "cargo_available".into(),
            passed: false,
            severity: Severity::Error,
            message: "Cargo not found. Required for Synodic dogfood scoring.".into(),
        },
    }
}

fn check_test_lists_exist(benchmark: &str, testbed_dir: &Path) -> EnvCheck {
    let task_dir = testbed_dir.join(format!(".{}", benchmark));
    let f2p = task_dir.join("fail_to_pass.json");
    let p2p = task_dir.join("pass_to_pass.json");

    let f2p_exists = f2p.exists();
    let p2p_exists = p2p.exists();

    if f2p_exists && p2p_exists {
        // Check they're non-empty and valid JSON
        let f2p_valid = std::fs::read_to_string(&f2p)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .is_some();
        let p2p_valid = std::fs::read_to_string(&p2p)
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .is_some();

        if f2p_valid && p2p_valid {
            EnvCheck {
                name: "test_lists".into(),
                passed: true,
                severity: Severity::Error,
                message: "F2P and P2P test lists found and valid".into(),
            }
        } else {
            EnvCheck {
                name: "test_lists".into(),
                passed: false,
                severity: Severity::Error,
                message: "Test list files exist but contain invalid JSON".into(),
            }
        }
    } else {
        let mut missing = Vec::new();
        if !f2p_exists {
            missing.push("fail_to_pass.json");
        }
        if !p2p_exists {
            missing.push("pass_to_pass.json");
        }
        EnvCheck {
            name: "test_lists".into(),
            passed: false,
            severity: Severity::Error,
            message: format!(
                "Missing test list file(s): {}. Cannot score without test definitions.",
                missing.join(", ")
            ),
        }
    }
}

fn check_repo_dir(testbed_dir: &Path) -> EnvCheck {
    let repo_dir = testbed_dir.join("repo");
    let exists = repo_dir.is_dir();

    if exists {
        // Check it's a git repo or at least has source files
        let is_git = repo_dir.join(".git").exists();
        EnvCheck {
            name: "repo_directory".into(),
            passed: true,
            severity: Severity::Error,
            message: if is_git {
                "Repository directory found (git initialized)".into()
            } else {
                "Repository directory found (not a git repo)".into()
            },
        }
    } else {
        EnvCheck {
            name: "repo_directory".into(),
            passed: false,
            severity: Severity::Error,
            message: format!(
                "Repository directory not found at {}/repo",
                testbed_dir.display()
            ),
        }
    }
}

fn check_python_imports(testbed_dir: &Path) -> EnvCheck {
    let repo_dir = testbed_dir.join("repo");
    if !repo_dir.is_dir() {
        return EnvCheck {
            name: "python_imports".into(),
            passed: false,
            severity: Severity::Warning,
            message: "Cannot check imports — repo directory missing".into(),
        };
    }

    // Try importing the project's test dependencies
    let venv_python = testbed_dir.join("repo/venv/bin/python");
    let python = if venv_python.exists() {
        venv_python.to_string_lossy().to_string()
    } else {
        "python3".to_string()
    };

    let result = Command::new(&python)
        .args(["-c", "import pytest; print('pytest', pytest.__version__)"])
        .current_dir(&repo_dir)
        .output();

    match result {
        Ok(output) if output.status.success() => EnvCheck {
            name: "python_imports".into(),
            passed: true,
            severity: Severity::Warning,
            message: format!(
                "Core test dependency available: {}",
                String::from_utf8_lossy(&output.stdout).trim()
            ),
        },
        _ => {
            // Try Django instead
            let django_result = Command::new(&python)
                .args(["-c", "import django; print('django', django.__version__)"])
                .current_dir(&repo_dir)
                .output();

            match django_result {
                Ok(output) if output.status.success() => EnvCheck {
                    name: "python_imports".into(),
                    passed: true,
                    severity: Severity::Warning,
                    message: format!(
                        "Core test dependency available: {}",
                        String::from_utf8_lossy(&output.stdout).trim()
                    ),
                },
                _ => EnvCheck {
                    name: "python_imports".into(),
                    passed: false,
                    severity: Severity::Warning,
                    message: "Neither pytest nor django importable — test execution may fail".into(),
                },
            }
        }
    }
}

fn check_synodic_meta(testbed_dir: &Path) -> EnvCheck {
    let meta_file = testbed_dir.join(".synodic/meta.json");
    if !meta_file.exists() {
        return EnvCheck {
            name: "synodic_meta".into(),
            passed: false,
            severity: Severity::Error,
            message: "Missing .synodic/meta.json — run setup first".into(),
        };
    }

    match std::fs::read_to_string(&meta_file)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    {
        Some(meta) => {
            let has_score_dir = meta.get("score_dir").and_then(|v| v.as_str()).is_some();
            EnvCheck {
                name: "synodic_meta".into(),
                passed: has_score_dir,
                severity: Severity::Error,
                message: if has_score_dir {
                    format!(
                        "Synodic metadata valid (score_dir: {})",
                        meta["score_dir"].as_str().unwrap_or("?")
                    )
                } else {
                    "Synodic metadata missing score_dir field".into()
                },
            }
        }
        None => EnvCheck {
            name: "synodic_meta".into(),
            passed: false,
            severity: Severity::Error,
            message: "Synodic metadata file exists but contains invalid JSON".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_validate_missing_testbed() {
        let report = validate("swebench", Path::new("/tmp/nonexistent-testbed-xyz"));
        assert!(!report.ready);
        assert!(report.blocking_count > 0);
    }

    #[test]
    fn test_validate_empty_testbed() {
        let dir = std::env::temp_dir().join("meta-test-empty-testbed");
        let _ = fs::create_dir_all(&dir);
        let report = validate("swebench", &dir);
        // Testbed exists but structure is wrong
        assert!(!report.ready);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_validate_synodic_no_meta() {
        let dir = std::env::temp_dir().join("meta-test-synodic-no-meta");
        let _ = fs::create_dir_all(dir.join(".synodic"));
        let _ = fs::create_dir_all(dir.join("repo"));
        let report = validate("synodic", &dir);
        // Has structure but no meta.json
        let meta_check = report
            .checks
            .iter()
            .find(|c| c.name == "synodic_meta")
            .unwrap();
        assert!(!meta_check.passed);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_validate_well_formed_synodic() {
        let dir = std::env::temp_dir().join("meta-test-synodic-good");
        let _ = fs::create_dir_all(dir.join(".synodic"));
        let _ = fs::create_dir_all(dir.join("repo/.git"));
        fs::write(
            dir.join(".synodic/meta.json"),
            r#"{"score_dir": "cli"}"#,
        )
        .unwrap();
        let report = validate("synodic", &dir);
        // All checks should pass (cargo is available in this env)
        let blocking: Vec<_> = report
            .checks
            .iter()
            .filter(|c| !c.passed && c.severity == Severity::Error)
            .collect();
        assert!(
            blocking.is_empty(),
            "Unexpected blockers: {:?}",
            blocking
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_validate_swebench_with_test_lists() {
        let dir = std::env::temp_dir().join("meta-test-swebench-lists");
        let task_dir = dir.join(".swebench");
        let _ = fs::create_dir_all(&task_dir);
        let _ = fs::create_dir_all(dir.join("repo"));
        fs::write(task_dir.join("fail_to_pass.json"), r#"["test_foo"]"#).unwrap();
        fs::write(task_dir.join("pass_to_pass.json"), r#"["test_bar"]"#).unwrap();
        let report = validate("swebench", &dir);
        let list_check = report
            .checks
            .iter()
            .find(|c| c.name == "test_lists")
            .unwrap();
        assert!(list_check.passed);
        let _ = fs::remove_dir_all(&dir);
    }
}
