use clap::{Args, Subcommand};

use crate::harness;
use crate::pipeline;
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

        /// Pipeline name (factory, fractal, swarm, adversarial)
        #[arg(long)]
        pipeline: Option<String>,

        /// Spec path for pipeline execution
        #[arg(long)]
        spec: Option<String>,

        /// Agent command (everything after --)
        #[arg(last = true)]
        agent_cmd: Vec<String>,
    },

    /// Validate a pipeline YAML before execution
    Validate {
        /// Pipeline name or path to YAML file
        pipeline: String,
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

    /// AI meta-testing: analyze project, propose testing strategy, implement and validate tests
    Meta {
        /// Working directory (defaults to repo root)
        #[arg(long)]
        workdir: Option<String>,

        /// Git diff to test (reads from stdin if omitted, or auto-detects from git)
        #[arg(long)]
        diff: Option<String>,

        /// Spec or feature description file path
        #[arg(long)]
        spec: Option<String>,

        /// AI agent command for consultation
        #[arg(long, default_value = "claude")]
        agent: String,

        /// Max rework cycles for test validation
        #[arg(long, default_value = "2")]
        max_rework: u32,

        /// Minimal output
        #[arg(long, short)]
        quiet: bool,

        /// Machine-readable JSON output
        #[arg(long)]
        json: bool,

        /// Show plan without executing tests
        #[arg(long)]
        dry_run: bool,
    },
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
                pipeline: pipeline_name,
                spec,
                agent_cmd,
            } => {
                // Pipeline mode: synodic harness run --pipeline <name>
                if let Some(name) = pipeline_name {
                    let repo_root = util::find_repo_root()?;
                    let harness_dir = resolve_harness_dir(&repo_root)?;
                    let pipeline_path = resolve_pipeline_path(&name, &harness_dir)?;
                    let config = pipeline::executor::ExecConfig {
                        pipeline_path,
                        spec_path: spec,
                        repo_root: repo_root.clone(),
                        harness_dir,
                        dry_run,
                        json_output: json,
                    };
                    let result = pipeline::executor::execute(&config)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&result)?);
                    } else {
                        eprintln!("Pipeline '{}': {}", result.pipeline, result.status);
                        for step in &result.steps {
                            let status_str = match &step.status {
                                pipeline::executor::StepStatus::Passed => "PASS",
                                pipeline::executor::StepStatus::Failed => "FAIL",
                                pipeline::executor::StepStatus::Skipped => "SKIP",
                            };
                            eprintln!("  {} {} ({}ms)", status_str, step.name, step.duration_ms);
                        }
                    }
                    // Write governance log.
                    let gov_path = repo_root.join(".harness").join(format!("{}.governance.jsonl", name));
                    let entry = serde_json::json!({
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                        "pipeline": result.pipeline,
                        "status": result.status,
                        "duration_ms": result.duration_ms,
                        "steps": result.steps.len(),
                    });
                    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&gov_path) {
                        use std::io::Write;
                        let _ = writeln!(f, "{}", serde_json::to_string(&entry)?);
                    }
                    return Ok(());
                }

                // Legacy mode: synodic harness run -- <agent_cmd>
                if agent_cmd.is_empty() {
                    anyhow::bail!("either --pipeline <name> or agent command (after --) is required");
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
            HarnessSubcommand::Validate { pipeline: name } => {
                let repo_root = util::find_repo_root()?;
                let harness_dir = resolve_harness_dir(&repo_root)?;
                let pipeline_path = resolve_pipeline_path(&name, &harness_dir)?;
                let p = pipeline::Pipeline::from_file(&pipeline_path)?;
                let errors = p.validate();
                if errors.is_empty() {
                    println!("Pipeline '{}' is valid ({} steps)", p.name, p.steps.len());
                    for step in &p.steps {
                        let kind = match &step.kind {
                            pipeline::StepKind::Agent(_) => "agent",
                            pipeline::StepKind::Run(_) => "run",
                            pipeline::StepKind::Branch(_) => "branch",
                            pipeline::StepKind::Fan(_) => "fan",
                        };
                        println!("  {} ({})", step.name, kind);
                    }
                    Ok(())
                } else {
                    eprintln!("Pipeline '{}' has {} error(s):", p.name, errors.len());
                    for err in &errors {
                        eprintln!("  - {}", err);
                    }
                    std::process::exit(1);
                }
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
            HarnessSubcommand::Meta {
                workdir,
                diff,
                spec,
                agent,
                max_rework,
                quiet,
                json,
                dry_run,
            } => {
                let repo_root = util::find_repo_root()?;
                let harness_dir = resolve_harness_dir(&repo_root)?;
                let work = workdir
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| repo_root.clone());

                // Read spec from file if path provided
                let spec_content = spec.and_then(|path| {
                    std::fs::read_to_string(&path)
                        .ok()
                        .or_else(|| {
                            eprintln!("Warning: could not read spec file: {path}");
                            None
                        })
                });

                let config = crate::meta::MetaConfig {
                    workdir: work,
                    diff,
                    spec: spec_content,
                    agent_cmd: agent,
                    max_rework,
                    quiet,
                    json_output: json,
                    dry_run,
                };

                let run_id = format!(
                    "meta-{}",
                    chrono::Utc::now().timestamp()
                );
                let run_dir = harness_dir.join(".runs").join(&run_id);
                std::fs::create_dir_all(&run_dir)?;

                let result = crate::meta::run(&config, &run_dir)?;

                // Write governance log
                let gov_log = harness_dir.join("meta.governance.jsonl");
                let entry = serde_json::json!({
                    "work_id": run_id,
                    "source": "meta-testing",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "status": result.status,
                    "strategy": result.plan.strategy,
                    "frameworks": result.plan.frameworks,
                    "tests_proposed": result.plan.tiers.iter().map(|t| t.tests.len()).sum::<usize>(),
                    "tests_passed": result.execution.as_ref().map(|e| e.total_passed),
                    "tests_failed": result.execution.as_ref().map(|e| e.total_failed),
                    "confidence": result.validation.as_ref().map(|v| v.confidence),
                    "run_dir": run_dir.display().to_string(),
                });
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&gov_log)
                {
                    use std::io::Write;
                    let _ = writeln!(f, "{}", serde_json::to_string(&entry)?);
                }

                if json {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else if !quiet {
                    eprintln!();
                    eprintln!("meta: ━━━ Result: {} ━━━", result.status);
                    eprintln!("meta: Run dir: {}", run_dir.display());
                    eprintln!("meta: Governance log: {}", gov_log.display());
                }

                match result.status.as_str() {
                    "passed" => Ok(()),
                    "unreliable" => {
                        eprintln!("meta: Tests completed but results are unreliable. Review findings.");
                        std::process::exit(2)
                    }
                    _ => std::process::exit(1),
                }
            }
        }
    }
}

/// Resolve a pipeline name or path to a YAML file path.
fn resolve_pipeline_path(
    name: &str,
    harness_dir: &std::path::Path,
) -> anyhow::Result<std::path::PathBuf> {
    // If it's already a file path, use directly.
    let as_path = std::path::PathBuf::from(name);
    if as_path.exists() {
        return Ok(as_path);
    }
    // Look in .harness/pipelines/<name>.yml
    let pipeline_path = harness_dir.join("pipelines").join(format!("{}.yml", name));
    if pipeline_path.exists() {
        return Ok(pipeline_path);
    }
    anyhow::bail!(
        "pipeline '{}' not found (tried: {}, {})",
        name,
        as_path.display(),
        pipeline_path.display()
    )
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
