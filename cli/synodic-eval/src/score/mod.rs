pub mod parser;
pub mod report;
pub mod runner;
pub mod verdict;

use serde::{Deserialize, Serialize};

/// Individual test outcome — no stringly-typed status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TestStatus {
    Passed,
    Failed,
    Error,
    Skipped,
}

impl TestStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TestStatus::Passed => "PASS",
            TestStatus::Failed => "FAIL",
            TestStatus::Error => "ERROR",
            TestStatus::Skipped => "SKIPPED",
        }
    }
}

/// Single test result from parser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub status: TestStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Aggregate score — the invariant is structural.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreResult {
    pub passed: usize,
    pub failed: usize,
    pub errors: usize,
    pub skipped: usize,
}

#[allow(dead_code)]
impl ScoreResult {
    /// Total is always passed+failed+errors+skipped.
    /// There is no way to have passed > total.
    pub fn total(&self) -> usize {
        self.passed + self.failed + self.errors + self.skipped
    }

    pub fn all_pass(&self) -> bool {
        self.failed == 0 && self.errors == 0 && self.passed > 0
    }

    /// Build from a list of test results.
    pub fn from_results(results: &[TestResult]) -> Self {
        let mut passed = 0;
        let mut failed = 0;
        let mut errors = 0;
        let mut skipped = 0;
        for r in results {
            match r.status {
                TestStatus::Passed => passed += 1,
                TestStatus::Failed => failed += 1,
                TestStatus::Error => errors += 1,
                TestStatus::Skipped => skipped += 1,
            }
        }
        ScoreResult {
            passed,
            failed,
            errors,
            skipped,
        }
    }
}

/// Test group identity — not a string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TestGroup {
    F2P,
    P2P,
}

/// Verdict for a test group (F2P or P2P).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupVerdict {
    pub group: TestGroup,
    pub expected: Vec<String>,
    pub results: Vec<TestResult>,
    pub score: ScoreResult,
}

/// Overall evaluation verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalVerdict {
    pub instance_id: String,
    pub f2p: GroupVerdict,
    pub p2p: GroupVerdict,
    pub resolved: bool,
}

/// Test framework detection.
#[derive(Debug, Clone, PartialEq)]
pub enum TestFramework {
    Django,
    Pytest,
}
