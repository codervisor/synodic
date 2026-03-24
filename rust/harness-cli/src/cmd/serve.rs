use anyhow::Result;
use clap::Args;

use crate::util;

#[derive(Args)]
pub struct ServeCmd {
    /// Port to listen on
    #[arg(long, default_value = "3000")]
    port: u16,
}

impl ServeCmd {
    pub fn run(self) -> Result<()> {
        let root = util::find_repo_root()?;
        let db_path = root.join(".harness").join("synodic.db");
        if !db_path.exists() {
            anyhow::bail!(
                "Database not found at {}. Run `synodic init` first.",
                db_path.display()
            );
        }

        // Launch synodic-http binary
        let exe_dir = std::env::current_exe()?
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        let http_bin = exe_dir.join("synodic-http");

        if !http_bin.exists() {
            anyhow::bail!(
                "synodic-http binary not found at {}. Build it with `cargo build`.",
                http_bin.display()
            );
        }

        let status = std::process::Command::new(http_bin)
            .env("DATABASE_URL", &db_path)
            .env("PORT", self.port.to_string())
            .status()?;

        std::process::exit(status.code().unwrap_or(1));
    }
}
