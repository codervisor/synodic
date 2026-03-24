use anyhow::Result;
use clap::Args;

use harness_core::parsers::claude::{self, ClaudeLogParser};
use harness_core::parsers::LogParser;
use harness_core::storage::sqlite::SqliteStore;
use harness_core::storage::EventStore;

use crate::util;

#[derive(Args)]
pub struct CollectCmd {
    /// Log source: claude, auto (default: auto)
    #[arg(long, default_value = "auto")]
    source: String,

    /// Only collect events from the last N hours/minutes (e.g., "1h", "30m")
    #[arg(long)]
    since: Option<String>,

    /// Show what would be collected without inserting
    #[arg(long)]
    dry_run: bool,
}

impl CollectCmd {
    pub fn run(self) -> Result<()> {
        let root = util::find_repo_root()?;
        let db_path = root.join(".harness").join("synodic.db");

        let store = if self.dry_run {
            None
        } else {
            if !db_path.exists() {
                anyhow::bail!(
                    "Database not found at {}. Run `synodic init` first.",
                    db_path.display()
                );
            }
            Some(SqliteStore::open(&db_path)?)
        };

        let mut total = 0;

        // Claude Code logs
        if self.source == "auto" || self.source == "claude" {
            let logs = claude::find_session_logs(&root)?;
            if logs.is_empty() {
                eprintln!("No Claude Code session logs found in {}", root.display());
            } else {
                let parser = ClaudeLogParser::new();
                for log_path in &logs {
                    let events = parser.parse(log_path)?;
                    if events.is_empty() {
                        continue;
                    }
                    eprintln!(
                        "{}: {} event(s)",
                        log_path.file_name().unwrap_or_default().to_string_lossy(),
                        events.len()
                    );
                    for event in &events {
                        eprintln!(
                            "  [{:?}] {} ({})",
                            event.severity, event.title, event.event_type
                        );
                        if let Some(ref store) = store {
                            store.insert(event)?;
                        }
                    }
                    total += events.len();
                }
            }
        }

        if self.dry_run {
            eprintln!("\nDry run: would collect {} event(s)", total);
        } else {
            eprintln!("\nCollected {} event(s)", total);
        }

        Ok(())
    }
}
