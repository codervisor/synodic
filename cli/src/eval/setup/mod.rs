pub mod devbench;
pub mod featurebench;
pub mod swebench;
pub mod synodic;

use std::path::Path;

use anyhow::{bail, Result};

/// Dispatch setup to the correct benchmark handler.
///
/// This replaces the case statement in run.sh Phase 1.
pub fn run_setup(
    benchmark: &str,
    instance_id: &str,
    testbed_dir: &str,
    skill: &str,
    split: &str,
    repo_root: &Path,
) -> Result<()> {
    match benchmark {
        "featurebench" => featurebench::setup(instance_id, testbed_dir, skill, repo_root),
        "swebench" => swebench::setup(instance_id, testbed_dir, split, skill, repo_root),
        "devbench" => devbench::setup(instance_id, testbed_dir, skill, repo_root),
        "synodic" => synodic::setup(instance_id, testbed_dir, skill, repo_root),
        other => bail!("Unknown benchmark: {}", other),
    }
}
