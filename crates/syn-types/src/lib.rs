use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Identifies which station a work item is at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StationId {
    Build,
    Inspect,
}

impl std::fmt::Display for StationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StationId::Build => write!(f, "BUILD"),
            StationId::Inspect => write!(f, "INSPECT"),
        }
    }
}

/// Outcome produced by a station after processing a work item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StationOutcome {
    /// Move to the next station.
    Pass { next: StationId },
    /// Send back for rework with feedback.
    Rework {
        back_to: StationId,
        feedback: String,
    },
    /// Final approval — work is done.
    Approved,
    /// Human intervention needed.
    Escalate { reason: String },
}

/// A record of one station transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationTransition {
    pub from: StationId,
    pub to: Option<StationId>,
    pub outcome: StationOutcome,
    pub timestamp: DateTime<Utc>,
    pub tokens_used: u64,
    /// Elapsed wall-clock milliseconds for this station invocation.
    #[serde(default)]
    pub duration_ms: u64,
}

/// Metrics tracked across the lifetime of a work item.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkMetrics {
    pub total_tokens: u64,
    pub cycle_time_secs: Option<f64>,
    pub first_pass_yield: Option<bool>,
    pub rework_count: u32,
}

/// The central work item that flows through the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItem {
    pub id: String,
    pub spec_path: PathBuf,
    pub station: StationId,
    pub attempt: u32,
    pub branch: String,
    pub artifacts_dir: PathBuf,
    pub history: Vec<StationTransition>,
    pub started_at: DateTime<Utc>,
    pub metrics: WorkMetrics,
    /// Rework feedback from INSPECT, consumed by the next BUILD cycle.
    #[serde(default)]
    pub rework_feedback: Option<String>,
}

/// Structured report produced by the BUILD station.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildReport {
    pub work_id: String,
    pub spec_path: PathBuf,
    pub branch: String,
    pub files_changed: Vec<String>,
    pub tests_passed: bool,
    pub summary: String,
    pub tokens_used: u64,
    pub timestamp: DateTime<Utc>,
}

/// Structured report produced by the INSPECT station.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewReport {
    pub work_id: String,
    pub approved: bool,
    pub review_comments: String,
    pub rework_items: Vec<String>,
    pub tokens_used: u64,
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_transition(duration_ms: u64) -> StationTransition {
        StationTransition {
            from: StationId::Build,
            to: Some(StationId::Inspect),
            outcome: StationOutcome::Pass {
                next: StationId::Inspect,
            },
            timestamp: Utc::now(),
            tokens_used: 42,
            duration_ms,
        }
    }

    #[test]
    fn test_duration_ms_round_trip() {
        let transition = sample_transition(500);
        let json = serde_json::to_string(&transition).unwrap();
        let deserialized: StationTransition = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.duration_ms, 500);
    }

    #[test]
    fn test_deserialize_without_duration_ms_defaults_to_zero() {
        let json = r#"{
            "from": "build",
            "to": "inspect",
            "outcome": {"type": "pass", "next": "inspect"},
            "timestamp": "2025-01-01T00:00:00Z",
            "tokens_used": 10
        }"#;
        let transition: StationTransition = serde_json::from_str(json).unwrap();
        assert_eq!(transition.duration_ms, 0);
    }

    #[test]
    fn test_deserialize_with_duration_ms() {
        let json = r#"{
            "from": "build",
            "to": "inspect",
            "outcome": {"type": "pass", "next": "inspect"},
            "timestamp": "2025-01-01T00:00:00Z",
            "tokens_used": 10,
            "duration_ms": 1234
        }"#;
        let transition: StationTransition = serde_json::from_str(json).unwrap();
        assert_eq!(transition.duration_ms, 1234);
    }

    #[test]
    fn test_construct_station_transition_with_all_fields() {
        let transition = StationTransition {
            from: StationId::Inspect,
            to: None,
            outcome: StationOutcome::Approved,
            timestamp: Utc::now(),
            tokens_used: 100,
            duration_ms: 9999,
        };
        assert_eq!(transition.duration_ms, 9999);
        assert_eq!(transition.from, StationId::Inspect);
        assert!(transition.to.is_none());
        assert_eq!(transition.tokens_used, 100);
    }
}
