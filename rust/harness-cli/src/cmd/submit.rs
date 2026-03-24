use anyhow::Result;
use clap::Args;

use harness_core::events::{Event, EventType, Severity};
use harness_core::storage::sqlite::SqliteStore;
use harness_core::storage::EventStore;

use crate::util;

#[derive(Args)]
pub struct SubmitCmd {
    /// Event type: tool_call_error, hallucination, compliance_violation, misalignment
    #[arg(long, value_name = "TYPE")]
    r#type: String,

    /// Event title
    #[arg(long)]
    title: String,

    /// Severity: low, medium, high, critical
    #[arg(long, default_value = "medium")]
    severity: String,

    /// Source agent (e.g., claude, copilot, manual)
    #[arg(long, default_value = "manual")]
    source: String,

    /// JSON metadata
    #[arg(long, default_value = "{}")]
    metadata: String,
}

impl SubmitCmd {
    pub fn run(self) -> Result<()> {
        let event_type: EventType = self.r#type.parse().map_err(|e| anyhow::anyhow!("{e}"))?;
        let severity: Severity = self.severity.parse().map_err(|e| anyhow::anyhow!("{e}"))?;
        let metadata: serde_json::Value = serde_json::from_str(&self.metadata)?;

        let event = Event::new(event_type, self.title, severity, self.source, metadata);

        let store = open_store()?;
        let id = event.id.clone();
        store.insert(&event)?;

        println!("{id}");
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
