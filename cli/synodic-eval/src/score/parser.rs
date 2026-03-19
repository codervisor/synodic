use regex::Regex;

use super::{TestFramework, TestResult, TestStatus};

/// Detect whether tests are in Django format or pytest format.
///
/// Django format: "test_name (module.TestClass)"
/// Pytest format: "tests/test_foo.py::TestBar::test_baz"
pub fn detect_test_format(tests: &[String]) -> TestFramework {
    if tests.is_empty() {
        return TestFramework::Pytest;
    }
    let sample = &tests[0];
    if sample.contains("::") {
        return TestFramework::Pytest;
    }
    let django_re = Regex::new(r"^[\w_]+ \([\w.]+\)$").unwrap();
    if django_re.is_match(sample) {
        return TestFramework::Django;
    }
    // Fallback: dotted module path without :: or / suggests Django
    if sample.contains('.') && !sample.contains("::") && !sample.contains('/') {
        return TestFramework::Django;
    }
    TestFramework::Pytest
}

/// Parse a test list file (JSON array or double-JSON-encoded string).
pub fn parse_test_list(content: &str) -> Vec<String> {
    let content = content.trim();
    if content.is_empty() {
        return Vec::new();
    }
    // Try parsing as JSON
    match serde_json::from_str::<serde_json::Value>(content) {
        Ok(serde_json::Value::Array(arr)) => arr
            .into_iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        Ok(serde_json::Value::String(s)) => {
            // Double-JSON-encoded: parse the inner string
            parse_test_list(&s)
        }
        _ => vec![content.to_string()],
    }
}

/// Group Django-format tests by their module for batch execution.
///
/// Input:  ["test_foo (myapp.tests.TestBar)", "test_baz (myapp.tests.TestBar)"]
/// Output: HashMap {"myapp.tests.TestBar" => ["test_foo", "test_baz"]}
pub fn group_django_tests(
    tests: &[String],
) -> (
    std::collections::HashMap<String, Vec<String>>,
    Vec<String>,
) {
    let re = Regex::new(r"^([\w_]+) \(([\w.]+)\)$").unwrap();
    let mut groups: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    let mut unparsed = Vec::new();

    for t in tests {
        if let Some(caps) = re.captures(t) {
            let test_name = caps[1].to_string();
            let module_class = caps[2].to_string();
            groups.entry(module_class).or_default().push(test_name);
        } else {
            unparsed.push(t.clone());
        }
    }
    (groups, unparsed)
}

/// Classify a Django test status word into TestStatus.
pub fn classify_django_status(status_word: &str) -> TestStatus {
    match status_word.to_lowercase().as_str() {
        "ok" => TestStatus::Passed,
        "fail" | "error" => TestStatus::Failed,
        _ => TestStatus::Error,
    }
}

/// Parse Django test runner output lines into test results.
///
/// Matches lines like:
///   test_name (module.TestClass) ... ok
///   test_name (module.TestClass.test_name) ... ok
///   Docstring first line ... ok
pub fn parse_django_output(
    output: &str,
    module_class: &str,
    expected_methods: &std::collections::HashSet<String>,
    unparsed_tests: &std::collections::HashSet<String>,
) -> (Vec<TestResult>, std::collections::HashSet<String>, std::collections::HashSet<String>) {
    let method_re = Regex::new(r"^([\w_]+) \(([\w.]+)\) \.\.\. (\w+)").unwrap();
    let desc_re = Regex::new(r"^(.+?) \.\.\. (\w+)\s*$").unwrap();

    let mut results = Vec::new();
    let mut seen_methods = std::collections::HashSet::new();
    let mut matched_unparsed = std::collections::HashSet::new();

    for line in output.lines() {
        // Try standard format: "test_name (dotted.path) ... status"
        if let Some(caps) = method_re.captures(line) {
            let method_name = &caps[1];
            let status_word = &caps[3];

            if expected_methods.contains(method_name) {
                let orig_id = format!("{} ({})", method_name, module_class);
                if !seen_methods.contains(&orig_id) {
                    seen_methods.insert(orig_id.clone());
                    let status = classify_django_status(status_word);
                    results.push(TestResult {
                        name: orig_id,
                        status,
                        reason: None,
                    });
                }
            }
            continue;
        }

        // Try description/fallback format: "anything ... status"
        if let Some(caps) = desc_re.captures(line) {
            let test_desc = caps[1].trim();
            let status_word = &caps[2];

            for ut in unparsed_tests.iter() {
                if !matched_unparsed.contains(ut)
                    && (ut == test_desc || test_desc.starts_with(ut.as_str()))
                {
                    matched_unparsed.insert(ut.clone());
                    let status = classify_django_status(status_word);
                    results.push(TestResult {
                        name: ut.clone(),
                        status,
                        reason: None,
                    });
                    break;
                }
            }
        }
    }

    (results, seen_methods, matched_unparsed)
}

