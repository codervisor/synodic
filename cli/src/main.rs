mod cmd;
mod util;

use clap::Parser;
use cmd::{eval::EvalCmd, harness::HarnessCmd};

/// Synodic governance CLI — harness + eval
#[derive(Parser)]
#[command(name = "synodic", version, about)]
enum Cli {
    /// Governance wrapper for agent commands
    Harness(HarnessCmd),
    /// Benchmark evaluation runner
    Eval(EvalCmd),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli {
        Cli::Harness(cmd) => cmd.run(),
        Cli::Eval(cmd) => cmd.run(),
    }
}
