use anyhow::Result;
use clap::Args;

use harness_core::storage::sqlite::SqliteStore;
use harness_core::storage::EventStore;

use crate::util;

#[derive(Args)]
pub struct SearchCmd {
    /// Search query
    query: String,

    /// Maximum results
    #[arg(long, default_value = "20")]
    limit: usize,

    /// Output as JSON
    #[arg(long)]
    json: bool,
}

impl SearchCmd {
    pub fn run(self) -> Result<()> {
        let store = open_store()?;
        let events = store.search(&self.query, self.limit)?;

        if self.json {
            println!("{}", serde_json::to_string_pretty(&events)?);
        } else if events.is_empty() {
            eprintln!("No events matching \"{}\"", self.query);
        } else {
            for e in &events {
                let status = if e.resolved { "resolved" } else { "open" };
                println!("[{}] {} ({}, {}) - {}", e.id, e.title, e.event_type, status, e.created_at.format("%Y-%m-%d %H:%M"));
            }
            eprintln!("\n{} result(s)", events.len());
        }

        Ok(())
    }
}

fn open_store() -> Result<SqliteStore> {
    let root = util::find_repo_root()?;
    let db_path = root.join(".harness").join("synodic.db");
    if !db_path.exists() {
        anyhow::bail!("Database not found. Run `synodic init` first.");
    }
    SqliteStore::open(&db_path)
}