/// Normalize a pytest node ID for comparison.
///
/// Converts path separators to dots and strips .py from module paths.
pub fn normalize_pytest_id(tid: &str) -> String {
    tid.replace('/', ".").replace(".py::", "::")
}

/// Parse JUnit XML content into test results.
///
/// Returns results only for tests in the expected set.
pub fn parse_junit_xml(
    xml_content: &str,
    expected_tests: &[String],
) -> Option<Vec<TestResult>> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let expected_normalized: std::collections::HashMap<String, String> = expected_tests
        .iter()
        .map(|t| (normalize_pytest_id(t), t.clone()))
        .collect();

    let mut results = Vec::new();
    let mut seen_input: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut reader = Reader::from_str(xml_content);

    let mut in_testcase = false;
    let mut current_input_test: Option<String> = None;
    let mut found_failure = false;
    let mut found_error = false;
    let mut found_skipped = false;
    let mut failure_message = String::new();

    fn find_input_test(
        e: &quick_xml::events::BytesStart<'_>,
        expected_normalized: &std::collections::HashMap<String, String>,
        seen_input: &std::collections::HashSet<String>,
    ) -> (Option<String>, String) {
        let mut classname = String::new();
        let mut name = String::new();
        for attr in e.attributes().flatten() {
            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
            let val = String::from_utf8_lossy(&attr.value).to_string();
            match key.as_str() {
                "classname" => classname = val,
                "name" => name = val,
                _ => {}
            }
        }
        let junit_id = format!("{}::{}", classname, name);
        let junit_normalized = normalize_pytest_id(&junit_id);

        let input_test = expected_normalized
            .get(&junit_normalized)
            .cloned()
            .or_else(|| {
                let suffix = format!("::{}", name);
                expected_normalized
                    .iter()
                    .find(|(norm_id, orig)| {
                        norm_id.ends_with(&suffix) && !seen_input.contains(*orig)
                    })
                    .map(|(_, orig)| orig.clone())
            });
        (input_test, name)
    }

    fn extract_message(e: &quick_xml::events::BytesStart<'_>) -> String {
        for attr in e.attributes().flatten() {
            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
            if key == "message" {
                let val = String::from_utf8_lossy(&attr.value).to_string();
                return val.chars().take(200).collect();
            }
        }
        String::new()
    }

    fn emit_result(
        current_input_test: &Option<String>,
        seen_input: &mut std::collections::HashSet<String>,
        results: &mut Vec<TestResult>,
        found_failure: bool,
        found_error: bool,
        found_skipped: bool,
        failure_message: &str,
    ) {
        if let Some(ref input_test) = current_input_test {
            if !seen_input.contains(input_test) {
                seen_input.insert(input_test.clone());
                let (status, reason) = if found_failure {
                    (TestStatus::Failed, Some(failure_message.to_string()).filter(|s| !s.is_empty()))
                } else if found_error {
                    (TestStatus::Error, Some(failure_message.to_string()).filter(|s| !s.is_empty()))
                } else if found_skipped {
                    (TestStatus::Passed, None)
                } else {
                    (TestStatus::Passed, None)
                };
                results.push(TestResult {
                    name: input_test.clone(),
                    status,
                    reason,
                });
            }
        }
    }

    loop {
        match reader.read_event() {
            Ok(Event::Empty(ref e)) => {
                let local_name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                match local_name.as_str() {
                    "testcase" => {
                        // Self-closing testcase: passed with no children
                        let (input_test, _) = find_input_test(e, &expected_normalized, &seen_input);
                        if let Some(ref it) = input_test {
                            if !seen_input.contains(it) {
                                seen_input.insert(it.clone());
                                results.push(TestResult {
                                    name: it.clone(),
                                    status: TestStatus::Passed,
                                    reason: None,
                                });
                            }
                        }
                    }
                    "failure" if in_testcase => {
                        found_failure = true;
                        failure_message = extract_message(e);
                    }
                    "error" if in_testcase => {
                        found_error = true;
                        failure_message = extract_message(e);
                    }
                    "skipped" if in_testcase => {
                        found_skipped = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Start(ref e)) => {
                let local_name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                match local_name.as_str() {
                    "testcase" => {
                        in_testcase = true;
                        found_failure = false;
                        found_error = false;
                        found_skipped = false;
                        failure_message.clear();
                        let (input_test, _) = find_input_test(e, &expected_normalized, &seen_input);
                        current_input_test = input_test;
                    }
                    "failure" if in_testcase => {
                        found_failure = true;
                        failure_message = extract_message(e);
                    }
                    "error" if in_testcase => {
                        found_error = true;
                        failure_message = extract_message(e);
                    }
                    "skipped" if in_testcase => {
                        found_skipped = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let local_name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if local_name == "testcase" && in_testcase {
                    emit_result(
                        &current_input_test,
                        &mut seen_input,
                        &mut results,
                        found_failure,
                        found_error,
                        found_skipped,
                        &failure_message,
                    );
                    in_testcase = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }

    // Any input tests not seen in JUnit results
    for t in expected_tests {
        if !seen_input.contains(t) {
            results.push(TestResult {
                name: t.clone(),
                status: TestStatus::Error,
                reason: Some("Not found in JUnit XML".into()),
            });
        }
    }

    Some(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_pytest_format() {
        let tests = vec!["tests/test_foo.py::TestBar::test_baz".into()];
        assert_eq!(detect_test_format(&tests), TestFramework::Pytest);
    }

    #[test]
    fn test_detect_django_format() {
        let tests = vec!["test_foo (myapp.tests.TestBar)".into()];
        assert_eq!(detect_test_format(&tests), TestFramework::Django);
    }

    #[test]
    fn test_detect_empty_defaults_pytest() {
        assert_eq!(detect_test_format(&[]), TestFramework::Pytest);
    }

    #[test]
    fn test_parse_test_list_json_array() {
        let content = r#"["test_a", "test_b", "test_c"]"#;
        assert_eq!(parse_test_list(content), vec!["test_a", "test_b", "test_c"]);
    }

    #[test]
    fn test_parse_test_list_double_encoded() {
        let content = r#""[\"test_a\", \"test_b\"]""#;
        assert_eq!(parse_test_list(content), vec!["test_a", "test_b"]);
    }

    #[test]
    fn test_parse_test_list_empty() {
        assert_eq!(parse_test_list(""), Vec::<String>::new());
    }

    #[test]
    fn test_group_django_tests() {
        let tests = vec![
            "test_foo (myapp.tests.TestBar)".into(),
            "test_baz (myapp.tests.TestBar)".into(),
            "test_qux (other.tests.TestQux)".into(),
        ];
        let (groups, unparsed) = group_django_tests(&tests);
        assert!(unparsed.is_empty());
        assert_eq!(groups["myapp.tests.TestBar"].len(), 2);
        assert_eq!(groups["other.tests.TestQux"].len(), 1);
    }

    #[test]
    fn test_group_django_tests_unparsed() {
        let tests = vec![
            "test_foo (myapp.tests.TestBar)".into(),
            "Some description style test".into(),
        ];
        let (groups, unparsed) = group_django_tests(&tests);
        assert_eq!(groups["myapp.tests.TestBar"].len(), 1);
        assert_eq!(unparsed, vec!["Some description style test"]);
    }

    #[test]
    fn test_classify_django_status() {
        assert_eq!(classify_django_status("ok"), TestStatus::Passed);
        assert_eq!(classify_django_status("OK"), TestStatus::Passed);
        assert_eq!(classify_django_status("fail"), TestStatus::Failed);
        assert_eq!(classify_django_status("FAIL"), TestStatus::Failed);
        assert_eq!(classify_django_status("error"), TestStatus::Failed);
        assert_eq!(classify_django_status("unexpected"), TestStatus::Error);
    }

    #[test]
    fn test_parse_django_output_basic() {
        let output = "test_create (myapp.tests.TestModel) ... ok\n\
                       test_delete (myapp.tests.TestModel) ... FAIL\n";
        let mut expected = std::collections::HashSet::new();
        expected.insert("test_create".to_string());
        expected.insert("test_delete".to_string());
        let unparsed = std::collections::HashSet::new();

        let (results, seen, _) =
            parse_django_output(output, "myapp.tests.TestModel", &expected, &unparsed);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].status, TestStatus::Passed);
        assert_eq!(results[1].status, TestStatus::Failed);
        assert_eq!(seen.len(), 2);
    }

    #[test]
    fn test_parse_django_output_description_style() {
        let output = "Check that something works correctly ... ok\n";
        let expected = std::collections::HashSet::new();
        let mut unparsed = std::collections::HashSet::new();
        unparsed.insert("Check that something works correctly".to_string());

        let (results, _, matched) =
            parse_django_output(output, "myapp.tests.TestModel", &expected, &unparsed);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, TestStatus::Passed);
        assert_eq!(matched.len(), 1);
    }

    #[test]
    fn test_normalize_pytest_id() {
        assert_eq!(
            normalize_pytest_id("tests/test_foo.py::TestBar::test_baz"),
            "tests.test_foo::TestBar::test_baz"
        );
    }

    #[test]
    fn test_parse_junit_xml_pass() {
        let xml = r#"<?xml version="1.0" ?>
<testsuite tests="1">
  <testcase classname="tests.test_foo.TestBar" name="test_baz" time="0.1"/>
</testsuite>"#;
        let expected = vec!["tests/test_foo.py::TestBar::test_baz".into()];
        let results = parse_junit_xml(xml, &expected).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, TestStatus::Passed);
    }

    #[test]
    fn test_parse_junit_xml_failure() {
        let xml = r#"<?xml version="1.0" ?>
<testsuite tests="1">
  <testcase classname="tests.test_foo.TestBar" name="test_baz" time="0.1">
    <failure message="AssertionError: expected True"/>
  </testcase>
</testsuite>"#;
        let expected = vec!["tests/test_foo.py::TestBar::test_baz".into()];
        let results = parse_junit_xml(xml, &expected).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, TestStatus::Failed);
    }

    #[test]
    fn test_parse_junit_xml_mixed() {
        let xml = r#"<?xml version="1.0" ?>
<testsuite tests="3">
  <testcase classname="tests.test_a.TestA" name="test_pass" time="0.1"/>
  <testcase classname="tests.test_a.TestA" name="test_fail" time="0.2">
    <failure message="nope"/>
  </testcase>
  <testcase classname="tests.test_a.TestA" name="test_err" time="0.3">
    <error message="boom"/>
  </testcase>
</testsuite>"#;
        let expected = vec![
            "tests/test_a.py::TestA::test_pass".into(),
            "tests/test_a.py::TestA::test_fail".into(),
            "tests/test_a.py::TestA::test_err".into(),
        ];
        let results = parse_junit_xml(xml, &expected).unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].status, TestStatus::Passed);
        assert_eq!(results[1].status, TestStatus::Failed);
        assert_eq!(results[2].status, TestStatus::Error);
    }

    #[test]
    fn test_parse_junit_xml_empty() {
        let xml = r#"<?xml version="1.0" ?><testsuite tests="0"/>"#;
        let expected: Vec<String> = vec![];
        let results = parse_junit_xml(xml, &expected).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_junit_xml_missing_test() {
        let xml = r#"<?xml version="1.0" ?>
<testsuite tests="0">
</testsuite>"#;
        let expected = vec!["tests/test_a.py::TestA::test_missing".into()];
        let results = parse_junit_xml(xml, &expected).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, TestStatus::Error);
        assert!(results[0].reason.as_ref().unwrap().contains("Not found"));
    }

    #[test]
    fn test_parse_test_list_single_string() {
        let content = r#""test_single""#;
        assert_eq!(parse_test_list(content), vec!["test_single"]);
    }

    #[test]
    fn test_parse_django_output_unicode() {
        let output = "test_ünïcödë (myapp.tests.TestModel) ... ok\n";
        let mut expected = std::collections::HashSet::new();
        expected.insert("test_ünïcödë".to_string());
        let unparsed = std::collections::HashSet::new();

        let (results, _, _) =
            parse_django_output(output, "myapp.tests.TestModel", &expected, &unparsed);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, TestStatus::Passed);
    }

    #[test]
    fn test_parse_django_output_empty() {
        let output = "";
        let expected = std::collections::HashSet::new();
        let unparsed = std::collections::HashSet::new();
        let (results, _, _) =
            parse_django_output(output, "myapp.tests.TestModel", &expected, &unparsed);
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_django_output_truncated() {
        // Line without the " ... status" suffix
        let output = "test_foo (myapp.tests.TestModel) ...\n";
        let mut expected = std::collections::HashSet::new();
        expected.insert("test_foo".to_string());
        let unparsed = std::collections::HashSet::new();
        let (results, _, _) =
            parse_django_output(output, "myapp.tests.TestModel", &expected, &unparsed);
        // Truncated output should not match
        assert!(results.is_empty());
    }
}
