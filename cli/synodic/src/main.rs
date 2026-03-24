mod cmd;
mod harness;
mod meta;
mod util;

use clap::Parser;

/// Synodic governance CLI
#[derive(Parser)]
#[command(name = "synodic", version, about)]
enum Cli {
    /// Governance wrapper for agent commands
    Harness(cmd::harness::HarnessCmd),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli {
        Cli::Harness(cmd) => cmd.run(),
    }
}
