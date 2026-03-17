use std::path::Path;

use anyhow::Result;
use serde_json::Value;

/// List available benchmark tasks from evals.json.
pub fn list_evals(repo_root: &Path, tag_filter: Option<&str>, json_output: bool) -> Result<()> {
    let evals_path = repo_root.join("evals/evals.json");
    if !evals_path.exists() {
        anyhow::bail!("evals/evals.json not found at {}", evals_path.display());
    }

    let content = std::fs::read_to_string(&evals_path)?;
    let registry: Value = serde_json::from_str(&content)?;

    let evals = registry
        .get("evals")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("invalid evals.json: missing 'evals' array"))?;

    let filtered: Vec<&Value> = evals
        .iter()
        .filter(|e| {
            if let Some(tag) = tag_filter {
                e.get("tags")
                    .and_then(|t| t.as_array())
                    .map(|tags| tags.iter().any(|t| t.as_str() == Some(tag)))
                    .unwrap_or(false)
            } else {
                true
            }
        })
        .collect();

    if json_output {
        println!("{}", serde_json::to_string_pretty(&filtered)?);
        return Ok(());
    }

    if filtered.is_empty() {
        println!("No evals found.");
        return Ok(());
    }

    println!(
        "{:<40} {:<18} {:<20}",
        "ID", "BENCHMARK", "ALIAS"
    );
    println!("{}", "─".repeat(78));

    for eval in &filtered {
        let id = eval.get("id").and_then(|v| v.as_str()).unwrap_or("?");
        let benchmark = eval
            .get("source")
            .and_then(|s| s.get("benchmark"))
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let alias = eval
            .get("e2e")
            .and_then(|e| e.get("alias"))
            .and_then(|v| v.as_str())
            .unwrap_or("-");

        println!("{:<40} {:<18} {:<20}", id, benchmark, alias);
    }

    println!("\n{} eval(s) found.", filtered.len());
    Ok(())
}
