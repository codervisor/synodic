mod cmd;
mod util;

use clap::Parser;

/// Synodic — AI agent governance via git hooks (L1) and Claude Code hooks (L2)
#[derive(Parser)]
#[command(
    name = "synodic",
    version,
    about = "The tool that watches the AI agents."
)]
enum Cli {
    /// Initialize governance: git hooksPath (L1) + Claude Code hooks (L2)
    Init(cmd::init::InitCmd),

    /// Evaluate agent tool call against interception rules (L2 intercept)
    Intercept(cmd::intercept::InterceptCmd),

    /// Record governance feedback (override, confirmed block, CI failure, incident)
    Feedback(cmd::feedback::FeedbackCmd),

    /// Manage governance rules
    Rules(cmd::rules::RulesCmd),

    /// Show governance status (safety, friction, coverage scores)
    Status(cmd::status::StatusCmd),

    /// Run adversarial probes against rules
    Probe(cmd::probe::ProbeCmd),

    /// Manage rule lifecycle transitions (promote, crystallize, deprecate)
    Lifecycle(cmd::lifecycle::LifecycleCmd),

    /// Scan feedback and propose rule optimizations
    Optimize(cmd::optimize::OptimizeCmd),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli {
        Cli::Init(cmd) => cmd.run(),
        Cli::Intercept(cmd) => cmd.run(),
        Cli::Feedback(cmd) => cmd.run().await,
        Cli::Rules(cmd) => cmd.run().await,
        Cli::Status(cmd) => cmd.run().await,
        Cli::Probe(cmd) => cmd.run().await,
        Cli::Lifecycle(cmd) => cmd.run().await,
        Cli::Optimize(cmd) => cmd.run().await,
    }
}
