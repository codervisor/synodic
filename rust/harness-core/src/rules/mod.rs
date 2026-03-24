use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::events::{Event, EventType, Severity};

/// A detection rule that matches patterns in agent session data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    pub description: String,
    /// Regex pattern to match against event content
    pub pattern: String,
    pub event_type: EventType,
    pub severity: Severity,
    pub enabled: bool,
}

/// Result of evaluating a rule against some content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleMatch {
    pub rule_name: String,
    pub matched_text: String,
    pub event_type: EventType,
    pub severity: Severity,
}

/// Tracks pattern frequency for rule crystallization.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PatternTracker {
    /// pattern -> occurrence count
    pub counts: HashMap<String, usize>,
    /// Threshold for promotion to L1 rule
    pub promotion_threshold: usize,
}

/// Engine that evaluates rules against content and tracks patterns.
pub struct RuleEngine {
    rules: Vec<Rule>,
    compiled: Vec<(usize, Regex)>,
    pub tracker: PatternTracker,
}

impl RuleEngine {
    pub fn new(rules: Vec<Rule>) -> Self {
        let compiled: Vec<(usize, Regex)> = rules
            .iter()
            .enumerate()
            .filter(|(_, r)| r.enabled)
            .filter_map(|(i, r)| Regex::new(&r.pattern).ok().map(|re| (i, re)))
            .collect();
        Self {
            rules,
            compiled,
            tracker: PatternTracker {
                counts: HashMap::new(),
                promotion_threshold: 3,
            },
        }
    }

    /// Evaluate all enabled rules against the given content.
    pub fn evaluate(&mut self, content: &str) -> Vec<RuleMatch> {
        let mut matches = Vec::new();
        for (idx, re) in &self.compiled {
            if let Some(m) = re.find(content) {
                let rule = &self.rules[*idx];
                matches.push(RuleMatch {
                    rule_name: rule.name.clone(),
                    matched_text: m.as_str().to_string(),
                    event_type: rule.event_type,
                    severity: rule.severity,
                });
                *self.tracker.counts.entry(rule.name.clone()).or_default() += 1;
            }
        }
        matches
    }

    /// Convert rule matches into events.
    pub fn matches_to_events(&self, matches: &[RuleMatch], source: &str) -> Vec<Event> {
        matches
            .iter()
            .map(|m| {
                Event::new(
                    m.event_type,
                    format!("[{}] {}", m.rule_name, m.matched_text),
                    m.severity,
                    source.to_string(),
                    serde_json::json!({
                        "rule": m.rule_name,
                        "matched": m.matched_text,
                    }),
                )
            })
            .collect()
    }

    /// Return rules that have met the promotion threshold.
    pub fn promotion_candidates(&self) -> Vec<&Rule> {
        self.rules
            .iter()
            .filter(|r| {
                self.tracker
                    .counts
                    .get(&r.name)
                    .copied()
                    .unwrap_or(0)
                    >= self.tracker.promotion_threshold
            })
            .collect()
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }
}

/// Built-in rules for common governance violations.
pub fn default_rules() -> Vec<Rule> {
    vec![
        Rule {
            name: "secret-in-output".to_string(),
            description: "Detects potential secrets or API keys in output".to_string(),
            pattern: r"(?i)(api[_-]?key|secret|password|token)\s*[=:]\s*\S+".to_string(),
            event_type: EventType::ComplianceViolation,
            severity: Severity::Critical,
            enabled: true,
        },
        Rule {
            name: "rm-rf-dangerous".to_string(),
            description: "Detects dangerous rm -rf commands".to_string(),
            pattern: r"rm\s+-rf\s+(/|~|\$HOME)".to_string(),
            event_type: EventType::ComplianceViolation,
            severity: Severity::Critical,
            enabled: true,
        },
        Rule {
            name: "force-push".to_string(),
            description: "Detects git force push to main/master".to_string(),
            pattern: r"git\s+push\s+.*--force.*\s+(main|master)".to_string(),
            event_type: EventType::ComplianceViolation,
            severity: Severity::High,
            enabled: true,
        },
        Rule {
            name: "nonexistent-file-ref".to_string(),
            description: "Detects references to common nonexistent patterns".to_string(),
            pattern: r"No such file or directory|ENOENT|FileNotFoundError".to_string(),
            event_type: EventType::Hallucination,
            severity: Severity::Medium,
            enabled: true,
        },
        Rule {
            name: "tool-error".to_string(),
            description: "Detects tool execution errors".to_string(),
            pattern: r"(?i)(error|failed|exception):\s+.+".to_string(),
            event_type: EventType::ToolCallError,
            severity: Severity::Medium,
            enabled: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_secret_detection() {
        let mut engine = RuleEngine::new(default_rules());
        let matches = engine.evaluate("config API_KEY=sk-abc123def");
        assert!(!matches.is_empty());
        assert_eq!(matches[0].rule_name, "secret-in-output");
        assert_eq!(matches[0].event_type, EventType::ComplianceViolation);
    }

    #[test]
    fn test_evaluate_no_match() {
        let mut engine = RuleEngine::new(default_rules());
        let matches = engine.evaluate("normal log output with no issues");
        assert!(matches.is_empty());
    }

    #[test]
    fn test_evaluate_rm_rf() {
        let mut engine = RuleEngine::new(default_rules());
        let matches = engine.evaluate("running: rm -rf /");
        assert!(!matches.is_empty());
        assert_eq!(matches[0].rule_name, "rm-rf-dangerous");
    }

    #[test]
    fn test_evaluate_force_push() {
        let mut engine = RuleEngine::new(default_rules());
        let matches = engine.evaluate("git push --force origin main");
        assert!(!matches.is_empty());
        assert_eq!(matches[0].rule_name, "force-push");
    }

    #[test]
    fn test_matches_to_events() {
        let mut engine = RuleEngine::new(default_rules());
        let matches = engine.evaluate("API_KEY=secret123");
        let events = engine.matches_to_events(&matches, "claude");
        assert_eq!(events.len(), matches.len());
        assert_eq!(events[0].source, "claude");
        assert_eq!(events[0].event_type, EventType::ComplianceViolation);
    }

    #[test]
    fn test_promotion_candidates() {
        let mut engine = RuleEngine::new(default_rules());
        // Trigger secret rule 3 times
        for _ in 0..3 {
            engine.evaluate("API_KEY=secret123");
        }
        let candidates = engine.promotion_candidates();
        assert!(candidates.iter().any(|r| r.name == "secret-in-output"));
    }

    #[test]
    fn test_disabled_rule_skipped() {
        let mut rules = default_rules();
        for r in &mut rules {
            r.enabled = false;
        }
        let mut engine = RuleEngine::new(rules);
        let matches = engine.evaluate("API_KEY=secret123");
        assert!(matches.is_empty());
    }

    #[test]
    fn test_file_not_found_detection() {
        let mut engine = RuleEngine::new(default_rules());
        let matches = engine.evaluate("Error: No such file or directory: /tmp/missing.txt");
        assert!(matches.iter().any(|m| m.rule_name == "nonexistent-file-ref"));
    }
}
