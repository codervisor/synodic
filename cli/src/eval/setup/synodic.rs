use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

/// Prepare a Synodic dogfood testbed for e2e evaluation.
///
/// A synodic instance clones the Synodic repo at a base commit, reads
/// the spec from the *current* Synodic install, and generates an agent
/// prompt. Scoring runs `cargo test` in the testbed's CLI directory.
pub fn setup(
    instance_alias: &str,
    testbed_dir: &str,
    skill: &str,
    repo_root: &Path,
) -> Result<()> {
    let testbed = PathBuf::from(testbed_dir);
    let task_dir = testbed.join(".synodic");
    let repo_dir = testbed.join("repo");

    println!("=== Synodic Dogfood Testbed Setup ===");
    println!("Instance: {}", instance_alias);
    println!("Skill:    {}", skill);
    println!("Testbed:  {}", testbed_dir);
    println!();

    // --- Step 1: Read instance metadata ---
    println!("[1/4] Reading instance metadata...");
    std::fs::create_dir_all(&task_dir).context("create task data dir")?;

    let meta_file = repo_root
        .join("evals/tasks/synodic")
        .join(format!("{}.meta.json", instance_alias));

    if !meta_file.exists() {
        // List available instances for helpful error message
        let tasks_dir = repo_root.join("evals/tasks/synodic");
        let available: Vec<String> = if tasks_dir.is_dir() {
            std::fs::read_dir(&tasks_dir)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.strip_suffix(".meta.json").map(|s| s.to_string())
                })
                .collect()
        } else {
            vec![]
        };
        bail!(
            "Instance metadata not found: {}\nAvailable instances: {}",
            meta_file.display(),
            if available.is_empty() {
                "(none)".to_string()
            } else {
                available.join(", ")
            }
        );
    }

    let meta = load_meta(&meta_file)?;
    println!("  Repo:        {}", meta.repo);
    println!("  Base commit: {}", &meta.base_commit[..12.min(meta.base_commit.len())]);
    println!("  Spec:        {}", meta.spec_path);
    println!("  Score dir:   {}", meta.score_dir);

    // Copy metadata to testbed
    std::fs::copy(&meta_file, task_dir.join("meta.json")).context("copy meta.json")?;
    println!();

    // --- Step 2: Clone the Synodic repo at base commit ---
    println!("[2/4] Cloning target repo...");

    if repo_dir.join(".git").exists() {
        println!("  Repo already cloned, resetting...");
        let status = Command::new("git")
            .args(["checkout", "-f", &meta.base_commit])
            .current_dir(&repo_dir)
            .status();
        if status.map(|s| !s.success()).unwrap_or(true) {
            Command::new("git")
                .args(["fetch", "origin"])
                .current_dir(&repo_dir)
                .status()
                .context("git fetch")?;
            Command::new("git")
                .args(["checkout", "-f", &meta.base_commit])
                .current_dir(&repo_dir)
                .status()
                .context("git checkout after fetch")?;
        }
    } else {
        println!("  Cloning https://github.com/{}...", meta.repo);
        Command::new("git")
            .args([
                "clone",
                "--no-checkout",
                &format!("https://github.com/{}.git", meta.repo),
                &repo_dir.to_string_lossy(),
            ])
            .status()
            .context("git clone")?;
        Command::new("git")
            .args(["checkout", "-f", &meta.base_commit])
            .current_dir(&repo_dir)
            .status()
            .context("git checkout")?;
    }
    println!(
        "  Checked out at {}",
        &meta.base_commit[..12.min(meta.base_commit.len())]
    );
    println!();

    // --- Step 3: Verify build ---
    println!("[3/4] Verifying build...");

    let score_dir = repo_dir.join(&meta.score_dir);
    let build_ok = Command::new("cargo")
        .args(["build", "--quiet"])
        .current_dir(&score_dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if build_ok {
        println!("  Build: OK");
    } else {
        println!("  WARNING: Initial build failed. The agent will need to fix this.");
    }
    println!();

    // --- Step 4: Write agent prompt ---
    println!("[4/4] Generating agent prompt (skill: {})...", skill);

    let spec_abs = repo_root.join(&meta.spec_path);
    if !spec_abs.exists() {
        bail!("Spec file not found: {}", spec_abs.display());
    }
    let spec_content = std::fs::read_to_string(&spec_abs).context("read spec")?;

    // Save problem statement
    std::fs::write(task_dir.join("problem_statement.txt"), &spec_content)
        .context("write problem statement")?;

    // Generate prompt
    let prompt = generate_synodic_prompt(skill, &spec_content, &repo_dir, &meta.score_dir);
    let prompt_file = task_dir.join("agent_prompt.md");
    std::fs::write(&prompt_file, &prompt).context("write agent prompt")?;
    println!("  Agent prompt written to: {}", prompt_file.display());
    println!();

    // --- Summary ---
    println!("=== Setup Complete ===");
    println!();
    println!("Benchmark:    Synodic (dogfood)");
    println!("Skill:        {}", skill);
    println!("Instance:     {}", instance_alias);
    println!("Testbed:      {}", testbed_dir);
    println!("Repo:         {}", repo_dir.display());
    println!("Score dir:    {}", score_dir.display());
    println!("Spec:         {}", spec_abs.display());
    println!("Agent prompt: {}", prompt_file.display());
    println!();

    Ok(())
}

