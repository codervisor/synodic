use anyhow::{Context, Result};
use std::path::Path;

use crate::events::Event;
use crate::parsers::LogParser;
use crate::rules::{self, RuleEngine};

/// Parser for GitHub Copilot event logs.
///
/// Copilot Chat and Copilot agent write event logs as JSONL in:
/// - VS Code: `~/.vscode/extensions/github.copilot-chat-*/` conversation logs
/// - Copilot agent: `~/.config/github-copilot/events.jsonl`
///
/// Each line is a JSON object. The parser focuses on L2-relevant signals:
/// semantic pattern detection via the rule engine (secrets, dangerous commands,
/// hallucination indicators). L1 concerns (tool errors, command failures,
/// content filter blocks) are handled by git hooks and CI, not Synodic.
pub struct CopilotLogParser {
    engine: std::cell::RefCell<RuleEngine>,
}

impl CopilotLogParser {
    pub fn new() -> Self {
        Self {
            engine: std::cell::RefCell::new(RuleEngine::new(rules::default_rules())),
        }
    }

    /// Parse a single JSONL line and detect L2-relevant events.
    fn parse_line(&self, line: &str) -> Vec<Event> {
        let obj: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };

        // Run rule engine against content fields for L2 pattern detection
        // (secrets, dangerous commands, hallucination indicators)
        let content_str = extract_content(&obj);
        match content_str {
            Some(content) => {
                let mut engine = self.engine.borrow_mut();
                let matches = engine.evaluate(&content);
                engine.matches_to_events(&matches, "copilot")
            }
            None => Vec::new(),
        }
    }
}

impl Default for CopilotLogParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LogParser for CopilotLogParser {
    fn parse(&self, path: &Path) -> Result<Vec<Event>> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading Copilot log: {}", path.display()))?;

        let mut events = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            events.extend(self.parse_line(line));
        }
        Ok(events)
    }

    fn source(&self) -> &str {
        "copilot"
    }
}

/// Extract textual content from a Copilot event for rule evaluation.
fn extract_content(obj: &serde_json::Value) -> Option<String> {
    // Try common content field locations
    let candidates = [
        // Direct content fields
        obj.get("content").and_then(|v| v.as_str()),
        obj.get("message").and_then(|v| v.as_str()),
        obj.get("output").and_then(|v| v.as_str()),
        obj.get("suggestion").and_then(|v| v.as_str()),
        // Nested in properties
        obj.get("properties")
            .and_then(|p| p.get("content"))
            .and_then(|v| v.as_str()),
        obj.get("properties")
            .and_then(|p| p.get("message"))
            .and_then(|v| v.as_str()),
        obj.get("properties")
            .and_then(|p| p.get("output"))
            .and_then(|v| v.as_str()),
    ];

    let mut parts: Vec<&str> = candidates.into_iter().flatten().collect();

    // Also check for array content (multi-turn conversations)
    if let Some(messages) = obj.get("messages").and_then(|v| v.as_array()) {
        for msg in messages {
            if let Some(text) = msg.get("content").and_then(|v| v.as_str()) {
                parts.push(text);
            }
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

/// Find Copilot event log files in common locations.
pub fn find_copilot_logs(project_dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut logs = Vec::new();

    // Check project-local .copilot-events/ directory
    let local_dir = project_dir.join(".copilot-events");
    if local_dir.is_dir() {
        collect_jsonl_files(&local_dir, &mut logs)?;
    }

    // Check ~/.config/github-copilot/ for global event logs
    if let Some(home) = dirs_path() {
        let config_dir = home.join(".config").join("github-copilot");
        if config_dir.is_dir() {
            collect_jsonl_files(&config_dir, &mut logs)?;
        }

        // Check VS Code extension logs
        let vscode_ext = home.join(".vscode").join("extensions");
        if vscode_ext.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&vscode_ext) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("github.copilot-chat-")
                        || name.starts_with("github.copilot-")
                    {
                        let ext_dir = entry.path();
                        // Look for conversation logs
                        let conv_dir = ext_dir.join("conversations");
                        if conv_dir.is_dir() {
                            collect_jsonl_files(&conv_dir, &mut logs)?;
                        }
                        // Look for event logs directly
                        collect_jsonl_files(&ext_dir, &mut logs)?;
                    }
                }
            }
        }
    }

    logs.sort();
    Ok(logs)
}

fn collect_jsonl_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<()> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.ends_with(".jsonl") || name.ends_with(".json") {
                    out.push(path);
                }
            }
        }
    }
    Ok(())
}

fn dirs_path() -> Option<std::path::PathBuf> {
    std::env::var("HOME").ok().map(std::path::PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventType;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp_log(lines: &[&str]) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        for line in lines {
            writeln!(f, "{}", line).unwrap();
        }
        f
    }

    #[test]
    fn test_parse_clean_log() {
        let parser = CopilotLogParser::new();
        let log = write_temp_log(&[
            r#"{"event": "completion", "outcome": "accepted", "suggestion": "console.log('hello')"}"#,
            r#"{"event": "chat_message", "content": "How do I write tests?"}"#,
        ]);
        let events = parser.parse(log.path()).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_parse_secret_in_output() {
        let parser = CopilotLogParser::new();
        let log =
            write_temp_log(&[r#"{"event": "completion", "output": "const API_KEY=sk-secret123"}"#]);
        let events = parser.parse(log.path()).unwrap();
        assert!(events
            .iter()
            .any(|e| e.event_type == EventType::ComplianceViolation));
    }

    #[test]
    fn test_parse_dangerous_command() {
        let parser = CopilotLogParser::new();
        let log =
            write_temp_log(&[r#"{"event": "suggestion", "output": "rm -rf / --no-preserve-root"}"#]);
        let events = parser.parse(log.path()).unwrap();
        assert!(events
            .iter()
            .any(|e| e.event_type == EventType::ComplianceViolation));
    }

    #[test]
    fn test_parse_empty_file() {
        let parser = CopilotLogParser::new();
        let log = write_temp_log(&[]);
        let events = parser.parse(log.path()).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_parse_malformed_json() {
        let parser = CopilotLogParser::new();
        let log = write_temp_log(&["not json", "{bad", ""]);
        let events = parser.parse(log.path()).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_l1_concerns_not_detected() {
        // L1 concerns (tool errors, command failures) should NOT be detected
        // These are handled by git hooks and CI, not Synodic's L2 layer
        let parser = CopilotLogParser::new();
        let log = write_temp_log(&[
            r#"{"event": "tool_error", "tool": "terminal", "message": "Command failed with exit code 1"}"#,
            r#"{"event": "completion", "outcome": "error", "message": "Rate limit exceeded"}"#,
            r#"{"event": "agent_action", "action": "file_edit", "status": "error", "error": "Permission denied"}"#,
        ]);
        let events = parser.parse(log.path()).unwrap();
        // None of these should produce ToolCallError events
        assert!(
            !events.iter().any(|e| e.event_type == EventType::ToolCallError),
            "L1 concerns should not be detected by copilot parser"
        );
    }

    #[test]
    fn test_source_name() {
        let parser = CopilotLogParser::new();
        assert_eq!(parser.source(), "copilot");
    }
}
