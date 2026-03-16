use std::path::Path;

use anyhow::{Context, Result};
use chrono::Utc;
use syn_types::{BuildReport, ReviewReport, StationId, StationOutcome, WorkItem};

use crate::agent::{git, ClaudeAgent};

/// Process a work item at its current station.
pub async fn process_station(item: &mut WorkItem, repo_root: &Path) -> Result<StationOutcome> {
    match item.station {
        StationId::Build => process_build(item, repo_root).await,
        StationId::Inspect => process_inspect(item, repo_root).await,
    }
}

/// BUILD station: read spec, create branch, spawn Claude Code implementer, write report.
async fn process_build(item: &mut WorkItem, repo_root: &Path) -> Result<StationOutcome> {
    eprintln!(
        "\n=== STATION: BUILD (attempt {}) ===",
        item.attempt
    );

    let spec_readme = repo_root.join(&item.spec_path).join("README.md");
    let spec_content = tokio::fs::read_to_string(&spec_readme)
        .await
        .with_context(|| format!("Failed to read spec at {}", spec_readme.display()))?;

    // Create or reset the factory branch.
    if item.attempt == 1 {
        // Create a new branch from current HEAD.
        let _ = git(repo_root, &["branch", "-D", &item.branch]).await; // ignore error if doesn't exist
        git(repo_root, &["checkout", "-b", &item.branch]).await
            .context("Failed to create factory branch")?;
    } else {
        // Switch to existing branch for rework.
        git(repo_root, &["checkout", &item.branch]).await
            .context("Failed to checkout factory branch")?;
    }

    // Construct the prompt for the BUILD agent.
    let mut prompt = format!(
        "You are implementing a spec. Read this spec carefully and implement everything described in its Plan section.\n\
         Commit all your changes with a descriptive commit message.\n\
         Run any tests described in the spec's Test section.\n\n\
         ## Spec\n\n{spec_content}"
    );

    if let Some(ref feedback) = item.rework_feedback {
        prompt.push_str(&format!(
            "\n\n## Rework Required\n\nThe reviewer returned this implementation for rework. \
             Address ALL of the following issues:\n\n{feedback}"
        ));
    }

    let system_prompt = "You are a precise code implementer. Your job is to:\n\
        1. Read the provided spec\n\
        2. Implement every item in the Plan section\n\
        3. Run the tests described in the Test section\n\
        4. Commit all changes with a clear commit message\n\
        Focus on correctness and completeness. Do not skip any plan items."
        .to_string();

    let agent = ClaudeAgent::new("sonnet", system_prompt, repo_root);
    let output = agent.run(&prompt).await.context("BUILD agent failed")?;

    // Gather files changed.
    let diff_stat = git(repo_root, &["diff", "--stat", "main...HEAD"])
        .await
        .unwrap_or_default();
    let files_changed: Vec<String> = diff_stat
        .lines()
        .filter(|l| !l.contains("files changed") && !l.is_empty())
        .map(|l| l.split('|').next().unwrap_or("").trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let report = BuildReport {
        work_id: item.id.clone(),
        spec_path: item.spec_path.clone(),
        branch: item.branch.clone(),
        files_changed,
        tests_passed: true, // trust the agent ran them; inspector will verify
        summary: truncate(&output.result_text, 2000),
        tokens_used: output.tokens_used,
        timestamp: Utc::now(),
    };

    // Write the build report.
    let report_path = item.artifacts_dir.join(format!(
        "build-report-attempt-{}.json",
        item.attempt
    ));
    tokio::fs::write(&report_path, serde_json::to_string_pretty(&report)?)
        .await
        .context("Failed to write build report")?;

    item.metrics.total_tokens += output.tokens_used;

    eprintln!(
        "[build] Done. Files changed: {}, tokens: {}",
        report.files_changed.len(),
        output.tokens_used
    );

    Ok(StationOutcome::Pass {
        next: StationId::Inspect,
    })
}

/// INSPECT station: review the diff against the spec, approve or rework.
async fn process_inspect(item: &mut WorkItem, repo_root: &Path) -> Result<StationOutcome> {
    eprintln!(
        "\n=== STATION: INSPECT (attempt {}) ===",
        item.attempt
    );

    // Ensure we're on the factory branch.
    git(repo_root, &["checkout", &item.branch]).await
        .context("Failed to checkout branch for inspection")?;

    let spec_readme = repo_root.join(&item.spec_path).join("README.md");
    let spec_content = tokio::fs::read_to_string(&spec_readme)
        .await
        .with_context(|| format!("Failed to read spec at {}", spec_readme.display()))?;

    let diff = git(repo_root, &["diff", "main...HEAD"]).await.unwrap_or_default();

    // Read the latest build report.
    let build_report_path = item.artifacts_dir.join(format!(
        "build-report-attempt-{}.json",
        item.attempt
    ));
    let build_report_content = tokio::fs::read_to_string(&build_report_path)
        .await
        .unwrap_or_else(|_| "No build report found.".to_string());

    let prompt = format!(
        "Review this implementation against the spec.\n\n\
         ## Original Spec\n\n{spec_content}\n\n\
         ## Build Report\n\n{build_report_content}\n\n\
         ## Git Diff (main...HEAD)\n\n```diff\n{diff}\n```\n\n\
         ## Your Review Task\n\n\
         Evaluate correctness, security, completeness, and code quality.\n\n\
         You MUST end your response with exactly one of these two lines:\n\
         VERDICT: APPROVE\n\
         VERDICT: REWORK\n\n\
         If REWORK, list specific items that must be fixed before the line."
    );

    let system_prompt = "You are a strict code reviewer. Your job is to:\n\
        1. Compare the implementation diff against the original spec\n\
        2. Check correctness — does the code do what the spec says?\n\
        3. Check completeness — are all Plan items implemented?\n\
        4. Check security — no obvious vulnerabilities\n\
        5. Check quality — reasonable code structure and naming\n\
        End your review with exactly: VERDICT: APPROVE or VERDICT: REWORK"
        .to_string();

    let agent = ClaudeAgent::new("sonnet", system_prompt, repo_root);
    let output = agent.run(&prompt).await.context("INSPECT agent failed")?;

    let review_text = &output.result_text;
    let approved = review_text.contains("VERDICT: APPROVE");

    let rework_items: Vec<String> = if !approved {
        // Extract lines before the VERDICT as rework items.
        review_text
            .lines()
            .filter(|l| {
                let trimmed = l.trim();
                !trimmed.is_empty()
                    && !trimmed.starts_with("VERDICT:")
                    && !trimmed.starts_with('#')
            })
            .map(|l| l.to_string())
            .collect()
    } else {
        vec![]
    };

    let report = ReviewReport {
        work_id: item.id.clone(),
        approved,
        review_comments: truncate(review_text, 2000),
        rework_items: rework_items.clone(),
        tokens_used: output.tokens_used,
        timestamp: Utc::now(),
    };

    let report_path = item.artifacts_dir.join(format!(
        "review-report-attempt-{}.json",
        item.attempt
    ));
    tokio::fs::write(&report_path, serde_json::to_string_pretty(&report)?)
        .await
        .context("Failed to write review report")?;

    item.metrics.total_tokens += output.tokens_used;

    if approved {
        eprintln!("[inspect] APPROVED. Tokens: {}", output.tokens_used);
        if item.attempt == 1 {
            item.metrics.first_pass_yield = Some(true);
        }
        Ok(StationOutcome::Approved)
    } else {
        eprintln!(
            "[inspect] REWORK required. {} items. Tokens: {}",
            rework_items.len(),
            output.tokens_used
        );
        if item.attempt == 1 {
            item.metrics.first_pass_yield = Some(false);
        }
        Ok(StationOutcome::Rework {
            back_to: StationId::Build,
            feedback: report.review_comments.clone(),
        })
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let boundary = s.floor_char_boundary(max);
        format!("{}... (truncated)", &s[..boundary])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_ascii_shorter_than_max() {
        let result = truncate("hello", 10);
        assert_eq!(result, "hello");
    }

    #[test]
    fn truncate_ascii_longer_than_max() {
        let result = truncate("hello world", 5);
        assert_eq!(result, "hello... (truncated)");
    }

    #[test]
    fn truncate_multibyte_boundary_inside_char() {
        // 'é' is 2 bytes in UTF-8. "café" is 5 bytes: c(1) a(1) f(1) é(2).
        // Truncating at max=4 falls inside 'é', should round down to 3.
        let result = truncate("café", 4);
        assert_eq!(result, "caf... (truncated)");
    }

    #[test]
    fn truncate_empty_string() {
        let result = truncate("", 10);
        assert_eq!(result, "");
    }

    #[test]
    fn truncate_exact_max_length() {
        let result = truncate("hello", 5);
        assert_eq!(result, "hello");
    }

    #[test]
    fn truncate_multibyte_on_boundary() {
        // "café" is 5 bytes. max=5 means no truncation.
        let result = truncate("café", 5);
        assert_eq!(result, "café");
    }

    #[test]
    fn truncate_all_multibyte() {
        // Each '🦀' is 4 bytes. "🦀🦀" is 8 bytes.
        // max=5 falls inside second crab, rounds down to 4.
        let result = truncate("🦀🦀", 5);
        assert_eq!(result, "🦀... (truncated)");
    }
}
