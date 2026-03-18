use std::path::{Path, PathBuf};
use std::process::Command;

/// Walk up from CWD to find the project root (contains `evals/` or `.git`).
///
/// Respects `EVAL_ROOT` env var for explicit override.
pub fn find_project_root() -> anyhow::Result<PathBuf> {
    if let Ok(root) = std::env::var("EVAL_ROOT") {
        let p = PathBuf::from(&root);
        if p.is_dir() {
            return Ok(p);
        }
    }
    let mut dir = std::env::current_dir()?;
    loop {
        if dir.join("evals").is_dir() || dir.join(".git").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            anyhow::bail!(
                "could not find project root (no evals/ or .git/ in any parent directory)"
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
