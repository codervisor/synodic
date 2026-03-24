use anyhow::Result;
use chrono::{Duration, Utc};
use clap::Args;

use harness_core::parsers::claude::{self, ClaudeLogParser};
use harness_core::parsers::copilot::{self, CopilotLogParser};
use harness_core::parsers::LogParser;
use harness_core::storage::sqlite::SqliteStore;
use harness_core::storage::EventStore;

use crate::util;

#[derive(Args)]
pub struct CollectCmd {
    /// Log source: claude, copilot, auto (default: auto)
    #[arg(long, default_value = "auto")]
    source: String,

    /// Only collect events newer than this duration (e.g., "1h", "30m", "7d")
    #[arg(long)]
    since: Option<String>,

    /// Show what would be collected without inserting
    #[arg(long)]
    dry_run: bool,
}

/// Parse a duration string like "1h", "30m", "7d" into a chrono::Duration.
fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();
    if s.is_empty() {
        anyhow::bail!("empty duration string");
    }
    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: i64 = num_str.parse().map_err(|_| anyhow::anyhow!("invalid duration: {s}"))?;
    match unit {
        "s" => Ok(Duration::seconds(num)),
        "m" => Ok(Duration::minutes(num)),
        "h" => Ok(Duration::hours(num)),
        "d" => Ok(Duration::days(num)),
        _ => anyhow::bail!("unknown duration unit '{unit}' in '{s}'. Use s/m/h/d."),
    }
}

impl CollectCmd {
    pub fn run(self) -> Result<()> {
        let root = util::find_repo_root()?;
        let db_path = root.join(".harness").join("synodic.db");

        let cutoff = self
            .since
            .as_deref()
            .map(|s| parse_duration(s).map(|d| Utc::now() - d))
            .transpose()?;

        let store = if self.dry_run {
            None
        } else {
            if !db_path.exists() {
                anyhow::bail!(
                    "Database not found at {}. Run `synodic init` first.",
                    db_path.display()
                );
            }
            Some(SqliteStore::open(&db_path)?)
        };

        let mut total = 0;

        // Claude Code logs
        if self.source == "auto" || self.source == "claude" {
            let logs = claude::find_session_logs(&root)?;
            if logs.is_empty() {
                eprintln!("No Claude Code session logs found in {}", root.display());
            } else {
                let parser = ClaudeLogParser::new();
                for log_path in &logs {
                    // Skip log files older than cutoff based on file modification time
                    if let Some(cutoff_ts) = cutoff {
                        if let Ok(meta) = std::fs::metadata(log_path) {
                            if let Ok(modified) = meta.modified() {
                                let mod_time: chrono::DateTime<Utc> = modified.into();
                                if mod_time < cutoff_ts {
                                    continue;
                                }
                            }
                        }
                    }

                    let events = parser.parse(log_path)?;
                    // Filter events by timestamp
                    let events: Vec<_> = if let Some(cutoff_ts) = cutoff {
                        events.into_iter().filter(|e| e.created_at >= cutoff_ts).collect()
                    } else {
                        events
                    };

                    if events.is_empty() {
                        continue;
                    }
                    eprintln!(
                        "{}: {} event(s)",
                        log_path.file_name().unwrap_or_default().to_string_lossy(),
                        events.len()
                    );
                    for event in &events {
                        eprintln!(
                            "  [{:?}] {} ({})",
                            event.severity, event.title, event.event_type
                        );
                        if let Some(ref store) = store {
                            store.insert(event)?;
                        }
                    }
                    total += events.len();
                }
            }
        }

        // Copilot logs
        if self.source == "auto" || self.source == "copilot" {
            let logs = copilot::find_copilot_logs(&root)?;
            if logs.is_empty() {
                if self.source == "copilot" {
                    eprintln!("No Copilot event logs found");
                }
            } else {
                let parser = CopilotLogParser::new();
                for log_path in &logs {
                    // Skip log files older than cutoff
                    if let Some(cutoff_ts) = cutoff {
                        if let Ok(meta) = std::fs::metadata(log_path) {
                            if let Ok(modified) = meta.modified() {
                                let mod_time: chrono::DateTime<Utc> = modified.into();
                                if mod_time < cutoff_ts {
                                    continue;
                                }
                            }
                        }
                    }

                    let events = parser.parse(log_path)?;
                    let events: Vec<_> = if let Some(cutoff_ts) = cutoff {
                        events.into_iter().filter(|e| e.created_at >= cutoff_ts).collect()
                    } else {
                        events
                    };

                    if events.is_empty() {
                        continue;
                    }
                    eprintln!(
                        "{}: {} event(s)",
                        log_path.file_name().unwrap_or_default().to_string_lossy(),
                        events.len()
                    );
                    for event in &events {
                        eprintln!(
                            "  [{:?}] {} ({})",
                            event.severity, event.title, event.event_type
                        );
                        if let Some(ref store) = store {
                            store.insert(event)?;
                        }
                    }
                    total += events.len();
                }
            }
        }

        if self.dry_run {
            eprintln!("\nDry run: would collect {} event(s)", total);
        } else {
            eprintln!("\nCollected {} event(s)", total);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_seconds() {
        assert_eq!(parse_duration("30s").unwrap(), Duration::seconds(30));
    }

    #[test]
    fn test_parse_duration_minutes() {
        assert_eq!(parse_duration("5m").unwrap(), Duration::minutes(5));
    }

    #[test]
    fn test_parse_duration_hours() {
        assert_eq!(parse_duration("2h").unwrap(), Duration::hours(2));
    }

    #[test]
    fn test_parse_duration_days() {
        assert_eq!(parse_duration("7d").unwrap(), Duration::days(7));
    }

    #[test]
    fn test_parse_duration_invalid() {
        assert!(parse_duration("abc").is_err());
        assert!(parse_duration("").is_err());
        assert!(parse_duration("5x").is_err());
    }
}
