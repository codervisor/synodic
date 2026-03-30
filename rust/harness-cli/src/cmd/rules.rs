use anyhow::Result;
use clap::{Args, Subcommand};
use harness_core::storage;

/// Manage governance rules.
#[derive(Args)]
pub struct RulesCmd {
    #[command(subcommand)]
    action: RulesAction,

    /// Database URL (defaults to ~/.synodic/synodic.db)
    #[arg(long, env = "DATABASE_URL", global = true)]
    db_url: Option<String>,
}

#[derive(Subcommand)]
enum RulesAction {
    /// List all rules
    List {
        /// Show all rules, including disabled/deprecated
        #[arg(long)]
        all: bool,
    },
    /// Show a single rule's details
    Show {
        /// Rule ID
        id: String,
    },
}

impl RulesCmd {
    pub async fn run(self) -> Result<()> {
        let db_url = self
            .db_url
            .unwrap_or_else(storage::pool::resolve_database_url);
        let store = storage::pool::create_storage(&db_url).await?;

        match self.action {
            RulesAction::List { all } => list_rules(&*store, !all).await,
            RulesAction::Show { id } => show_rule(&*store, &id).await,
        }
    }
}

async fn list_rules(store: &dyn storage::Storage, active_only: bool) -> Result<()> {
    let rules = store.get_rules(active_only).await?;

    if rules.is_empty() {
        println!("No rules found.");
        return Ok(());
    }

    println!(
        "{:<25} {:<12} {:<20} {:<10} {:>5} {:>5} {:>7}",
        "ID", "LIFECYCLE", "CATEGORY", "ENABLED", "ALPHA", "BETA", "PREC %"
    );
    println!("{}", "-".repeat(90));

    for rule in &rules {
        let precision = if rule.alpha + rule.beta > 0 {
            (rule.alpha as f64 / (rule.alpha + rule.beta) as f64) * 100.0
        } else {
            0.0
        };

        println!(
            "{:<25} {:<12} {:<20} {:<10} {:>5} {:>5} {:>6.1}",
            rule.id,
            rule.lifecycle.as_str(),
            rule.category_id,
            if rule.enabled { "yes" } else { "no" },
            rule.alpha,
            rule.beta,
            precision,
        );
    }

    println!("\n{} rule(s)", rules.len());
    Ok(())
}

async fn show_rule(store: &dyn storage::Storage, id: &str) -> Result<()> {
    let rule = store
        .get_rule(id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("rule '{}' not found", id))?;

    let precision = if rule.alpha + rule.beta > 0 {
        (rule.alpha as f64 / (rule.alpha + rule.beta) as f64) * 100.0
    } else {
        0.0
    };

    println!("Rule: {}", rule.id);
    println!("  Description:  {}", rule.description);
    println!("  Category:     {}", rule.category_id);
    println!("  Lifecycle:    {}", rule.lifecycle);
    println!("  Enabled:      {}", rule.enabled);
    println!("  Tools:        {}", if rule.tools.is_empty() { "all".to_string() } else { rule.tools.join(", ") });
    println!("  Condition:    {} = {}", rule.condition_type, rule.condition_value);
    println!("  Alpha (TP):   {}", rule.alpha);
    println!("  Beta (FP):    {}", rule.beta);
    println!("  Precision:    {:.1}%", precision);
    println!("  Created:      {}", rule.created_at);
    println!("  Updated:      {}", rule.updated_at);

    if let Some(ts) = rule.crystallized_at {
        println!("  Crystallized: {}", ts);
    }

    // Show recent feedback
    let feedback = store
        .get_feedback(storage::FeedbackFilters {
            rule_id: Some(id.to_string()),
            limit: Some(5),
            ..Default::default()
        })
        .await?;

    if !feedback.is_empty() {
        println!("\n  Recent feedback:");
        for event in &feedback {
            let reason = event
                .override_reason
                .as_deref()
                .map(|r| format!(" — {r}"))
                .unwrap_or_default();
            println!(
                "    {} {} {}{}",
                event.created_at.format("%Y-%m-%d %H:%M"),
                event.signal_type,
                event.tool_name,
                reason
            );
        }
    }

    Ok(())
}
