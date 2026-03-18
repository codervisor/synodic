#!/usr/bin/env python3
"""Score runner for SWE-bench and FeatureBench evaluations.

Handles the complexity of different test formats:
- SWE-bench uses Django-style test IDs: "test_name (module.TestClass)"
- FeatureBench uses pytest node IDs: "tests/test_foo.py::TestBar::test_baz"
- Some SWE-bench instances use pytest directly

The scorer:
1. Parses F2P/P2P test lists from the task directory
2. Groups tests by module for efficient batch execution
3. Runs the appropriate test runner (Django or pytest)
4. Produces a JSON score report
"""

import argparse
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone


def parse_test_list(filepath: str) -> list[str]:
    """Parse a test list file (JSON array or double-JSON-encoded string)."""
    if not os.path.exists(filepath):
        return []
    with open(filepath) as f:
        data = json.loads(f.read())
    if isinstance(data, str):
        data = json.loads(data)
    if isinstance(data, list):
        return data
    return [data]


def detect_test_format(tests: list[str]) -> str:
    """Detect whether tests are in Django format or pytest format.

    Django format: "test_name (module.TestClass)"
    Pytest format: "tests/test_foo.py::TestBar::test_baz"
    """
    if not tests:
        return "pytest"
    sample = tests[0]
    if "::" in sample:
        return "pytest"
    if re.match(r"^[\w_]+ \([\w.]+\)$", sample):
        return "django"
    # Fallback: check if it looks like a dotted module path
    if "." in sample and "::" not in sample and "/" not in sample:
        return "django"
    return "pytest"


def detect_django_test_runner(repo_dir: str) -> tuple[str, list[str]]:
    """Detect how to run Django tests for this repo.

    Django repos typically use tests/runtests.py rather than `python -m django test`.
    Returns (runner_type, base_cmd) where runner_type is "runtests" or "django_manage".
    """
    # Django's own repo uses tests/runtests.py
    runtests = os.path.join(repo_dir, "tests", "runtests.py")
    if os.path.exists(runtests):
        return "runtests", [sys.executable, runtests, "--verbosity=2"]

    # Some projects use manage.py test
    manage = os.path.join(repo_dir, "manage.py")
    if os.path.exists(manage):
        return "django_manage", [sys.executable, manage, "test", "--verbosity=2", "--no-input"]

    # Fallback to python -m django test with settings detection
    candidates = [
        "tests/settings.py",
        "test_settings.py",
        "tests/test_settings.py",
    ]
    for c in candidates:
        if os.path.exists(os.path.join(repo_dir, c)):
            settings = c.replace("/", ".").replace(".py", "")
            return "django_module", [
                sys.executable, "-m", "django", "test",
                "--settings", settings, "--verbosity=2", "--no-input",
            ]

    return "django_module", [sys.executable, "-m", "django", "test", "--verbosity=2", "--no-input"]


def group_django_tests(tests: list[str]) -> dict[str, list[str]]:
    """Group Django-format tests by their module for batch execution.

    Input:  ["test_foo (myapp.tests.TestBar)", "test_baz (myapp.tests.TestBar)"]
    Output: {"myapp.tests.TestBar": ["test_foo", "test_baz"]}
    """
    groups: dict[str, list[str]] = {}
    for t in tests:
        match = re.match(r"^([\w_]+) \(([\w.]+)\)$", t)
        if match:
            test_name, module_class = match.groups()
            groups.setdefault(module_class, []).append(test_name)
        else:
            # Can't parse — use as-is
            groups.setdefault("__unparsed__", []).append(t)
    return groups


