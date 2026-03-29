use clap::{Args, Subcommand};

use crate::harness;
use crate::util;

#[derive(Args)]
pub struct GovernanceCmd {
    #[command(subcommand)]
    command: GovernanceSubcommand,
}

#[derive(Subcommand)]
enum GovernanceSubcommand {
    /// Execute an agent command with governance checkpoints
    Run {
        /// Max rework cycles before escalation
        #[arg(long, default_value = "3")]
        max_rework: u32,

        /// Working directory to observe
        #[arg(long)]
        workdir: Option<String>,

        /// Skip Layer 2 AI judge
        #[arg(long)]
        no_l2: bool,

        /// AI judge command
        #[arg(long, default_value = "claude")]
        judge: String,

        /// Git base ref for diff
        #[arg(long)]
        base_ref: Option<String>,

        /// Show what would happen without executing
        #[arg(long)]
        dry_run: bool,

        /// Minimal output
        #[arg(long, short)]
        quiet: bool,

        /// Machine-readable JSON output
        #[arg(long)]
        json: bool,

        /// Agent command (everything after --)
        #[arg(last = true)]
        agent_cmd: Vec<String>,
    },

    /// Display governance log
    Log {
        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Show last N entries
        #[arg(long, default_value = "20")]
        tail: usize,
    },
}

impl GovernanceCmd {
    pub fn run(self) -> anyhow::Result<()> {
        match self.command {
            GovernanceSubcommand::Run {
                max_rework,
                workdir,
                no_l2,
                judge,
                base_ref,
                dry_run,
                quiet,
                json,
                agent_cmd,
            } => {
                if agent_cmd.is_empty() {
                    anyhow::bail!("agent command (after --) is required");
                }
                harness::run::execute(harness::run::RunConfig {
                    max_rework,
                    workdir,
                    no_l2,
                    judge,
                    base_ref,
                    dry_run,
                    quiet,
                    json_output: json,
                    agent_cmd,
                })
            }
            GovernanceSubcommand::Log { json, tail } => {
                let repo_root = util::find_repo_root()?;
                let harness_dir = resolve_harness_dir(&repo_root)?;
                harness::log::display(&harness_dir, json, tail)
            }
        }
    }
}

fn resolve_harness_dir(repo_root: &std::path::Path) -> anyhow::Result<std::path::PathBuf> {
    if let Ok(dir) = std::env::var("HARNESS_DIR") {
        let p = std::path::PathBuf::from(dir);
        if p.is_dir() {
            return Ok(p);
        }
    }
    let hdir = repo_root.join(".harness");
    if !hdir.is_dir() {
        anyhow::bail!(".harness/ directory not found at {}", hdir.display());
    }
    Ok(hdir)
}