/// Metadata from a synodic instance .meta.json file.
struct InstanceMeta {
    repo: String,
    base_commit: String,
    spec_path: String,
    score_dir: String,
}

fn load_meta(meta_file: &Path) -> Result<InstanceMeta> {
    let content = std::fs::read_to_string(meta_file).context("read meta.json")?;
    let v: serde_json::Value = serde_json::from_str(&content).context("parse meta.json")?;
    Ok(InstanceMeta {
        repo: v["repo"].as_str().context("meta.repo")?.to_string(),
        base_commit: v["base_commit"]
            .as_str()
            .context("meta.base_commit")?
            .to_string(),
        spec_path: v["spec_path"].as_str().context("meta.spec_path")?.to_string(),
        score_dir: v["score_dir"].as_str().context("meta.score_dir")?.to_string(),
    })
}

/// Generate a skill-specific agent prompt for a synodic dogfood task.
fn generate_synodic_prompt(
    skill: &str,
    spec_content: &str,
    repo_dir: &Path,
    score_dir: &str,
) -> String {
    let mut prompt = String::new();

    match skill {
        "factory" => {
            prompt.push_str(
                "# Synodic Dogfood Evaluation — Factory Skill\n\n\
                 You have the factory skill loaded.\n\n\
                 ## Instructions\n\n\
                 1. Read the spec below carefully.\n\
                 2. Analyze the codebase to understand what needs to be implemented.\n\
                 3. Run `/factory run` with the spec to implement the changes.\n\
                 4. After implementation, run `cargo test` in the `cli/` directory to verify.\n\n\
                 ## Spec\n\n",
            );
        }
        "fractal" => {
            prompt.push_str(
                "# Synodic Dogfood Evaluation — Fractal Decomposition\n\n\
                 You have the fractal decomposition skill loaded.\n\n\
                 ## Instructions\n\n\
                 1. Read the spec below carefully.\n\
                 2. Analyze the codebase to understand what needs to be implemented.\n\
                 3. Use `/fractal decompose` with `output_mode=code` to implement the spec.\n\
                 4. After implementation, run `cargo test` in the `cli/` directory to verify.\n\n\
                 ## Spec\n\n",
            );
        }
        "baseline" => {
            prompt.push_str(
                "# Synodic Dogfood Evaluation — Baseline (No Skill)\n\n\
                 ## Instructions\n\n\
                 1. Read the spec below carefully.\n\
                 2. Analyze the codebase to understand what needs to be implemented.\n\
                 3. Implement the spec directly.\n\
                 4. After implementation, run `cargo test` in the `cli/` directory to verify.\n\n\
                 ## Spec\n\n",
            );
        }
        other => {
            prompt.push_str(&format!(
                "# Synodic Dogfood Evaluation — {}\n\n\
                 ## Instructions\n\n\
                 1. Read the spec below carefully.\n\
                 2. Implement the spec in the Synodic codebase.\n\
                 3. Run `cargo test` in `cli/` to verify.\n\n\
                 ## Spec\n\n",
                other
            ));
        }
    }

    prompt.push_str(spec_content);

    prompt.push_str(&format!(
        "\n\n## Configuration\n\n\
         ```\n\
         Repo root:  {}\n\
         Score dir:  {}/{}\n\
         Score cmd:  cargo test\n\
         Benchmark:  synodic (dogfood)\n\
         ```\n\n\
         Follow the full orchestration protocol from SKILL.md.\n",
        repo_dir.display(),
        repo_dir.display(),
        score_dir,
    ));

    prompt
}
