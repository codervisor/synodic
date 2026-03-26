use anyhow::{Context, Result};
use std::path::Path;

use crate::events::{Event, EventType, Severity};
use crate::parsers::LogParser;
use crate::rules::{self, RuleEngine};

/// Parser for Claude Code JSONL session logs.
///
/// Claude Code writes session logs as JSONL files in:
/// `~/.claude/projects/<project>/session-*.jsonl`
///
/// Each line is a JSON object with at minimum a "type" field.
/// Common types: "assistant", "user", "tool_use", "tool_result", "system".
pub struct ClaudeLogParser {
    engine: std::cell::RefCell<RuleEngine>,
}

impl ClaudeLogParser {
    pub fn new() -> Self {
        Self {
            engine: std::cell::RefCell::new(RuleEngine::new(rules::default_rules())),
        }
    }

    /// Parse a single JSONL line and detect events.
    fn parse_line(&self, line: &str) -> Vec<Event> {
        let mut events = Vec::new();

        let obj: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => return events,
        };

        let msg_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");

        // Check tool_result for errors
        if msg_type == "tool_result" {
            if let Some(true) = obj.get("is_error").and_then(|v| v.as_bool()) {
                let content = obj
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error");
                let tool_name = obj
                    .get("tool_use_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                events.push(Event::new(
                    EventType::ToolCallError,
                    format!("Tool error in {tool_name}: {}", truncate(content, 120)),
                    Severity::Medium,
                    "claude".to_string(),
                    serde_json::json!({
                        "tool_use_id": tool_name,
                        "error": content,
                    }),
                ));
            }
        }

        // Run rule engine against full content
        let content_str = match msg_type {
            "assistant" => obj
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| {
                    if let Some(s) = c.as_str() {
                        Some(s.to_string())
                    } else if let Some(arr) = c.as_array() {
                        Some(
                            arr.iter()
                                .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                                .collect::<Vec<_>>()
                                .join("\n"),
                        )
                    } else {
                        None
                    }
                }),
            "tool_result" => obj
                .get("content")
                .and_then(|v| v.as_str())
                .map(String::from),
            _ => None,
        };

        if let Some(content) = content_str {
            let mut engine = self.engine.borrow_mut();
            let matches = engine.evaluate(&content);
            let rule_events = engine.matches_to_events(&matches, "claude");
            events.extend(rule_events);
        }

        events
    }
}

impl Default for ClaudeLogParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LogParser for ClaudeLogParser {
    fn parse(&self, path: &Path) -> Result<Vec<Event>> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading log file: {}", path.display()))?;

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
        "claude"
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

/// Find Claude Code session log files in the default location.
pub fn find_session_logs(project_dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let claude_dir = project_dir.join(".claude");
    if !claude_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut logs = Vec::new();
    // Look for projects/*/session-*.jsonl
    if let Ok(entries) = std::fs::read_dir(claude_dir.join("projects")) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if let Ok(files) = std::fs::read_dir(entry.path()) {
                    for file in files.flatten() {
                        let name = file.file_name();
                        let name_str = name.to_string_lossy();
                        if name_str.ends_with(".jsonl") {
                            logs.push(file.path());
                        }
                    }
                }
            }
        }
    }

    logs.sort();
    Ok(logs)
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_parse_tool_error() {
        let parser = ClaudeLogParser::new();
        let log = write_temp_log(&[
            r#"{"type": "tool_result", "is_error": true, "content": "No such file or directory", "tool_use_id": "bash_123"}"#,
        ]);
        let events = parser.parse(log.path()).unwrap();
        // Should get at least a tool_call_error event
        assert!(events
            .iter()
            .any(|e| e.event_type == EventType::ToolCallError));
    }

    #[test]
    fn test_parse_clean_log() {
        let parser = ClaudeLogParser::new();
        let log = write_temp_log(&[
            r#"{"type": "user", "message": "hello"}"#,
            r#"{"type": "assistant", "message": {"content": "Hi there!"}}"#,
        ]);
        let events = parser.parse(log.path()).unwrap();
        // No rule violations in clean output
        assert!(events.is_empty());
    }

    #[test]
    fn test_parse_secret_in_output() {
        let parser = ClaudeLogParser::new();
        let log = write_temp_log(&[
            r#"{"type": "tool_result", "content": "config: API_KEY=sk-abc123secret"}"#,
        ]);
        let events = parser.parse(log.path()).unwrap();
        assert!(events
            .iter()
            .any(|e| e.event_type == EventType::ComplianceViolation));
    }

    #[test]
    fn test_parse_empty_file() {
        let parser = ClaudeLogParser::new();
        let log = write_temp_log(&[]);
        let events = parser.parse(log.path()).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_parse_malformed_json() {
        let parser = ClaudeLogParser::new();
        let log = write_temp_log(&["not json at all", "{invalid", ""]);
        let events = parser.parse(log.path()).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_parse_line_no_error() {
        let parser = ClaudeLogParser::new();
        let events = parser
            .parse_line(r#"{"type": "tool_result", "is_error": false, "content": "success"}"#);
        assert!(events.is_empty());
    }
}
