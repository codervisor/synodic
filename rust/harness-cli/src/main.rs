mod cmd;
mod harness;
mod meta;
mod util;

use clap::Parser;

/// Synodic — AI agent event governance platform
#[derive(Parser)]
#[command(
    name = "synodic",
    version,
    about = "The tool that watches the AI agents."
)]
enum Cli {
    /// Initialize .harness/ directory and SQLite database
    Init(cmd::init::InitCmd),

    /// Submit a governance event
    Submit(cmd::submit::SubmitCmd),

    /// Collect events from AI agent session logs
    Collect(cmd::collect::CollectCmd),

    /// List governance events
    List(cmd::list::ListCmd),

    /// Search events by text
    Search(cmd::search::SearchCmd),

    /// Show aggregate event statistics
    Stats(cmd::stats::StatsCmd),

    /// Resolve a governance event
    Resolve(cmd::resolve::ResolveCmd),

    /// Manage detection rules
    Rules(cmd::rules::RulesCmd),

    /// Live event monitoring (TUI)
    Watch(cmd::watch::WatchCmd),

    /// Start API server and dashboard
    Serve(cmd::serve::ServeCmd),

    /// Legacy governance harness (L1/L2 evaluation loop)
    Harness(cmd::harness_legacy::HarnessCmd),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli {
        Cli::Init(cmd) => cmd.run(),
        Cli::Submit(cmd) => cmd.run(),
        Cli::Collect(cmd) => cmd.run(),
        Cli::List(cmd) => cmd.run(),
        Cli::Search(cmd) => cmd.run(),
        Cli::Stats(cmd) => cmd.run(),
        Cli::Resolve(cmd) => cmd.run(),
        Cli::Rules(cmd) => cmd.run(),
        Cli::Watch(cmd) => cmd.run(),
        Cli::Serve(cmd) => cmd.run(),
        Cli::Harness(cmd) => cmd.run(),
    }
}
