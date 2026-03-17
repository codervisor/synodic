use std::fs;
use std::path::Path;

use anyhow::Result;

/// List crystallized rules natively.
pub fn list(harness_dir: &Path) -> Result<()> {
    let rules_dir = harness_dir.join("rules");

    if !rules_dir.is_dir() {
        println!("No rules directory at {}", rules_dir.display());
        return Ok(());
    }

    println!("=== Crystallized Rules ===");
    let mut count = 0u32;

    if let Ok(entries) = fs::read_dir(&rules_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !entry.path().is_file() || name == ".gitkeep" {
                continue;
            }

            let executable = if is_executable(&entry.path()) {
                " (executable)"
            } else {
                ""
            };
            println!("  {name}{executable}");
            count += 1;
        }
    }

    if count == 0 {
        println!("  (none — rules directory is empty)");
        println!();
        println!("Rules are created through the crystallization process when");
        println!("governance logs show the same issue in ≥3 independent runs.");
    }

    println!();
    println!("Rules directory: {}", rules_dir.display());
    Ok(())
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    true
}
