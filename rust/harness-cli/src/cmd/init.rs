use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

use crate::util;

#[derive(Args)]
pub struct InitCmd {
    /// Project directory (default: current repo root)
    #[arg(long)]
    dir: Option<String>,
}

impl InitCmd {
    pub fn run(self) -> Result<()> {
        let root = match self.dir {
            Some(d) => PathBuf::from(d),
            None => util::find_repo_root()?,
        };

        let harness_dir = root.join(".harness");
        std::fs::create_dir_all(&harness_dir)?;
        std::fs::create_dir_all(harness_dir.join("rules"))?;
        std::fs::create_dir_all(harness_dir.join("scripts"))?;
        std::fs::create_dir_all(harness_dir.join(".runs"))?;

        // Create SQLite database
        let db_path = harness_dir.join("synodic.db");
        if !db_path.exists() {
            harness_core::storage::sqlite::SqliteStore::open(&db_path)?;
            eprintln!("Created database: {}", db_path.display());
        } else {
            eprintln!("Database already exists: {}", db_path.display());
        }

        // Create default gates.yml if not present
        let gates_path = harness_dir.join("gates.yml");
        if !gates_path.exists() {
            std::fs::write(
                &gates_path,
                "gates:\n  preflight: []\n",
            )?;
            eprintln!("Created: {}", gates_path.display());
        }

        eprintln!("Initialized .harness/ at {}", harness_dir.display());
        Ok(())
    }
}
