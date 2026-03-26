use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// Categories of governance events detected in AI agent sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// Tool execution failure (e.g., command returned non-zero, file not found)
    ToolCallError,
    /// Reference to nonexistent files, APIs, or symbols
    Hallucination,
    /// Secrets exposure, dangerous commands, unauthorized prod access
    ComplianceViolation,
    /// Agent action diverges from user intent or spec
    Misalignment,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ToolCallError => "tool_call_error",
            Self::Hallucination => "hallucination",
            Self::ComplianceViolation => "compliance_violation",
            Self::Misalignment => "misalignment",
        }
    }
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for EventType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tool_call_error" => Ok(Self::ToolCallError),
            "hallucination" => Ok(Self::Hallucination),
            "compliance_violation" => Ok(Self::ComplianceViolation),
            "misalignment" => Ok(Self::Misalignment),
            _ => Err(format!("unknown event type: {s}")),
        }
    }
}

/// Severity level of a governance event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Severity {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "critical" => Ok(Self::Critical),
            _ => Err(format!("unknown severity: {s}")),
        }
    }
}

/// A governance event detected in an AI agent session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub event_type: EventType,
    pub title: String,
    pub severity: Severity,
    /// Source agent/tool that produced this event (e.g., "claude", "copilot", "manual")
    pub source: String,
    /// Arbitrary structured metadata
    pub metadata: serde_json::Value,
    pub resolved: bool,
    pub resolution_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl Event {
    /// Create a new event with a generated ULID.
    pub fn new(
        event_type: EventType,
        title: String,
        severity: Severity,
        source: String,
        metadata: serde_json::Value,
    ) -> Self {
        Self {
            id: Ulid::new().to_string(),
            event_type,
            title,
            severity,
            source,
            metadata,
            resolved: false,
            resolution_notes: None,
            created_at: Utc::now(),
            resolved_at: None,
        }
    }
}

/// Filter criteria for querying events.
#[derive(Debug, Default, Clone)]
pub struct EventFilter {
    pub event_type: Option<EventType>,
    pub severity: Option<Severity>,
    pub unresolved_only: bool,
    pub source: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}

/// Aggregate statistics about governance events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stats {
    pub total: usize,
    pub unresolved: usize,
    pub by_type: std::collections::HashMap<String, usize>,
    pub by_severity: std::collections::HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_roundtrip() {
        for et in [
            EventType::ToolCallError,
            EventType::Hallucination,
            EventType::ComplianceViolation,
            EventType::Misalignment,
        ] {
            let s = et.as_str();
            let parsed: EventType = s.parse().unwrap();
            assert_eq!(et, parsed);
        }
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Low < Severity::Medium);
        assert!(Severity::Medium < Severity::High);
        assert!(Severity::High < Severity::Critical);
    }

    #[test]
    fn test_event_new() {
        let event = Event::new(
            EventType::Hallucination,
            "Referenced nonexistent file".to_string(),
            Severity::Medium,
            "claude".to_string(),
            serde_json::json!({"file": "src/missing.rs"}),
        );
        assert!(!event.id.is_empty());
        assert_eq!(event.event_type, EventType::Hallucination);
        assert!(!event.resolved);
        assert!(event.resolution_notes.is_none());
    }

    #[test]
    fn test_event_json_roundtrip() {
        let event = Event::new(
            EventType::ComplianceViolation,
            "Secret in log output".to_string(),
            Severity::Critical,
            "claude".to_string(),
            serde_json::json!({}),
        );
        let json = serde_json::to_string(&event).unwrap();
        let parsed: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, event.id);
        assert_eq!(parsed.event_type, EventType::ComplianceViolation);
        assert_eq!(parsed.severity, Severity::Critical);
    }

    #[test]
    fn test_severity_roundtrip() {
        for s in [
            Severity::Low,
            Severity::Medium,
            Severity::High,
            Severity::Critical,
        ] {
            let str = s.as_str();
            let parsed: Severity = str.parse().unwrap();
            assert_eq!(s, parsed);
        }
    }
}
