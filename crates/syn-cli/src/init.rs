use std::path::Path;

use anyhow::{Context, Result};
use tokio::fs;

const HARNESS_MD_TEMPLATE: &str = include_str!("../templates/HARNESS.md");
const STATIC_GATE_SH: &str = include_str!("../templates/static_gate.sh");
const DECOMPOSE_GATE_PY: &str = include_str!("../templates/decompose_gate.py");
const AGGREGATE_GOVERNANCE_PY: &str = include_str!("../templates/aggregate_governance.py");
const HARNESS_README: &str = include_str!("../templates/harness-readme.md");

pub async fn cmd_init(
    target: &Path,
    topology: &str,
    rules_dir: &str,
    force: bool,
) -> Result<()> {
    let target = target.canonicalize().unwrap_or_else(|_| target.to_path_buf());

    eprintln!("Initializing Harness governance in {}", target.display());

    // Step 1 — Create directory structure
    let harness_dir = target.join(".harness");
    let rules_path = target.join(rules_dir);
    let templates_dir = harness_dir.join("templates");
    let scripts_dir = harness_dir.join("scripts");

    fs::create_dir_all(&rules_path).await.context("Failed to create rules directory")?;
    fs::create_dir_all(&templates_dir).await.context("Failed to create templates directory")?;
    fs::create_dir_all(&scripts_dir).await.context("Failed to create scripts directory")?;

    // .gitkeep files
    write_if_missing(&rules_path.join(".gitkeep"), "", force).await?;
    write_if_missing(&templates_dir.join(".gitkeep"), "", force).await?;

    // .harness/README.md
    write_if_missing(&harness_dir.join("README.md"), HARNESS_README, force).await?;

    // Step 2 — Copy HARNESS.md template
    let harness_md = target.join("HARNESS.md");
    if harness_md.exists() && !force {
        eprintln!("  HARNESS.md already exists. Skipping. Use --force to overwrite.");
    } else {
        fs::write(&harness_md, HARNESS_MD_TEMPLATE).await?;
        eprintln!("  Created HARNESS.md");
    }

    // Step 3 — Create governance log files
    let topologies: Vec<&str> = topology.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    for topo in &topologies {
        let log_file = harness_dir.join(format!("{topo}.governance.jsonl"));
        if !log_file.exists() {
            fs::write(&log_file, "").await?;
            eprintln!("  Created .harness/{topo}.governance.jsonl");
        }
    }

    // Step 4 — Create helper scripts
    write_script(&scripts_dir.join("static_gate.sh"), STATIC_GATE_SH, force).await?;
    write_script(&scripts_dir.join("decompose_gate.py"), DECOMPOSE_GATE_PY, force).await?;
    write_script(&scripts_dir.join("aggregate_governance.py"), AGGREGATE_GOVERNANCE_PY, force).await?;

    // Step 5 — Update .gitignore
    update_gitignore(&target).await?;

    // Step 6 — Update AGENTS.md if it exists
    update_agents_md(&target).await?;

    // Step 7 — Print summary
    eprintln!();
    eprintln!("✓ Synodic Harness initialized");
    eprintln!();
    eprintln!("  Created:");
    eprintln!("    HARNESS.md              — Governance protocol (edit §5 to customize taxonomy)");
    eprintln!("    .harness/rules/         — Static rules directory (populated by crystallization)");
    eprintln!("    .harness/scripts/       — Helper scripts for governance checkpoints");
    for topo in &topologies {
        eprintln!("    .harness/{topo}.governance.jsonl — {topo} governance log");
    }
    eprintln!();
    eprintln!("  Next steps:");
    eprintln!("    1. Read HARNESS.md and customize §5 taxonomy if needed");
    eprintln!("    2. Add governance checkpoints to your skills (see §9 compliance checklist)");
    eprintln!("    3. Run your skills — governance logs accumulate automatically");
    eprintln!("    4. After 10+ runs: python .harness/scripts/aggregate_governance.py");

    Ok(())
}

async fn write_if_missing(path: &Path, content: &str, force: bool) -> Result<()> {
    if path.exists() && !force {
        return Ok(());
    }
    fs::write(path, content).await?;
    Ok(())
}

async fn write_script(path: &Path, content: &str, force: bool) -> Result<()> {
    if path.exists() && !force {
        eprintln!("  {} already exists. Skipping.", path.display());
        return Ok(());
    }
    fs::write(path, content).await?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(path, perms)?;
    }

    eprintln!("  Created {}", path.file_name().unwrap_or_default().to_string_lossy());
    Ok(())
}

async fn update_gitignore(target: &Path) -> Result<()> {
    let gitignore_path = target.join(".gitignore");
    let sentinel = "# Harness — per-run manifests are local";

    let existing = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path).await?
    } else {
        String::new()
    };

    if existing.contains(sentinel) {
        return Ok(());
    }

    let block = format!(
        "\n{sentinel}, governance logs are tracked\n\
         .factory/*/\n\
         .fractal/*/\n\
         \n\
         # Do NOT ignore:\n\
         # .harness/           (governance infrastructure)\n\
         # .harness/rules/     (crystallized rules)\n\
         # .harness/*.governance.jsonl  (learning logs)\n"
    );

    let updated = format!("{existing}{block}");
    fs::write(&gitignore_path, updated).await?;
    eprintln!("  Updated .gitignore");
    Ok(())
}

async fn update_agents_md(target: &Path) -> Result<()> {
    let agents_path = target.join("AGENTS.md");
    if !agents_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&agents_path).await?;
    if content.contains("HARNESS.md") {
        return Ok(());
    }

    let governance_block = "\n\n## Governance\n\n\
        All agent operations in this repository are subject to the governance protocol\n\
        defined in [HARNESS.md](./HARNESS.md). Before executing any skill, read HARNESS.md\n\
        to understand checkpoint requirements, feedback classification, and escalation rules.\n";

    let updated = format!("{content}{governance_block}");
    fs::write(&agents_path, updated).await?;
    eprintln!("  Updated AGENTS.md");
    Ok(())
}
