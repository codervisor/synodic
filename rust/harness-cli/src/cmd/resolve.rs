use anyhow::Result;
use clap::Args;

use harness_core::storage::sqlite::SqliteStore;
use harness_core::storage::EventStore;

use crate::util;

#[derive(Args)]
pub struct ResolveCmd {
    /// Event ID to resolve
    id: String,

    /// Resolution notes
    #[arg(long, default_value = "")]
    notes: String,
}

impl ResolveCmd {
    pub fn run(self) -> Result<()> {
        let store = open_store()?;
        store.resolve(&self.id, &self.notes)?;
        eprintln!("Resolved: {}", self.id);
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