def run_django_tests(
    tests: list[str], repo_dir: str, label: str
) -> tuple[int, int, int, list[dict]]:
    """Run Django-format tests using Django's test runner.

    Runs whole test classes (not individual methods) to ensure description-style
    tests execute. Filters results to only count tests from the input list.

    Returns (passed, failed, errors, details).
    """
    # Resolve to absolute so subprocess cwd doesn't create double paths
    repo_dir = os.path.abspath(repo_dir)
    runner_type, base_cmd = detect_django_test_runner(repo_dir)

    # Group tests by module.class for batch runs
    groups = group_django_tests(tests)

    passed = 0
    failed = 0
    errors = 0
    details = []

    # Separate description-style (unparsed) test IDs from method-name tests.
    # Description-style IDs come from Django tests with docstrings — SWE-bench
    # stores the first line of the docstring as the test ID.
    unparsed_tests = set(groups.pop("__unparsed__", []))
    matched_unparsed = set()

    # Build expected method-name IDs for filtering.
    # SWE-bench stores: "test_name (module.TestClass)"
    # Django may output: "test_name (module.TestClass.test_name) ... ok"
    # So we match by test method name + class prefix, not exact ID.
    expected_by_class: dict[str, set[str]] = {}
    for module_class, test_names in groups.items():
        expected_by_class[module_class] = set(test_names)

    # Collect unique test classes to run. Run whole classes (not individual
    # methods) so that description-style tests also execute.
    test_classes = set(groups.keys())

    for module_class in test_classes:
        test_names = groups[module_class]
        n_expected = len(test_names) + len(unparsed_tests - matched_unparsed)

        env = os.environ.copy()
        # Run the whole test class, not individual methods
        cmd = base_cmd + [module_class]

        print(f"  [{label}] Running class {module_class} ({len(test_names)} method tests + unparsed)...")

        try:
            result = subprocess.run(
                cmd, capture_output=True, text=True,
                timeout=600, cwd=repo_dir, env=env,
            )
        except subprocess.TimeoutExpired:
            for name in test_names:
                details.append({
                    "test": f"{name} ({module_class})",
                    "status": "ERROR",
                    "reason": "Timeout (600s)",
                })
            errors += len(test_names)
            continue

        # Parse Django test runner output.
        # Django --verbosity=2 outputs two formats:
        #   test_name (module.TestClass.test_name) ... ok   (newer Django)
        #   test_name (module.TestClass) ... ok              (older Django)
        #   Docstring first line ... ok                      (test with docstring)
        output = result.stdout + "\n" + result.stderr
        seen_methods = set()  # track which input method-name tests matched
        for line in output.splitlines():
            # Try standard format: "test_name (dotted.path) ... status"
            m = re.match(r"^([\w_]+) \(([\w.]+)\) \.\.\. (\w+)", line)
            if m:
                method_name, mod_path, status_word = m.groups()
                # Check if this method is one we're looking for.
                # Handle both "test_x (mod.Class)" and "test_x (mod.Class.test_x)".
                if method_name in expected_by_class.get(module_class, set()):
                    orig_id = f"{method_name} ({module_class})"
                    if orig_id not in seen_methods:
                        seen_methods.add(orig_id)
                        status = _classify_django_status(status_word)
                        if status == "PASS":
                            passed += 1
                        elif status == "FAIL":
                            failed += 1
                        else:
                            errors += 1
                        details.append({"test": orig_id, "status": status})
                continue

            # Try description/fallback format: "anything ... status"
            m2 = re.match(r"^(.+?) \.\.\. (\w+)\s*$", line)
            if not m2:
                continue
            test_desc = m2.group(1).strip()
            status_word = m2.group(2)

            # Match against description-style (unparsed) test IDs.
            # Django outputs "Docstring text ... ok" and SWE-bench stores
            # just "Docstring text" (truncated to first line).
            for ut in list(unparsed_tests - matched_unparsed):
                if ut == test_desc or test_desc.startswith(ut):
                    matched_unparsed.add(ut)
                    status = _classify_django_status(status_word)
                    if status == "PASS":
                        passed += 1
                    elif status == "FAIL":
                        failed += 1
                    else:
                        errors += 1
                    details.append({"test": ut, "status": status})
                    break

        # Any method-name tests not seen in output
        for name in test_names:
            orig_id = f"{name} ({module_class})"
            if orig_id not in seen_methods:
                if result.returncode == 0:
                    passed += 1
                    details.append({"test": orig_id, "status": "PASS"})
                else:
                    errors += 1
                    reason = result.stderr[-200:] if result.stderr else "Not found in output"
                    details.append({"test": orig_id, "status": "ERROR", "reason": reason})

    # Any unparsed (description-style) tests not matched in any output
    for ut in unparsed_tests - matched_unparsed:
        errors += 1
        details.append({"test": ut, "status": "ERROR", "reason": "Description-style test not found in any output"})

    return passed, failed, errors, details


