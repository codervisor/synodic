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

    /// Meta-testing: validate environment, analyze strategy, detect false positives/negatives
    Meta {
        /// Benchmark alias (e.g. fb:mlflow-tracing, swe:django-16379)
        alias: String,

        /// Override testbed location
        #[arg(long)]
        testbed_dir: Option<String>,

        /// SWE-bench split
        #[arg(long, default_value = "verified")]
        split: String,

        /// Run only environment checks (pre-test)
        #[arg(long)]
        env_only: bool,

        /// Run only strategy analysis (pre-test)
        #[arg(long)]
        strategy_only: bool,

        /// Analyze quality of a completed run (post-test)
        #[arg(long)]
        quality: bool,

        /// Output as JSON
        #[arg(long)]
        json: bool,
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
        EvalSubcommand::Meta {
            alias,
            testbed_dir,
            split: _,
            env_only,
            strategy_only,
            quality,
            json,
        } => {
            use synodic_eval::meta;
            use synodic_eval::run::resolve_target;

            let target = resolve_target(&alias)?;
            let testbed = testbed_dir.unwrap_or_else(|| {
                format!(
                    "/tmp/{}-testbed/{}",
                    target.benchmark, target.instance_id
                )
            });
            let testbed_path = std::path::Path::new(&testbed);

            let mut env_report = None;
            let mut strategy_report = None;
            let mut quality_report = None;

            // Environment validation (pre-test)
            if !quality || env_only {
                let report = meta::environment::validate(&target.benchmark, testbed_path);
                if !json {
                    println!("━━━ Environment Validation ━━━");
                    println!();
                    for check in &report.checks {
                        let icon = if check.passed { "+" } else { "x" };
                        let sev = match check.severity {
                            meta::Severity::Error => "ERR",
                            meta::Severity::Warning => "WRN",
                            meta::Severity::Info => "INF",
                        };
                        println!("  [{}] [{}] {}: {}", icon, sev, check.name, check.message);
                    }
                    println!();
                    if report.ready {
                        println!("  Environment: READY");
                    } else {
                        println!(
                            "  Environment: NOT READY ({} blocking issue(s))",
                            report.blocking_count
                        );
                    }
                    println!();
                }
                env_report = Some(report);
            }

            // Strategy analysis (pre-test)
            if !quality && !env_only || strategy_only {
                let report = meta::strategy::analyze(&target.benchmark, testbed_path);
                if !json {
                    println!("━━━ Testing Strategy ━━━");
                    println!();
                    println!("  Framework:   {}", report.framework);
                    println!("  F2P tests:   {}", report.f2p_count);
                    println!("  P2P tests:   {}", report.p2p_count);
                    println!(
                        "  Granularity: {} unit, {} integration, {} e2e, {} unknown",
                        report.granularity.unit,
                        report.granularity.integration,
                        report.granularity.end_to_end,
                        report.granularity.unknown
                    );
                    println!();
                    if !report.risk_factors.is_empty() {
                        println!("  Risk factors:");
                        for risk in &report.risk_factors {
                            let sev = match risk.severity {
                                meta::Severity::Error => "HIGH",
                                meta::Severity::Warning => "MED ",
                                meta::Severity::Info => "LOW ",
                            };
                            println!("    [{}] {}", sev, risk.description);
                        }
                        println!();
                    }
                    if !report.recommendations.is_empty() {
                        println!("  Recommendations:");
                        for rec in &report.recommendations {
                            println!("    - {}", rec);
                        }
                        println!();
                    }
                }
                strategy_report = Some(report);
            }

            // Quality analysis (post-test, requires existing verdict)
            if quality {
                let task_dir = testbed_path.join(format!(".{}", target.benchmark));
                let score_report_path = task_dir.join("score_report.json");
                if !score_report_path.exists() {
                    anyhow::bail!(
                        "No score report found at {}. Run scoring first, then use --quality.",
                        score_report_path.display()
                    );
                }
                let content = std::fs::read_to_string(&score_report_path)?;
                let verdict: synodic_eval::score::EvalVerdict =
                    serde_json::from_str(&content).map_err(|e| {
                        anyhow::anyhow!("Failed to parse score report: {}", e)
                    })?;
                let report = meta::quality::analyze(&verdict);
                if !json {
                    println!("━━━ Test Quality Analysis ━━━");
                    println!();
                    println!("  {}", report.summary);
                    println!();
                    if !report.issues.is_empty() {
                        println!("  Issues:");
                        for issue in &report.issues {
                            let kind = match issue.kind {
                                meta::QualityIssueKind::FalsePositive => "FALSE+",
                                meta::QualityIssueKind::FalseNegative => "FALSE-",
                                meta::QualityIssueKind::Flaky => "FLAKY ",
                                meta::QualityIssueKind::Anomalous => "ANOMLY",
                            };
                            println!(
                                "    [{}] {} (confidence: {:.0}%)",
                                kind,
                                issue.test_name,
                                issue.confidence * 100.0
                            );
                            println!("           {}", issue.evidence);
                        }
                        println!();
                    }
                }
                quality_report = Some(report);
            }

            // Combined report
            let meta_report =
                meta::MetaReport::compute(env_report, strategy_report, quality_report);

            if json {
                println!("{}", serde_json::to_string_pretty(&meta_report)?);
            } else {
                println!("━━━ Overall ━━━");
                println!();
                println!(
                    "  Confidence: {:.0}%",
                    meta_report.overall_confidence * 100.0
                );
                if !meta_report.actionable_findings.is_empty() {
                    println!();
                    println!("  Findings:");
                    for finding in &meta_report.actionable_findings {
                        println!("    - {}", finding);
                    }
                }
                println!();
            }

            Ok(())
        }
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
