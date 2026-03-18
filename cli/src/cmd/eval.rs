use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::eval as eval_mod;
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

    /// Run batch evaluation across task × skill matrix
    Batch {
        /// Comma-separated task aliases (default: all e2e tasks)
        #[arg(long)]
        tasks: Option<String>,

        /// Comma-separated skills to compare
        #[arg(long, default_value = "fractal,factory,baseline")]
        skills: String,

        /// Filter to one benchmark: swebench, featurebench, devbench
        #[arg(long)]
        benchmark: Option<String>,

        /// SWE-bench split
        #[arg(long, default_value = "pro")]
        split: String,

        /// Directory for results
        #[arg(long)]
        output_dir: Option<String>,

        /// Agent command
        #[arg(long, default_value = "claude")]
        agent_cmd: String,

        /// Print what would run without executing
        #[arg(long)]
        dry_run: bool,

        /// Skip testbed setup
        #[arg(long)]
        skip_setup: bool,

        /// Resume from last incomplete batch
        #[arg(long)]
        resume: bool,
    },

    /// Generate a report from batch eval results
    Report {
        /// Batch results directory
        batch_dir: Option<String>,

        /// Report across all batches
        #[arg(long)]
        all: bool,

        /// Report from most recent batch
        #[arg(long)]
        latest: bool,

        /// Compare two batch runs side-by-side (provide two batch names)
        #[arg(long, num_args = 2)]
        compare: Option<Vec<String>>,

        /// Output format: table, json, csv
        #[arg(long, default_value = "table")]
        format: String,
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
            } => eval_mod::run::execute(eval_mod::run::RunOptions {
                alias,
                skill,
                testbed_dir,
                skip_setup,
                skip_agent,
                agent_cmd,
                output,
                dry_run,
                split,
                repo_root,
            }),
            EvalSubcommand::Score {
                instance_id,
                testbed_dir,
                output,
            } => {
                let testbed = testbed_dir.unwrap_or_else(|| {
                    let base = if instance_id.contains("__") {
                        // SWE-bench IDs use double-underscore (e.g. django__django-10097)
                        "/tmp/swebench-testbed"
                    } else {
                        "/tmp/featurebench-testbed"
                    };
                    format!("{}/{}", base, instance_id)
                });
                let output_path = output.map(std::path::PathBuf::from);
                eval_mod::score::verdict::score(
                    &instance_id,
                    std::path::Path::new(&testbed),
                    output_path.as_deref(),
                )?;
                Ok(())
            }
            EvalSubcommand::List { tag, json } => {
                eval_mod::list::list_evals(&repo_root, tag.as_deref(), json)
            }
            EvalSubcommand::Batch {
                tasks,
                skills,
                benchmark,
                split,
                output_dir,
                agent_cmd,
                dry_run,
                skip_setup,
                resume,
            } => eval_mod::batch::execute(
                &repo_root,
                eval_mod::batch::BatchOptions {
                    tasks,
                    skills,
                    benchmark,
                    split,
                    output_dir,
                    agent_cmd,
                    dry_run,
                    skip_setup,
                    resume,
                },
            ),
            EvalSubcommand::Report {
                batch_dir,
                all,
                latest,
                compare,
                format,
            } => {
                let results_dir = repo_root.join("evals/results");

                let mode = if all {
                    eval_mod::report::ReportMode::All
                } else if latest {
                    eval_mod::report::ReportMode::Latest
                } else if let Some(dirs) = compare {
                    eval_mod::report::ReportMode::Compare(
                        dirs[0].clone(),
                        dirs[1].clone(),
                    )
                } else if let Some(dir) = batch_dir {
                    eval_mod::report::ReportMode::Single(PathBuf::from(dir))
                } else {
                    anyhow::bail!(
                        "Specify a batch directory, --latest, --all, or --compare"
                    );
                };

                let fmt = match format.as_str() {
                    "json" => eval_mod::report::ReportFormat::Json,
                    "csv" => eval_mod::report::ReportFormat::Csv,
                    _ => eval_mod::report::ReportFormat::Table,
                };

                eval_mod::report::generate(&results_dir, mode, fmt)
            }
        }
    }
}