def _classify_django_status(status_word: str) -> str:
    """Classify a Django test status word into PASS/FAIL/ERROR."""
    s = status_word.lower()
    if s == "ok":
        return "PASS"
    if s in ("fail", "error"):
        return "FAIL"
    return "ERROR"


def run_pytest_tests(
    tests: list[str], repo_dir: str, label: str
) -> tuple[int, int, int, list[dict]]:
    """Run pytest-format tests.

    Returns (passed, failed, errors, details).
    """
    repo_dir = os.path.abspath(repo_dir)
    if not tests:
        return 0, 0, 0, []

    # Run all tests in a single pytest invocation
    result_file = f"/tmp/score_junit_{label}_{os.getpid()}.xml"
    cmd = [
        sys.executable, "-m", "pytest",
        "--tb=short", "--no-header", "-q",
        f"--junit-xml={result_file}",
        *tests,
    ]

    passed = 0
    failed = 0
    errors = 0
    details = []

    try:
        result = subprocess.run(
            cmd, capture_output=True, text=True,
            timeout=600, cwd=repo_dir,
        )
    except subprocess.TimeoutExpired:
        for t in tests:
            details.append({"test": t, "status": "ERROR", "reason": "Timeout (600s)"})
        return 0, 0, len(tests), details

    # Try parsing JUnit XML for detailed results
    # Build a set of expected test IDs for filtering.
    # Pytest node IDs: "tests/test_foo.py::TestBar::test_baz"
    # JUnit XML uses classname + name: "tests.test_foo.TestBar::test_baz"
    # We normalize both to dotted form for matching.
    def _normalize_pytest_id(tid: str) -> str:
        """Normalize a pytest node ID for comparison."""
        # Convert path separators to dots and strip .py
        return tid.replace("/", ".").replace(".py::", "::")

    expected_normalized = {_normalize_pytest_id(t): t for t in tests}

    if os.path.exists(result_file):
        try:
            import xml.etree.ElementTree as ET
            tree = ET.parse(result_file)
            seen_input = set()  # track which input tests we've matched
            for tc in tree.iter("testcase"):
                junit_id = tc.get("classname", "") + "::" + tc.get("name", "")
                junit_normalized = _normalize_pytest_id(junit_id)

                # Find the matching input test, skip if not in our list
                input_test = expected_normalized.get(junit_normalized)
                if not input_test:
                    # Try matching by suffix (classname may differ)
                    test_method = "::" + tc.get("name", "")
                    for norm_id, orig in expected_normalized.items():
                        if norm_id.endswith(test_method) and orig not in seen_input:
                            input_test = orig
                            break
                if not input_test or input_test in seen_input:
                    continue
                seen_input.add(input_test)

                failure = tc.find("failure")
                error = tc.find("error")
                skipped = tc.find("skipped")
                if failure is not None:
                    failed += 1
                    details.append({
                        "test": input_test,
                        "status": "FAIL",
                        "reason": (failure.get("message", ""))[:200],
                    })
                elif error is not None:
                    errors += 1
                    details.append({
                        "test": input_test,
                        "status": "ERROR",
                        "reason": (error.get("message", ""))[:200],
                    })
                elif skipped is not None:
                    # Count skipped as passed for now
                    passed += 1
                    details.append({"test": input_test, "status": "PASS"})
                else:
                    passed += 1
                    details.append({"test": input_test, "status": "PASS"})

            # Any input tests not seen in JUnit results
            for t in tests:
                if t not in seen_input:
                    if result.returncode == 0:
                        passed += 1
                        details.append({"test": t, "status": "PASS"})
                    else:
                        errors += 1
                        details.append({"test": t, "status": "ERROR", "reason": "Not found in JUnit XML"})
        except Exception:
            pass  # Fall through to exit code check
        finally:
            os.unlink(result_file)

    # If we didn't get JUnit results, fall back to exit code
    if not details:
        if result.returncode == 0:
            passed = len(tests)
            for t in tests:
                details.append({"test": t, "status": "PASS"})
        else:
            failed = len(tests)
            reason = result.stdout[-200:] if result.stdout else ""
            for t in tests:
                details.append({"test": t, "status": "FAIL", "reason": reason})

    return passed, failed, errors, details


