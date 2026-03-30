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
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli {
        Cli::Init(cmd) => cmd.run(),
        Cli::Intercept(cmd) => cmd.run(),
    }
}
