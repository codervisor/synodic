use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use syn_engine::conveyor::{load_manifest, save_manifest};
use syn_engine::metrics::print_summary;
use syn_types::{StationId, WorkItem, WorkMetrics};

#[derive(Parser)]
#[command(name = "synodic", version, about = "AI-native agent orchestration — Factory MVP")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a spec through the BUILD → INSPECT pipeline
    Run {
        /// Path to the spec directory (e.g., specs/004-fleet-process-supervisor)
        spec_path: PathBuf,
    },
    /// Show status of a work item
    Status {
        /// Work item ID (e.g., work-001)
        work_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { spec_path } => cmd_run(spec_path).await,
        Commands::Status { work_id } => cmd_status(work_id).await,
    }
}

async fn cmd_run(spec_path: PathBuf) -> Result<()> {
    let repo_root = find_repo_root()?;

    // Validate spec exists.
    let full_spec = repo_root.join(&spec_path);
    if !full_spec.join("README.md").exists() {
        anyhow::bail!(
            "Spec not found: {} (expected README.md at {})",
            spec_path.display(),
            full_spec.join("README.md").display()
        );
    }

    // Generate a unique work ID.
    let short_id = &uuid::Uuid::new_v4().to_string()[..8];
    let work_id = format!("work-{short_id}");

    // Create artifacts directory.
    let artifacts_dir = repo_root.join(".factory").join(&work_id);
    tokio::fs::create_dir_all(&artifacts_dir)
        .await
        .context("Failed to create artifacts directory")?;

    let branch = format!("factory/{work_id}");
    let started_at = Utc::now();

    let mut item = WorkItem {
        id: work_id.clone(),
        spec_path: spec_path.clone(),
        station: StationId::Build,
        attempt: 1,
        branch: branch.clone(),
        artifacts_dir: artifacts_dir.clone(),
        history: vec![],
        started_at,
        metrics: WorkMetrics::default(),
        rework_feedback: None,
    };

    // Save initial manifest.
    save_manifest(&item).await?;

    eprintln!("╔══════════════════════════════════════╗");
    eprintln!("║        Synodic Factory — Run         ║");
    eprintln!("╠══════════════════════════════════════╣");
    eprintln!("║  Work ID:  {:<26}║", work_id);
    eprintln!("║  Spec:     {:<26}║", spec_path.display());
    eprintln!("║  Branch:   {:<26}║", branch);
    eprintln!("╚══════════════════════════════════════╝");

    // Run the pipeline.
    let result = syn_engine::conveyor::run_pipeline(&mut item, &repo_root).await;

    // Compute cycle time.
    let elapsed = Utc::now() - started_at;
    item.metrics.cycle_time_secs = Some(elapsed.num_milliseconds() as f64 / 1000.0);

    // Save final manifest.
    save_manifest(&item).await?;

    // Switch back to main branch (best-effort).
    let _ = syn_engine::agent::git(&repo_root, &["checkout", "main"]).await;

    match result {
        Ok(()) => {
            print_summary(&item);
            println!(
                "\nFactory run complete. Branch '{}' is ready for PR.",
                item.branch
            );
            Ok(())
        }
        Err(e) => {
            print_summary(&item);
            eprintln!("\nFactory run FAILED: {e:#}");
            Err(e)
        }
    }
}

async fn cmd_status(work_id: String) -> Result<()> {
    let repo_root = find_repo_root()?;
    let artifacts_dir = repo_root.join(".factory").join(&work_id);

    if !artifacts_dir.exists() {
        anyhow::bail!(
            "Work item '{}' not found. No directory at {}",
            work_id,
            artifacts_dir.display()
        );
    }

    let item = load_manifest(&artifacts_dir).await?;

    println!("Work Item: {}", item.id);
    println!("Spec:      {}", item.spec_path.display());
    println!("Station:   {}", item.station);
    println!("Attempt:   {}", item.attempt);
    println!("Branch:    {}", item.branch);
    println!("Started:   {}", item.started_at);

    if let Some(secs) = item.metrics.cycle_time_secs {
        println!("Cycle time: {:.1}s", secs);
    }
    println!("Tokens:    {}", item.metrics.total_tokens);
    println!("Reworks:   {}", item.metrics.rework_count);
    if let Some(fpy) = item.metrics.first_pass_yield {
        println!("First-pass yield: {}", if fpy { "yes" } else { "no" });
    }

    println!("\nHistory ({} transitions):", item.history.len());
    for (i, t) in item.history.iter().enumerate() {
        let to_str = t
            .to
            .map(|s| s.to_string())
            .unwrap_or_else(|| "DONE".to_string());
        println!("  {}. {} → {} at {}", i + 1, t.from, to_str, t.timestamp);
    }

    Ok(())
}

/// Walk up from CWD to find the git repo root.
fn find_repo_root() -> Result<PathBuf> {
    let mut dir = std::env::current_dir().context("Failed to get current directory")?;
    loop {
        if dir.join(".git").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            anyhow::bail!("Not inside a git repository");
        }
    }
}
