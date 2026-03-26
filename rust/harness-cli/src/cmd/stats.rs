use anyhow::Result;
use clap::Args;

use harness_core::events::EventFilter;
use harness_core::storage::sqlite::SqliteStore;
use harness_core::storage::EventStore;

use crate::util;

#[derive(Args)]
pub struct StatsCmd {
    /// Output as JSON
    #[arg(long)]
    json: bool,
}

impl StatsCmd {
    pub fn run(self) -> Result<()> {
        let store = open_store()?;
        let stats = store.stats(&EventFilter::default())?;

        if self.json {
            println!("{}", serde_json::to_string_pretty(&stats)?);
        } else {
            println!("Total events:    {}", stats.total);
            println!("Unresolved:      {}", stats.unresolved);
            println!(
                "Resolution rate: {:.0}%",
                if stats.total > 0 {
                    ((stats.total - stats.unresolved) as f64 / stats.total as f64) * 100.0
                } else {
                    0.0
                }
            );

            if !stats.by_type.is_empty() {
                println!("\nBy type:");
                let mut types: Vec<_> = stats.by_type.iter().collect();
                types.sort_by(|a, b| b.1.cmp(a.1));
                for (t, count) in types {
                    println!("  {:<25} {}", t, count);
                }
            }
            if !stats.by_severity.is_empty() {
                println!("\nBy severity:");
                for sev in ["critical", "high", "medium", "low"] {
                    if let Some(count) = stats.by_severity.get(sev) {
                        println!("  {:<25} {}", sev, count);
                    }
                }
            }
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
