use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use syn_types::{StationOutcome, StationTransition, WorkItem};

use crate::station::process_station;

/// Maximum number of rework cycles before escalation.
const MAX_REWORK: u32 = 3;

/// Run a work item through the BUILD → INSPECT pipeline with rework loops.
///
/// Returns the final work item on success (approved), or an error on escalation/failure.
pub async fn run_pipeline(item: &mut WorkItem, repo_root: &Path) -> Result<()> {
    // Resume logic: inspect history to determine where to pick up.
    if let Some(last) = item.history.last() {
        match &last.outcome {
            StationOutcome::Pass { next } => {
                item.station = *next;
            }
            StationOutcome::Rework { back_to, feedback } => {
                item.station = *back_to;
                item.rework_feedback = Some(feedback.clone());
            }
            StationOutcome::Approved => {
                eprintln!("Pipeline already approved, nothing to resume.");
                return Ok(());
            }
            StationOutcome::Escalate { .. } => {
                anyhow::bail!("Pipeline was previously escalated, cannot resume.");
            }
        }
    }

    loop {
        let station_id = item.station;
        let outcome = process_station(item, repo_root)
            .await
            .with_context(|| format!("Station {} failed", station_id))?;

        // Record the transition.
        let transition = StationTransition {
            from: station_id,
            to: match &outcome {
                StationOutcome::Pass { next } => Some(*next),
                StationOutcome::Rework { back_to, .. } => Some(*back_to),
                StationOutcome::Approved => None,
                StationOutcome::Escalate { .. } => None,
            },
            outcome: outcome.clone(),
            timestamp: Utc::now(),
            tokens_used: 0, // already tracked in item.metrics
        };
        item.history.push(transition);

        // Persist manifest after each transition.
        save_manifest(item).await?;

        match outcome {
            StationOutcome::Pass { next } => {
                item.station = next;
                item.rework_feedback = None;
            }
            StationOutcome::Rework { back_to, feedback } => {
                item.attempt += 1;
                item.metrics.rework_count += 1;

                if item.attempt > MAX_REWORK {
                    eprintln!(
                        "\n[conveyor] Max rework limit ({}) reached. Escalating.",
                        MAX_REWORK
                    );
                    anyhow::bail!(
                        "Escalation: max rework limit ({}) reached. Last feedback:\n{}",
                        MAX_REWORK,
                        feedback
                    );
                }

                eprintln!(
                    "[conveyor] Rework cycle {} → routing back to {}",
                    item.attempt, back_to
                );
                item.station = back_to;
                item.rework_feedback = Some(feedback);
            }
            StationOutcome::Approved => {
                eprintln!("\n[conveyor] Work item {} APPROVED!", item.id);
                return Ok(());
            }
            StationOutcome::Escalate { reason } => {
                anyhow::bail!("Escalation: {}", reason);
            }
        }
    }
}

/// Save the work item manifest to its artifacts directory.
pub async fn save_manifest(item: &WorkItem) -> Result<()> {
    let manifest_path = item.artifacts_dir.join("manifest.json");
    let json = serde_json::to_string_pretty(item)
        .context("Failed to serialize manifest")?;
    tokio::fs::write(&manifest_path, json)
        .await
        .context("Failed to write manifest")?;
    Ok(())
}

/// Load a work item manifest from a work directory.
pub async fn load_manifest(artifacts_dir: &Path) -> Result<WorkItem> {
    let manifest_path = artifacts_dir.join("manifest.json");
    let json = tokio::fs::read_to_string(&manifest_path)
        .await
        .with_context(|| format!("Failed to read manifest at {}", manifest_path.display()))?;
    let item: WorkItem =
        serde_json::from_str(&json).context("Failed to parse manifest")?;
    Ok(item)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::path::PathBuf;
    use syn_types::{StationId, StationOutcome, StationTransition, WorkItem, WorkMetrics};

    fn make_item(history: Vec<StationTransition>, station: StationId, attempt: u32) -> WorkItem {
        WorkItem {
            id: "test-001".to_string(),
            spec_path: PathBuf::from("specs/test"),
            station,
            attempt,
            branch: "factory/test-001".to_string(),
            artifacts_dir: PathBuf::from("/tmp/syn-test-artifacts"),
            history,
            started_at: Utc::now(),
            metrics: WorkMetrics::default(),
            rework_feedback: None,
        }
    }

    fn make_transition(outcome: StationOutcome, from: StationId) -> StationTransition {
        StationTransition {
            from,
            to: match &outcome {
                StationOutcome::Pass { next } => Some(*next),
                StationOutcome::Rework { back_to, .. } => Some(*back_to),
                _ => None,
            },
            outcome,
            timestamp: Utc::now(),
            tokens_used: 0,
        }
    }

    #[tokio::test]
    async fn resume_empty_history_station_unchanged() {
        let mut item = make_item(vec![], StationId::Build, 1);
        // run_pipeline will fail in the loop (no real repo), but station should stay Build
        let result = run_pipeline(&mut item, Path::new("/nonexistent")).await;
        assert!(result.is_err()); // expected: process_station fails
        assert_eq!(item.station, StationId::Build);
    }

    #[tokio::test]
    async fn resume_after_pass_sets_station_to_next() {
        let history = vec![make_transition(
            StationOutcome::Pass {
                next: StationId::Inspect,
            },
            StationId::Build,
        )];
        let mut item = make_item(history, StationId::Build, 1);
        // run_pipeline will fail in the loop, but station should be set to Inspect
        let result = run_pipeline(&mut item, Path::new("/nonexistent")).await;
        assert!(result.is_err());
        assert_eq!(item.station, StationId::Inspect);
    }

    #[tokio::test]
    async fn resume_after_rework_sets_station_and_feedback() {
        let history = vec![make_transition(
            StationOutcome::Rework {
                back_to: StationId::Build,
                feedback: "fix X".to_string(),
            },
            StationId::Inspect,
        )];
        let mut item = make_item(history, StationId::Inspect, 2);
        let result = run_pipeline(&mut item, Path::new("/nonexistent")).await;
        assert!(result.is_err());
        assert_eq!(item.station, StationId::Build);
        assert_eq!(item.rework_feedback, Some("fix X".to_string()));
    }

    #[tokio::test]
    async fn resume_after_approved_returns_ok() {
        let history = vec![make_transition(StationOutcome::Approved, StationId::Inspect)];
        let mut item = make_item(history, StationId::Inspect, 1);
        let result = run_pipeline(&mut item, Path::new("/nonexistent")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn resume_after_escalate_returns_err() {
        let history = vec![make_transition(
            StationOutcome::Escalate {
                reason: "too many reworks".to_string(),
            },
            StationId::Inspect,
        )];
        let mut item = make_item(history, StationId::Inspect, 3);
        let result = run_pipeline(&mut item, Path::new("/nonexistent")).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("previously escalated"),
            "Expected escalation error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn resume_after_rework_does_not_increment_attempt() {
        let history = vec![make_transition(
            StationOutcome::Rework {
                back_to: StationId::Build,
                feedback: "fix Y".to_string(),
            },
            StationId::Inspect,
        )];
        let mut item = make_item(history, StationId::Inspect, 2);
        let _result = run_pipeline(&mut item, Path::new("/nonexistent")).await;
        // attempt should still be 2 — resume logic must NOT increment it
        assert_eq!(item.attempt, 2);
    }
}
