use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Standalone AI coding evaluation framework
#[derive(Parser)]
#[command(name = "synodic-eval", version, about)]
struct Cli {
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

    /// Run batch evaluation across task x skill matrix
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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let project_root = synodic_eval::util::find_project_root()?;

    match cli.command {
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
            let result = synodic_eval::run::execute(synodic_eval::run::RunOptions {
                alias,
                skill,
                testbed_dir,
                skip_setup,
                skip_agent,
                agent_cmd,
                output,
                dry_run,
                split,
                project_root,
            })?;
            if !result.resolved {
                std::process::exit(1);
            }
            Ok(())
        }
        EvalSubcommand::Score {
            instance_id,
            testbed_dir,
            output,
        } => {
            let testbed = testbed_dir.unwrap_or_else(|| {
                let base = if instance_id.contains("__") {
                    "/tmp/swebench-testbed"
                } else {
                    "/tmp/featurebench-testbed"
                };
                format!("{}/{}", base, instance_id)
            });
            let output_path = output.map(PathBuf::from);
            synodic_eval::score::verdict::score(
                &instance_id,
                std::path::Path::new(&testbed),
                output_path.as_deref(),
            )?;
            Ok(())
        }
        EvalSubcommand::List { tag, json } => {
            synodic_eval::list::list_evals(&project_root, tag.as_deref(), json)
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
        } => synodic_eval::batch::execute(
            &project_root,
            synodic_eval::batch::BatchOptions {
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
            let results_dir = project_root.join("evals/results");

            let mode = if all {
                synodic_eval::report::ReportMode::All
            } else if latest {
                synodic_eval::report::ReportMode::Latest
            } else if let Some(dirs) = compare {
                synodic_eval::report::ReportMode::Compare(
                    dirs[0].clone(),
                    dirs[1].clone(),
                )
            } else if let Some(dir) = batch_dir {
                synodic_eval::report::ReportMode::Single(PathBuf::from(dir))
            } else {
                anyhow::bail!(
                    "Specify a batch directory, --latest, --all, or --compare"
                );
            };

            let fmt = match format.as_str() {
                "json" => synodic_eval::report::ReportFormat::Json,
                "csv" => synodic_eval::report::ReportFormat::Csv,
                _ => synodic_eval::report::ReportFormat::Table,
            };

            synodic_eval::report::generate(&results_dir, mode, fmt)
        }
    }
}
