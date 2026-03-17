use clap::{Args, Subcommand};

use crate::util;

#[derive(Args)]
pub struct EvalCmd {
    #[command(subcommand)]
    command: EvalSubcommand,
}

#[derive(Subcommand)]
enum EvalSubcommand {
    /// Run a benchmark evaluation
    Run {
        /// Benchmark alias (e.g. fb:mlflow-tracing, swe:django-16379)
        alias: String,

        /// Skill to invoke: fractal, factory, baseline
        #[arg(long, default_value = "fractal")]
        skill: String,

        /// Override testbed location
        #[arg(long)]
        testbed_dir: Option<String>,

        /// Skip testbed setup
        #[arg(long)]
        skip_setup: bool,

        /// Skip agent invocation (just score existing code)
        #[arg(long)]
        skip_agent: bool,

        /// Agent command
        #[arg(long, default_value = "claude")]
        agent_cmd: String,

        /// Score report output path
        #[arg(long)]
        output: Option<String>,

        /// Print the agent prompt without running
        #[arg(long)]
        dry_run: bool,

        /// SWE-bench split: verified, lite, pro
        #[arg(long, default_value = "verified")]
        split: String,
    },

    /// Score a completed benchmark run
    Score {
        /// Instance ID to score
        instance_id: String,

        /// Testbed directory
        #[arg(long)]
        testbed_dir: Option<String>,

        /// Score report output path
        #[arg(long)]
        output: Option<String>,
    },

    /// List available benchmark tasks
    List {
        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

impl EvalCmd {
    pub fn run(self) -> anyhow::Result<()> {
        let repo_root = util::find_repo_root()?;

        match self.command {
            EvalSubcommand::Run {
                alias,
                skill,
                testbed_dir,
                skip_setup,
                skip_agent,
                agent_cmd,
                output,
                dry_run,
                split,
            } => {
                let script = repo_root.join("evals/run.sh");
                if !script.exists() {
                    anyhow::bail!("evals/run.sh not found at {}", script.display());
                }

                let mut args = vec![alias];
                args.push("--skill".into());
                args.push(skill);
                if let Some(dir) = testbed_dir {
                    args.push("--testbed-dir".into());
                    args.push(dir);
                }
                if skip_setup {
                    args.push("--skip-setup".into());
                }
                if skip_agent {
                    args.push("--skip-agent".into());
                }
                args.push("--agent-cmd".into());
                args.push(agent_cmd);
                if let Some(o) = output {
                    args.push("--output".into());
                    args.push(o);
                }
                if dry_run {
                    args.push("--dry-run".into());
                }
                args.push("--split".into());
                args.push(split);

                util::exec_script(&script, &args)
            }
            EvalSubcommand::Score {
                instance_id,
                testbed_dir,
                output,
            } => {
                let script = repo_root.join("evals/score.sh");
                if !script.exists() {
                    anyhow::bail!("evals/score.sh not found at {}", script.display());
                }

                let mut args = vec![instance_id];
                if let Some(dir) = testbed_dir {
                    args.push("--testbed-dir".into());
                    args.push(dir);
                }
                if let Some(o) = output {
                    args.push("--output".into());
                    args.push(o);
                }

                util::exec_script(&script, &args)
            }
            EvalSubcommand::List { tag, json } => {
                list_evals(&repo_root, tag.as_deref(), json)
            }
        }
    }
}

fn list_evals(
    repo_root: &std::path::Path,
    tag_filter: Option<&str>,
    json_output: bool,
) -> anyhow::Result<()> {
    let evals_path = repo_root.join("evals/evals.json");
    if !evals_path.exists() {
        anyhow::bail!("evals/evals.json not found at {}", evals_path.display());
    }

    let content = std::fs::read_to_string(&evals_path)?;
    let registry: serde_json::Value = serde_json::from_str(&content)?;

    let evals = registry
        .get("evals")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("invalid evals.json: missing 'evals' array"))?;

    let filtered: Vec<&serde_json::Value> = evals
        .iter()
        .filter(|e| {
            if let Some(tag) = tag_filter {
                e.get("tags")
                    .and_then(|t| t.as_array())
                    .map(|tags| tags.iter().any(|t| t.as_str() == Some(tag)))
                    .unwrap_or(false)
            } else {
                true
            }
        })
        .collect();

    if json_output {
        println!("{}", serde_json::to_string_pretty(&filtered)?);
        return Ok(());
    }

    if filtered.is_empty() {
        println!("No evals found.");
        return Ok(());
    }

    // Table header
    println!(
        "{:<40} {:<12} {:<20}",
        "ID", "BENCHMARK", "ALIAS"
    );
    println!("{}", "-".repeat(72));

    for eval in &filtered {
        let id = eval.get("id").and_then(|v| v.as_str()).unwrap_or("?");
        let benchmark = eval
            .get("source")
            .and_then(|s| s.get("benchmark"))
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let alias = eval
            .get("e2e")
            .and_then(|e| e.get("alias"))
            .and_then(|v| v.as_str())
            .unwrap_or("-");

        println!("{:<40} {:<12} {:<20}", id, benchmark, alias);
    }

    println!("\n{} eval(s) found.", filtered.len());
    Ok(())
}
