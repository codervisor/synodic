use anyhow::Result;
use clap::{Args, Subcommand};

use harness_core::rules;

#[derive(Args)]
pub struct RulesCmd {
    #[command(subcommand)]
    subcmd: RulesSubCmd,
}

#[derive(Subcommand)]
enum RulesSubCmd {
    /// List all detection rules
    List {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Test a rule against a log file
    Test {
        /// Rule name to test
        rule: String,
        /// Log file to test against
        #[arg(long)]
        against: String,
    },
}

impl RulesCmd {
    pub fn run(self) -> Result<()> {
        match self.subcmd {
            RulesSubCmd::List { json } => {
                let rules = rules::default_rules();
                if json {
                    println!("{}", serde_json::to_string_pretty(&rules)?);
                } else {
                    println!(
                        "{:<25} {:<22} {:<10} {:<8} DESCRIPTION",
                        "NAME", "EVENT_TYPE", "SEVERITY", "ENABLED"
                    );
                    println!("{}", "-".repeat(90));
                    for r in &rules {
                        println!(
                            "{:<25} {:<22} {:<10} {:<8} {}",
                            r.name, r.event_type, r.severity, r.enabled, r.description
                        );
                    }
                }
                Ok(())
            }
            RulesSubCmd::Test { rule, against } => {
                let all_rules = rules::default_rules();
                let target = all_rules
                    .into_iter()
                    .find(|r| r.name == rule)
                    .ok_or_else(|| anyhow::anyhow!("rule not found: {rule}"))?;

                let content = std::fs::read_to_string(&against)?;
                let mut engine = rules::RuleEngine::new(vec![target]);
                let matches = engine.evaluate(&content);

                if matches.is_empty() {
                    eprintln!("No matches for rule '{rule}' in {against}");
                } else {
                    for m in &matches {
                        println!("[{}] {}", m.severity, m.matched_text);
                    }
                    eprintln!("\n{} match(es)", matches.len());
                }
                Ok(())
            }
        }
    }
}
