use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;
use anyhow::{Context, Result};

use super::parser;
use super::{TestResult, TestStatus};

/// Detect how to run Django tests for this repo.
///
/// Returns the base command to execute.
fn detect_django_runner(repo_dir: &Path, python: &str) -> Vec<String> {
    let runtests = repo_dir.join("tests/runtests.py");
    if runtests.exists() {
        return vec![
            python.to_string(),
            runtests.to_string_lossy().to_string(),
            "--verbosity=2".to_string(),
        ];
    }

    let manage = repo_dir.join("manage.py");
    if manage.exists() {
        return vec![
            python.to_string(),
            manage.to_string_lossy().to_string(),
            "test".to_string(),
            "--verbosity=2".to_string(),
            "--no-input".to_string(),
        ];
    }

    // Try detecting settings
    let candidates = [
        "tests/settings.py",
        "test_settings.py",
        "tests/test_settings.py",
    ];
    for c in candidates {
        if repo_dir.join(c).exists() {
            let settings = c.replace('/', ".").replace(".py", "");
            return vec![
                python.to_string(),
                "-m".to_string(),
                "django".to_string(),
                "test".to_string(),
                "--settings".to_string(),
                settings,
                "--verbosity=2".to_string(),
                "--no-input".to_string(),
            ];
        }
    }

    vec![
        python.to_string(),
        "-m".to_string(),
        "django".to_string(),
        "test".to_string(),
        "--verbosity=2".to_string(),
        "--no-input".to_string(),
    ]
}

/// Resolve the python binary: prefer venv python, fallback to system python3.
pub fn resolve_python(testbed_dir: &Path) -> String {
    let venv_python = testbed_dir.join("venv/bin/python");
    if venv_python.exists() {
        venv_python.to_string_lossy().to_string()
    } else {
        "python3".to_string()
    }
}

/// Run Django-format tests using Django's test runner.
///
/// Returns test results filtered to only tests from the input list.
pub fn run_django_tests(
    tests: &[String],
    repo_dir: &Path,
    python: &str,
    label: &str,
) -> Result<Vec<TestResult>> {
    if tests.is_empty() {
        return Ok(Vec::new());
    }

    let repo_dir = repo_dir.canonicalize().context("canonicalize repo dir")?;
    let base_cmd = detect_django_runner(&repo_dir, python);
    let (groups, unparsed) = parser::group_django_tests(tests);

    let mut all_results = Vec::new();
    let unparsed_set: HashSet<String> = unparsed.into_iter().collect();
    let mut matched_unparsed: HashSet<String> = HashSet::new();

    // Build expected method-name IDs for filtering
    let mut expected_by_class: HashMap<String, HashSet<String>> = HashMap::new();
    for (module_class, test_names) in &groups {
        expected_by_class.insert(
            module_class.clone(),
            test_names.iter().cloned().collect(),
        );
    }

    let test_classes: Vec<String> = groups.keys().cloned().collect();

    for module_class in &test_classes {
        let test_names = &groups[module_class];
        let expected_methods = expected_by_class
            .get(module_class)
            .cloned()
            .unwrap_or_default();

        eprintln!(
            "  [{}] Running class {} ({} method tests)...",
            label,
            module_class,
            test_names.len()
        );

        let mut cmd_args = base_cmd.clone();
        cmd_args.push(module_class.clone());

        let output = Command::new(&cmd_args[0])
            .args(&cmd_args[1..])
            .current_dir(&repo_dir)
            .output();

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = format!("{}\n{}", stdout, stderr);

                let (parsed_results, seen_methods, new_matched_unparsed) =
                    parser::parse_django_output(
                        &combined,
                        module_class,
                        &expected_methods,
                        &unparsed_set,
                    );

                all_results.extend(parsed_results);
                matched_unparsed.extend(new_matched_unparsed);

                // Any method-name tests not seen in output
                for name in test_names {
                    let orig_id = format!("{} ({})", name, module_class);
                    if !seen_methods.contains(&orig_id) {
                        if output.status.success() {
                            all_results.push(TestResult {
                                name: orig_id,
                                status: TestStatus::Passed,
                                reason: None,
                            });
                        } else {
                            let reason = if stderr.len() > 200 {
                                stderr[stderr.len() - 200..].to_string()
                            } else if !stderr.is_empty() {
                                stderr.to_string()
                            } else {
                                "Not found in output".into()
                            };
                            all_results.push(TestResult {
                                name: orig_id,
                                status: TestStatus::Error,
                                reason: Some(reason),
                            });
                        }
                    }
                }
            }
            Err(_) => {
                // Timeout or execution failure
                for name in test_names {
                    all_results.push(TestResult {
                        name: format!("{} ({})", name, module_class),
                        status: TestStatus::Error,
                        reason: Some("Execution failed".into()),
                    });
                }
            }
        }
    }

    // Any unparsed (description-style) tests not matched
    for ut in &unparsed_set {
        if !matched_unparsed.contains(ut) {
            all_results.push(TestResult {
                name: ut.clone(),
                status: TestStatus::Error,
                reason: Some("Description-style test not found in any output".into()),
            });
        }
    }

    Ok(all_results)
}

/// Run pytest-format tests.
///
/// Returns test results.
pub fn run_pytest_tests(
    tests: &[String],
    repo_dir: &Path,
    python: &str,
    label: &str,
) -> Result<Vec<TestResult>> {
    if tests.is_empty() {
        return Ok(Vec::new());
    }

    let repo_dir = repo_dir.canonicalize().context("canonicalize repo dir")?;
    let result_file = format!(
        "/tmp/score_junit_{}_{}.xml",
        label,
        std::process::id()
    );

    let mut cmd = Command::new(python);
    cmd.args(["-m", "pytest", "--tb=short", "--no-header", "-q"])
        .arg(format!("--junit-xml={}", result_file))
        .args(tests)
        .current_dir(&repo_dir);

    eprintln!("  [{}] Running {} pytest tests...", label, tests.len());

    let output = cmd.output();

    let mut results = Vec::new();

    match output {
        Ok(output) => {
            // Try parsing JUnit XML for detailed results
            if Path::new(&result_file).exists() {
                if let Ok(xml_content) = std::fs::read_to_string(&result_file) {
                    if let Some(xml_results) =
                        parser::parse_junit_xml(&xml_content, tests)
                    {
                        results = xml_results;
                    }
                }
                let _ = std::fs::remove_file(&result_file);
            }

            // If we didn't get JUnit results, fall back to exit code
            if results.is_empty() {
                if output.status.success() {
                    for t in tests {
                        results.push(TestResult {
                            name: t.clone(),
                            status: TestStatus::Passed,
                            reason: None,
                        });
                    }
                } else {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let reason: String = stdout.chars().rev().take(200).collect::<String>().chars().rev().collect();
                    for t in tests {
                        results.push(TestResult {
                            name: t.clone(),
                            status: TestStatus::Failed,
                            reason: Some(reason.clone()),
                        });
                    }
                }
            }
        }
        Err(_) => {
            // Timeout or execution failure
            for t in tests {
                results.push(TestResult {
                    name: t.clone(),
                    status: TestStatus::Error,
                    reason: Some("Timeout (600s)".into()),
                });
            }
        }
    }

    Ok(results)
}
