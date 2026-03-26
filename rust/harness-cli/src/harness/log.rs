use std::fs;
use std::path::Path;

use anyhow::Result;
use serde_json::Value;

/// Display governance log entries natively.
pub fn display(harness_dir: &Path, json_mode: bool, tail_n: usize) -> Result<()> {
    let mut all_lines: Vec<String> = Vec::new();

    if let Ok(entries) = fs::read_dir(harness_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".governance.jsonl") && entry.path().is_file() {
                let content = fs::read_to_string(entry.path()).unwrap_or_default();
                for line in content.lines() {
                    if !line.trim().is_empty() {
                        all_lines.push(line.to_string());
                    }
                }
            }
        }
    }

    if all_lines.is_empty() {
        println!("No governance logs found in {}", harness_dir.display());
        return Ok(());
    }

    if json_mode {
        for line in &all_lines {
            println!("{line}");
        }
        return Ok(());
    }

    println!("=== Governance Log (last {tail_n} entries) ===");

    let start = if all_lines.len() > tail_n {
        all_lines.len() - tail_n
    } else {
        0
    };

    for line in &all_lines[start..] {
        match serde_json::from_str::<Value>(line) {
            Ok(record) => {
                let wid = record
                    .get("work_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let status = record.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                let attempt = record
                    .get("metrics")
                    .and_then(|m| m.get("attempt_count"))
                    .and_then(|v| v.as_u64())
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "?".to_string());
                let ts = record
                    .get("timestamp")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let cats: Vec<String> = record
                    .get("rework_items")
                    .and_then(|v| v.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|i| i.get("category").and_then(|v| v.as_str()))
                            .map(|s| s.to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                let cat_str = if cats.is_empty() {
                    "(clean)".to_string()
                } else {
                    cats.join(", ")
                };
                println!("  {ts}  {wid:<30} {status:<10} attempts={attempt}  {cat_str}");
            }
            Err(_) => {
                println!("  {line}");
            }
        }
    }

    Ok(())
}
