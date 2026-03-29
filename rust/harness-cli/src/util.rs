use std::path::PathBuf;

/// Walk up from CWD to find the repo root (contains `.harness/` or `.git`).
///
/// Respects `SYNODIC_ROOT` env var — set by `harness run` so that agent
/// subprocesses can find the correct project root.
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
