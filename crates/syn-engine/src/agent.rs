use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::process::Command;

/// Output parsed from Claude Code's JSON response.
#[derive(Debug, Clone)]
pub struct AgentOutput {
    pub result_text: String,
    pub tokens_used: u64,
}

/// Spawns Claude Code as a subprocess with a given prompt and system prompt.
pub struct ClaudeAgent {
    pub model: String,
    pub system_prompt: String,
    pub working_dir: std::path::PathBuf,
    pub max_turns: Option<u32>,
}

/// The JSON schema Claude Code emits with `--output-format json`.
#[derive(Debug, Deserialize)]
struct ClaudeJsonOutput {
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    cost_usd: Option<f64>,
    #[serde(default)]
    #[allow(dead_code)]
    total_cost_usd: Option<f64>,
    #[serde(default)]
    usage: Option<ClaudeUsage>,
}

#[derive(Debug, Deserialize)]
struct ClaudeUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

impl ClaudeAgent {
    pub fn new(
        model: impl Into<String>,
        system_prompt: impl Into<String>,
        working_dir: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            model: model.into(),
            system_prompt: system_prompt.into(),
            working_dir: working_dir.into(),
            max_turns: None,
        }
    }

    pub fn with_max_turns(mut self, turns: u32) -> Self {
        self.max_turns = Some(turns);
        self
    }

    /// Spawn `claude` as a subprocess and parse the JSON output.
    pub async fn run(&self, prompt: &str) -> Result<AgentOutput> {
        let mut cmd = Command::new("claude");
        cmd.arg("-p")
            .arg("--output-format")
            .arg("json")
            .arg("--model")
            .arg(&self.model)
            .arg("--append-system-prompt")
            .arg(&self.system_prompt)
            .arg("--dangerously-skip-permissions")
            .arg(prompt)
            .current_dir(&self.working_dir);

        if let Some(turns) = self.max_turns {
            cmd.arg("--max-turns").arg(turns.to_string());
        }

        eprintln!(
            "[agent] spawning claude (model={}, dir={})",
            self.model,
            self.working_dir.display()
        );

        let output = cmd
            .output()
            .await
            .context("Failed to spawn claude process. Is `claude` CLI installed and on PATH?")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            anyhow::bail!(
                "claude exited with status {}: stderr={}, stdout={}",
                output.status,
                stderr.chars().take(500).collect::<String>(),
                stdout.chars().take(500).collect::<String>(),
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_claude_output(&stdout)
    }
}

/// Parse the JSON output from `claude -p --output-format json`.
fn parse_claude_output(raw: &str) -> Result<AgentOutput> {
    // Claude Code may emit multiple JSON objects or extra text.
    // Try to find and parse the main JSON object.
    let parsed: ClaudeJsonOutput = serde_json::from_str(raw.trim())
        .context("Failed to parse Claude Code JSON output")?;

    let result_text = parsed.result.unwrap_or_default();
    let tokens_used = parsed
        .usage
        .map(|u| u.input_tokens + u.output_tokens)
        .unwrap_or(0);

    Ok(AgentOutput {
        result_text,
        tokens_used,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_complete_json() {
        let raw = r#"{"result": "done", "usage": {"input_tokens": 100, "output_tokens": 50}}"#;
        let output = parse_claude_output(raw).unwrap();
        assert_eq!(output.result_text, "done");
        assert_eq!(output.tokens_used, 150);
    }

    #[test]
    fn test_parse_missing_result_field() {
        let raw = r#"{"usage": {"input_tokens": 10, "output_tokens": 5}}"#;
        let output = parse_claude_output(raw).unwrap();
        assert_eq!(output.result_text, "");
        assert_eq!(output.tokens_used, 15);
    }

    #[test]
    fn test_parse_missing_usage_field() {
        let raw = r#"{"result": "hello"}"#;
        let output = parse_claude_output(raw).unwrap();
        assert_eq!(output.result_text, "hello");
        assert_eq!(output.tokens_used, 0);
    }

    #[test]
    fn test_parse_empty_json_object() {
        let raw = r#"{}"#;
        let output = parse_claude_output(raw).unwrap();
        assert_eq!(output.result_text, "");
        assert_eq!(output.tokens_used, 0);
    }

    #[test]
    fn test_parse_invalid_json() {
        let raw = "not json at all";
        let result = parse_claude_output(raw);
        assert!(result.is_err());
    }
}

/// Run a git command in the given directory.
pub async fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .await
        .with_context(|| format!("Failed to run git {:?}", args))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {:?} failed: {}", args, stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
