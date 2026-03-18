use std::path::{Path, PathBuf};
use std::process::Command;

/// Walk up from CWD to find the repo root (contains `.harness/` or `.git`).
///
/// Respects `SYNODIC_ROOT` env var — set by `harness run` so that eval
/// subprocesses write governance logs to the correct project, not the testbed.
pub fn find_repo_root() -> anyhow::Result<PathBuf> {
    if let Ok(root) = std::env::var("SYNODIC_ROOT") {
        let p = PathBuf::from(&root);
        if p.is_dir() {
            return Ok(p);
        }
    }
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join(".harness").is_dir() || dir.join(".git").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            anyhow::bail!(
                "could not find repo root (no .harness/ or .git/ in any parent directory)"
            );
        }
    }
}

/// Spawn a script with the given args, inheriting stdio.
/// Exits the process with the script's exit code.
pub fn exec_script(script: &Path, args: &[String]) -> anyhow::Result<()> {
    let status = Command::new(script)
        .args(args)
        .status()
        .map_err(|e| anyhow::anyhow!("failed to execute {}: {}", script.display(), e))?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
