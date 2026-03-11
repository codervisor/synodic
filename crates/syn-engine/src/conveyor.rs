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
