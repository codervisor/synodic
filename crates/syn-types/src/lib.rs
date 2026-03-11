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
