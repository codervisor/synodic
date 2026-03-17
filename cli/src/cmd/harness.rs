use clap::{Args, Subcommand};

use crate::harness;
use crate::util;

#[derive(Args)]
pub struct HarnessCmd {
    #[command(subcommand)]
    command: HarnessSubcommand,
}

#[derive(Subcommand)]
enum HarnessSubcommand {
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
        #[arg(last = true, required = true)]
        agent_cmd: Vec<String>,
    },

    /// Run Layer 2 evaluation (evaluate_harness.py)
    Eval {
        /// Additional args forwarded to evaluate_harness.py
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
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

    /// List crystallized rules
    Rules,
}

impl HarnessCmd {
    pub fn run(self) -> anyhow::Result<()> {
        match self.command {
            HarnessSubcommand::Run {
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
            HarnessSubcommand::Eval { args } => {
                // Delegate to evaluate_harness.py
                let repo_root = util::find_repo_root()?;
                let harness_dir = resolve_harness_dir(&repo_root)?;
                let script = harness_dir.join("scripts/evaluate_harness.py");
                if !script.exists() {
                    anyhow::bail!(
                        "evaluate_harness.py not found at {}",
                        script.display()
                    );
                }
                let mut cmd_args = vec![
                    script.display().to_string(),
                    harness_dir.display().to_string(),
                ];
                cmd_args.extend(args);
                util::exec_script(std::path::Path::new("python3"), &cmd_args)
            }
            HarnessSubcommand::Log { json, tail } => {
                let repo_root = util::find_repo_root()?;
                let harness_dir = resolve_harness_dir(&repo_root)?;
                harness::log::display(&harness_dir, json, tail)
            }
            HarnessSubcommand::Rules => {
                let repo_root = util::find_repo_root()?;
                let harness_dir = resolve_harness_dir(&repo_root)?;
                harness::rules::list(&harness_dir)
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
