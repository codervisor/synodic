use anyhow::Result;
use chrono::Utc;
use clap::Args;
use harness_core::{
    probing::{self, backtest_expansion, expand_pattern},
    storage::{self, ProbeResult, UpdateRule},
};
use uuid::Uuid;

/// Run adversarial probes against governance rules.
///
/// Tests whether rules can be bypassed via syntactic variation,
/// indirection, encoding, semantic equivalence, or path traversal.
#[derive(Args)]
pub struct ProbeCmd {
    /// Probe a specific rule (default: all active rules)
    #[arg(long)]
    rule: Option<String>,

    /// Use a specific strategy (default: all applicable)
    #[arg(long)]
    strategy: Option<String>,

    /// Automatically apply safe expansions (precision drop < 1%)
    #[arg(long)]
    auto_apply: bool,

    /// Database URL
    #[arg(long, env = "DATABASE_URL")]
    db_url: Option<String>,
}

impl ProbeCmd {
    pub async fn run(self) -> Result<()> {
        let db_url = self
            .db_url
            .unwrap_or_else(storage::pool::resolve_database_url);
        let store = storage::pool::create_storage(&db_url).await?;

        let rules = if let Some(ref rule_id) = self.rule {
            let rule = store
                .get_rule(rule_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("rule '{}' not found", rule_id))?;
            vec![rule]
        } else {
            store.get_rules(true).await?
        };

        let mut total_bypasses = 0;
        let mut proposals = Vec::new();

        for rule in &rules {
            println!("Probing rule '{}'...\n", rule.id);

            let reports = probing::run_all_probes(rule);

            for report in &reports {
                if let Some(ref filter) = self.strategy {
                    if report.strategy != *filter {
                        continue;
                    }
                }

                println!("  Strategy: {}", report.strategy);

                let mut strategy_bypasses = 0;
                for variant in &report.variants {
                    if variant.bypassed {
                        println!("    BYPASS: {}", variant.input);
                        strategy_bypasses += 1;

                        // Record probe result
                        store
                            .record_probe(ProbeResult {
                                id: Uuid::new_v4(),
                                rule_id: rule.id.clone(),
                                strategy: report.strategy.clone(),
                                probe_input: serde_json::json!({ "command": &variant.input }),
                                bypassed: true,
                                proposed_expansion: None,
                                expansion_precision_drop: None,
                                expansion_approved: None,
                                created_at: Utc::now(),
                            })
                            .await
                            .ok();

                        // Generate expansion proposal
                        let expanded =
                            expand_pattern(&rule.condition_value, &variant.input);
                        let backtest =
                            backtest_expansion(&expanded, &rule.condition_type);

                        proposals.push((
                            rule.id.clone(),
                            variant.input.clone(),
                            expanded,
                            backtest,
                        ));
                    } else {
                        println!("    caught: {}", variant.input);
                    }
                }
                total_bypasses += strategy_bypasses;
                println!();
            }
        }

        if total_bypasses == 0 {
            println!("No bypasses found. Rules are robust against tested strategies.");
            return Ok(());
        }

        println!(
            "\nFound {} bypass(es). Expansion proposals:\n",
            total_bypasses
        );

        for (i, (rule_id, bypass, pattern, backtest)) in proposals.iter().enumerate() {
            let safe_icon = if backtest.safe_to_apply { "ok" } else { "!!" };
            println!("  {}. Rule '{}' bypassed by: {}", i + 1, rule_id, bypass);
            println!("     Proposed: {}", pattern);
            println!(
                "     Backtest: [{}] {} safe commands blocked",
                safe_icon,
                backtest.safe_commands_blocked.len()
            );
            if !backtest.safe_commands_blocked.is_empty() {
                for cmd in &backtest.safe_commands_blocked {
                    println!("       - would block: {}", cmd);
                }
            }
            println!();
        }

        // Auto-apply safe expansions
        if self.auto_apply {
            let safe_proposals: Vec<_> = proposals
                .iter()
                .filter(|(_, _, _, bt)| bt.safe_to_apply)
                .collect();

            if safe_proposals.is_empty() {
                println!("No safe expansions to auto-apply (all would cause false positives).");
            } else {
                println!(
                    "Auto-applying {} safe expansion(s)...\n",
                    safe_proposals.len()
                );
                for (rule_id, bypass, pattern, _) in &safe_proposals {
                    store
                        .update_rule(
                            rule_id,
                            UpdateRule {
                                condition_value: Some(pattern.clone()),
                                ..Default::default()
                            },
                        )
                        .await?;
                    println!(
                        "  Applied expansion for '{}' (catches: {})",
                        rule_id, bypass
                    );
                }
            }
        }

        Ok(())
    }
}