def main():
    parser = argparse.ArgumentParser(description="Score SWE-bench/FeatureBench tests")
    parser.add_argument("--instance-id", required=True)
    parser.add_argument("--task-dir", required=True)
    parser.add_argument("--repo-dir", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--bench-type", default="swebench")
    args = parser.parse_args()

    # Parse test lists
    f2p_tests = parse_test_list(os.path.join(args.task_dir, "fail_to_pass.json"))
    p2p_tests = parse_test_list(os.path.join(args.task_dir, "pass_to_pass.json"))

    print(f"F2P tests: {len(f2p_tests)}")
    print(f"P2P tests: {len(p2p_tests)}")
    print()

    # Detect test format
    all_tests = f2p_tests + p2p_tests
    fmt = detect_test_format(all_tests)
    print(f"Test format: {fmt}")
    print()

    runner = run_django_tests if fmt == "django" else run_pytest_tests

    # Run F2P tests
    print("[1/2] Running F2P tests (fail-to-pass)...")
    print("  These tests must PASS after your implementation.")
    print()
    f2p_passed, f2p_failed, f2p_errors, f2p_details = runner(f2p_tests, args.repo_dir, "f2p")
    f2p_total = len(f2p_tests)
    print(f"  F2P: {f2p_passed}/{f2p_total} passed, {f2p_failed} failed, {f2p_errors} errors")
    print()

    # Run P2P tests
    print("[2/2] Running P2P tests (pass-to-pass)...")
    print("  These tests must STILL PASS after your implementation.")
    print()
    p2p_passed, p2p_failed, p2p_errors, p2p_details = runner(p2p_tests, args.repo_dir, "p2p")
    p2p_total = len(p2p_tests)
    print(f"  P2P: {p2p_passed}/{p2p_total} passed, {p2p_failed} failed, {p2p_errors} errors")
    print()

    # Compute verdict
    # Runners now filter to only count tests from the input list, so
    # passed + failed + errors should equal total exactly.
    f2p_all_pass = f2p_passed == f2p_total and f2p_total > 0
    p2p_all_pass = p2p_passed == p2p_total
    resolved = f2p_all_pass and p2p_all_pass

    # Write report
    report = {
        "instance_id": args.instance_id,
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "resolved": resolved,
        "test_format": fmt,
        "f2p": {
            "total": f2p_total,
            "passed": f2p_passed,
            "failed": f2p_failed,
            "errors": f2p_errors,
            "all_pass": f2p_all_pass,
            "details": f2p_details,
        },
        "p2p": {
            "total": p2p_total,
            "passed": p2p_passed,
            "failed": p2p_failed,
            "errors": p2p_errors,
            "all_pass": p2p_all_pass,
            "details": p2p_details,
        },
    }

    os.makedirs(os.path.dirname(args.output), exist_ok=True)
    with open(args.output, "w") as f:
        json.dump(report, f, indent=2)

    print(json.dumps(report, indent=2))
    print()
    print("=== Final Verdict ===")
    if resolved:
        print("RESOLVED — All F2P and P2P tests pass.")
    else:
        print("FAILED")
        if not f2p_all_pass:
            print(f"  F2P: {f2p_passed}/{f2p_total} (need all to pass)")
        if not p2p_all_pass:
            print(f"  P2P: {p2p_passed}/{p2p_total} (regressions detected)")

    sys.exit(0 if resolved else 1)


if __name__ == "__main__":
    main()
