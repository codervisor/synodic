use serde::{Deserialize, Serialize};

use crate::events::Event;
use crate::rules::{self, RuleEngine, RuleMatch};

/// Result of an L1 (static/deterministic) evaluation pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1Result {
    pub passed: bool,
    pub matches: Vec<L1Finding>,
    pub events: Vec<Event>,
}

/// A single L1 finding from rule evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L1Finding {
    pub rule_name: String,
    pub matched_text: String,
    pub event_type: String,
    pub severity: String,
}

impl From<&RuleMatch> for L1Finding {
    fn from(m: &RuleMatch) -> Self {
        Self {
            rule_name: m.rule_name.clone(),
            matched_text: m.matched_text.clone(),
            event_type: m.event_type.as_str().to_string(),
            severity: m.severity.as_str().to_string(),
        }
    }
}

/// L1 evaluator — runs static/deterministic rules against content.
///
/// This is the core Layer 1 evaluation that runs at zero AI cost.
/// It wraps the `RuleEngine` with a higher-level API for evaluating
/// diffs, log content, or arbitrary text.
pub struct L1Evaluator {
    engine: RuleEngine,
    source: String,
}

impl L1Evaluator {
    /// Create an evaluator with the default built-in rules.
    pub fn new(source: &str) -> Self {
        Self {
            engine: RuleEngine::new(rules::default_rules()),
            source: source.to_string(),
        }
    }

    /// Create an evaluator with custom rules.
    pub fn with_rules(rules: Vec<rules::Rule>, source: &str) -> Self {
        Self {
            engine: RuleEngine::new(rules),
            source: source.to_string(),
        }
    }

    /// Evaluate content against all L1 rules.
    ///
    /// Returns an `L1Result` with pass/fail status, findings, and generated events.
    pub fn evaluate(&mut self, content: &str) -> L1Result {
        let matches = self.engine.evaluate(content);
        let findings: Vec<L1Finding> = matches.iter().map(L1Finding::from).collect();
        let events = self.engine.matches_to_events(&matches, &self.source);
        let passed = matches.is_empty();

        L1Result {
            passed,
            matches: findings,
            events,
        }
    }

    /// Evaluate a diff (patch) against L1 rules.
    ///
    /// Only evaluates added lines (lines starting with '+') to avoid
    /// flagging content that was removed.
    pub fn evaluate_diff(&mut self, diff: &str) -> L1Result {
        let added_lines: String = diff
            .lines()
            .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
            .map(|line| &line[1..]) // strip the leading '+'
            .collect::<Vec<_>>()
            .join("\n");

        self.evaluate(&added_lines)
    }

    /// Evaluate multiple content chunks, returning a combined result.
    pub fn evaluate_all(&mut self, chunks: &[&str]) -> L1Result {
        let mut all_findings = Vec::new();
        let mut all_events = Vec::new();

        for chunk in chunks {
            let result = self.evaluate(chunk);
            all_findings.extend(result.matches);
            all_events.extend(result.events);
        }

        L1Result {
            passed: all_findings.is_empty(),
            matches: all_findings,
            events: all_events,
        }
    }

    /// Access the underlying rule engine for promotion candidates, etc.
    pub fn engine(&self) -> &RuleEngine {
        &self.engine
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventType;

    #[test]
    fn test_evaluate_clean_content() {
        let mut eval = L1Evaluator::new("test");
        let result = eval.evaluate("normal safe content with no issues");
        assert!(result.passed);
        assert!(result.matches.is_empty());
        assert!(result.events.is_empty());
    }

    #[test]
    fn test_evaluate_secret_detection() {
        let mut eval = L1Evaluator::new("test");
        let result = eval.evaluate("config API_KEY=sk-abc123secret");
        assert!(!result.passed);
        assert!(result
            .matches
            .iter()
            .any(|f| f.rule_name == "secret-in-output"));
        assert!(result
            .events
            .iter()
            .any(|e| e.event_type == EventType::ComplianceViolation));
    }

    #[test]
    fn test_evaluate_diff_only_added() {
        let mut eval = L1Evaluator::new("test");
        let diff = "\
--- a/config.py
+++ b/config.py
@@ -1,3 +1,4 @@
 import os
-old_value = 'safe'
+API_KEY=sk-secret123
+other_line = 'safe'
";
        let result = eval.evaluate_diff(diff);
        assert!(!result.passed);
        assert!(result
            .matches
            .iter()
            .any(|f| f.rule_name == "secret-in-output"));
    }

    #[test]
    fn test_evaluate_diff_removed_lines_ignored() {
        let mut eval = L1Evaluator::new("test");
        let diff = "\
--- a/config.py
+++ b/config.py
@@ -1,2 +1,2 @@
-API_KEY=sk-secret123
+safe_value = 'clean'
";
        let result = eval.evaluate_diff(diff);
        assert!(result.passed);
    }

    #[test]
    fn test_evaluate_all_multiple_chunks() {
        let mut eval = L1Evaluator::new("test");
        let result = eval.evaluate_all(&["normal content", "config API_KEY=sk-abc123", "rm -rf /"]);
        assert!(!result.passed);
        assert!(result.matches.len() >= 2);
    }

    #[test]
    fn test_events_have_correct_source() {
        let mut eval = L1Evaluator::new("my-source");
        let result = eval.evaluate("API_KEY=sk-test");
        assert!(result.events.iter().all(|e| e.source == "my-source"));
    }

    #[test]
    fn test_with_custom_rules() {
        let custom = vec![rules::Rule {
            name: "custom-test".to_string(),
            description: "test rule".to_string(),
            pattern: r"TODO".to_string(),
            event_type: EventType::Misalignment,
            severity: crate::events::Severity::Low,
            enabled: true,
        }];
        let mut eval = L1Evaluator::with_rules(custom, "test");
        let result = eval.evaluate("there is a TODO here");
        assert!(!result.passed);
        assert_eq!(result.matches[0].rule_name, "custom-test");
    }
}
