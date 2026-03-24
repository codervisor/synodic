use anyhow::Result;
use clap::Args;

use harness_core::events::{EventFilter, EventType, Severity};
use harness_core::storage::sqlite::SqliteStore;
use harness_core::storage::EventStore;

use crate::util;

#[derive(Args)]
pub struct ListCmd {
    /// Filter by event type
    #[arg(long, value_name = "TYPE")]
    r#type: Option<String>,

    /// Filter by severity
    #[arg(long)]
    severity: Option<String>,

    /// Show only unresolved events
    #[arg(long)]
    unresolved: bool,

    /// Maximum number of events to show
    #[arg(long, default_value = "50")]
    limit: usize,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

impl ListCmd {
    pub fn run(self) -> Result<()> {
        let store = open_store()?;

        let filter = EventFilter {
            event_type: self
                .r#type
                .as_deref()
                .map(|s| s.parse::<EventType>().map_err(|e| anyhow::anyhow!("{e}")))
                .transpose()?,
            severity: self
                .severity
                .as_deref()
                .map(|s| s.parse::<Severity>().map_err(|e| anyhow::anyhow!("{e}")))
                .transpose()?,
            unresolved_only: self.unresolved,
            limit: Some(self.limit),
            ..Default::default()
        };

        let events = store.list(&filter)?;

        if self.json {
            println!("{}", serde_json::to_string_pretty(&events)?);
        } else if events.is_empty() {
            eprintln!("No events found.");
        } else {
            println!(
                "{:<28} {:<22} {:<10} {:<10} {}",
                "ID", "TYPE", "SEVERITY", "STATUS", "TITLE"
            );
            println!("{}", "-".repeat(90));
            for e in &events {
                let status = if e.resolved { "resolved" } else { "open" };
                let id_short = if e.id.len() > 26 { &e.id[..26] } else { &e.id };
                let title = if e.title.len() > 40 {
                    format!("{}...", &e.title[..37])
                } else {
                    e.title.clone()
                };
                println!(
                    "{:<28} {:<22} {:<10} {:<10} {}",
                    id_short, e.event_type, e.severity, status, title
                );
            }
            eprintln!("\n{} event(s)", events.len());
        }

        Ok(())
    }
}

fn open_store() -> Result<SqliteStore> {
    let root = util::find_repo_root()?;
    let db_path = root.join(".harness").join("synodic.db");
    if !db_path.exists() {
        anyhow::bail!(
            "Database not found at {}. Run `synodic init` first.",
            db_path.display()
        );
    }
    SqliteStore::open(&db_path)
}
