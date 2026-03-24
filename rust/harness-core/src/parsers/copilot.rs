use anyhow::{Context, Result};
use std::path::Path;

use crate::events::{Event, EventType, Severity};
use crate::parsers::LogParser;
use crate::rules::{self, RuleEngine};

/// Parser for GitHub Copilot event logs.
///
/// Copilot Chat and Copilot agent write event logs as JSONL in:
/// - VS Code: `~/.vscode/extensions/github.copilot-chat-*/` conversation logs
/// - Copilot agent: `~/.config/github-copilot/events.jsonl`
///
/// Each line is a JSON object. We look for fields like "event", "type",
/// "outcome", "error", "suggestion", and "properties".
pub struct CopilotLogParser {
    engine: std::cell::RefCell<RuleEngine>,
}

impl CopilotLogParser {
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

        // Copilot events use "event" or "type" as discriminator
        let event_name = obj
            .get("event")
            .or_else(|| obj.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Detect tool/command errors
        if matches!(event_name, "error" | "tool_error" | "command_error") {
            let message = obj
                .get("message")
                .or_else(|| obj.get("error"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            let tool = obj
                .get("tool")
                .or_else(|| obj.get("command"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");

            events.push(Event::new(
                EventType::ToolCallError,
                format!("Copilot tool error in {tool}: {}", truncate(message, 120)),
                Severity::Medium,
                "copilot".to_string(),
                serde_json::json!({
                    "tool": tool,
                    "error": message,
                }),
            ));
        }

        // Detect outcome=error in completion/suggestion events
        if let Some(outcome) = obj.get("outcome").and_then(|v| v.as_str()) {
            if outcome == "error" || outcome == "failed" {
                let detail = obj
                    .get("message")
                    .or_else(|| obj.get("reason"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("no details");

                events.push(Event::new(
                    EventType::ToolCallError,
                    format!("Copilot {event_name} failed: {}", truncate(detail, 120)),
                    Severity::Low,
                    "copilot".to_string(),
                    serde_json::json!({
                        "event": event_name,
                        "outcome": outcome,
                        "detail": detail,
                    }),
                ));
            }
        }

        // Detect rejected/blocked completions (potential misalignment indicator)
        if event_name == "completion_rejected" || event_name == "suggestion_rejected" {
            let reason = obj
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("no reason");
            if reason.contains("content_filter") || reason.contains("blocked") {
                events.push(Event::new(
                    EventType::ComplianceViolation,
                    format!("Copilot completion blocked: {}", truncate(reason, 120)),
                    Severity::High,
                    "copilot".to_string(),
                    serde_json::json!({
                        "event": event_name,
                        "reason": reason,
                    }),
                ));
            }
        }

        // Detect agent action events with error states
        if event_name == "agent_action" || event_name == "chat_action" {
            if let Some(status) = obj.get("status").and_then(|v| v.as_str()) {
                if status == "error" || status == "failed" {
                    let action = obj
                        .get("action")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let detail = obj
                        .get("error")
                        .or_else(|| obj.get("message"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("no details");

                    events.push(Event::new(
                        EventType::ToolCallError,
                        format!("Copilot agent action failed: {action}: {}", truncate(detail, 120)),
                        Severity::Medium,
                        "copilot".to_string(),
                        serde_json::json!({
                            "action": action,
                            "status": status,
                            "error": detail,
                        }),
                    ));
                }
            }
        }

        // Run rule engine against content fields
        let content_str = extract_content(&obj);
        if let Some(content) = content_str {
            let mut engine = self.engine.borrow_mut();
            let matches = engine.evaluate(&content);
            let rule_events = engine.matches_to_events(&matches, "copilot");
            events.extend(rule_events);
        }

        events
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

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
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
                    if name.starts_with("github.copilot-chat-") || name.starts_with("github.copilot-") {
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
    std::env::var("HOME")
        .ok()
        .map(std::path::PathBuf::from)
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
        let parser = CopilotLogParser::new();
        let log = write_temp_log(&[
            r#"{"event": "tool_error", "tool": "terminal", "message": "Command failed with exit code 1"}"#,
        ]);
        let events = parser.parse(log.path()).unwrap();
        assert!(events.iter().any(|e| e.event_type == EventType::ToolCallError));
        assert!(events.iter().any(|e| e.source == "copilot"));
    }

    #[test]
    fn test_parse_outcome_error() {
        let parser = CopilotLogParser::new();
        let log = write_temp_log(&[
            r#"{"event": "completion", "outcome": "error", "message": "Rate limit exceeded"}"#,
        ]);
        let events = parser.parse(log.path()).unwrap();
        assert!(events.iter().any(|e| e.event_type == EventType::ToolCallError));
    }

    #[test]
    fn test_parse_content_filter_blocked() {
        let parser = CopilotLogParser::new();
        let log = write_temp_log(&[
            r#"{"event": "completion_rejected", "reason": "content_filter: potentially unsafe"}"#,
        ]);
        let events = parser.parse(log.path()).unwrap();
        assert!(events.iter().any(|e| e.event_type == EventType::ComplianceViolation));
    }

    #[test]
    fn test_parse_agent_action_failed() {
        let parser = CopilotLogParser::new();
        let log = write_temp_log(&[
            r#"{"event": "agent_action", "action": "file_edit", "status": "error", "error": "Permission denied"}"#,
        ]);
        let events = parser.parse(log.path()).unwrap();
        assert!(events.iter().any(|e| e.event_type == EventType::ToolCallError));
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
        let log = write_temp_log(&[
            r#"{"event": "completion", "output": "const API_KEY=sk-secret123"}"#,
        ]);
        let events = parser.parse(log.path()).unwrap();
        assert!(events.iter().any(|e| e.event_type == EventType::ComplianceViolation));
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
    fn test_source_name() {
        let parser = CopilotLogParser::new();
        assert_eq!(parser.source(), "copilot");
    }
}
