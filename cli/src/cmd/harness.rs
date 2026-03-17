use clap::{Args, Subcommand};

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
        #[arg(long)]
        tail: Option<u32>,
    },

    /// List crystallized rules
    Rules,
}

impl HarnessCmd {
    pub fn run(self) -> anyhow::Result<()> {
        let repo_root = util::find_repo_root()?;
        let harness_script = repo_root.join("harness");

        if !harness_script.exists() {
            anyhow::bail!(
                "harness script not found at {}",
                harness_script.display()
            );
        }

        let mut args: Vec<String> = Vec::new();

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
                args.push("run".into());
                args.push("--max-rework".into());
                args.push(max_rework.to_string());
                if let Some(dir) = workdir {
                    args.push("--workdir".into());
                    args.push(dir);
                }
                if no_l2 {
                    args.push("--no-l2".into());
                }
                args.push("--judge".into());
                args.push(judge);
                if let Some(r) = base_ref {
                    args.push("--base-ref".into());
                    args.push(r);
                }
                if dry_run {
                    args.push("--dry-run".into());
                }
                if quiet {
                    args.push("--quiet".into());
                }
                if json {
                    args.push("--json".into());
                }
                args.push("--".into());
                args.extend(agent_cmd);
            }
            HarnessSubcommand::Eval { args: extra } => {
                args.push("eval".into());
                args.extend(extra);
            }
            HarnessSubcommand::Log { json, tail } => {
                args.push("log".into());
                if json {
                    args.push("--json".into());
                }
                if let Some(n) = tail {
                    args.push("--tail".into());
                    args.push(n.to_string());
                }
            }
            HarnessSubcommand::Rules => {
                args.push("rules".into());
            }
        }

        util::exec_script(&harness_script, &args)
    }
}
