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
        #[arg(last = true)]
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
            HarnessSubcommand::Eval { args } => {
                let repo_root = util::find_repo_root()?;
                let harness_dir = resolve_harness_dir(&repo_root)?;
                let script = harness_dir.join("scripts/evaluate_harness.py");
                if !script.exists() {
                    anyhow::bail!("evaluate_harness.py not found at {}", script.display());
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

                let spec_content = spec.and_then(|path| {
                    std::fs::read_to_string(&path).ok().or_else(|| {
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

                let run_id = format!("meta-{}", chrono::Utc::now().timestamp());
                let run_dir = harness_dir.join(".runs").join(&run_id);
                std::fs::create_dir_all(&run_dir)?;

                let result = crate::meta::run(&config, &run_dir)?;

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
                        eprintln!(
                            "meta: Tests completed but results are unreliable. Review findings."
                        );
                        std::process::exit(2)
                    }
                    _ => std::process::exit(1),
                }
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
