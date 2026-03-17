mod cmd;
mod eval;
mod harness;
mod util;

use clap::Parser;

/// Synodic governance CLI — harness + eval
#[derive(Parser)]
#[command(name = "synodic", version, about)]
enum Cli {
    /// Governance wrapper for agent commands
    Harness(cmd::harness::HarnessCmd),
    /// Benchmark evaluation runner
    Eval(cmd::eval::EvalCmd),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli {
        Cli::Harness(cmd) => cmd.run(),
        Cli::Eval(cmd) => cmd.run(),
    }
}
