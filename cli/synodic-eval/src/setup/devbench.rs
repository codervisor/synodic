use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

/// Prepare a DevBench testbed for e2e evaluation.
///
/// This replaces setup/devbench.sh.
pub fn setup(
    project_name: &str,
    testbed_dir: &str,
    skill: &str,
    _repo_root: &Path,
) -> Result<()> {
    let testbed = PathBuf::from(testbed_dir);
    let task_dir = testbed.join(".devbench");
    let repo_dir = testbed.join("repo");

    println!("=== DevBench Testbed Setup ===");
    println!("Project:  {}", project_name);
    println!("Skill:    {}", skill);
    println!("Testbed:  {}", testbed_dir);
    println!();

    // --- Step 1: Get DevBench data ---
    println!("[1/4] Fetching DevBench project data...");
    std::fs::create_dir_all(&task_dir).context("create task data dir")?;

    let devbench_cache = PathBuf::from("/tmp/devbench-repo");

    if !devbench_cache.join(".git").exists() {
        println!("  Cloning DevBench repository...");
        Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                "https://github.com/open-compass/DevBench.git",
                &devbench_cache.to_string_lossy(),
            ])
            .status()
            .context("clone DevBench")?;
    } else {
        println!("  Using cached DevBench repo.");
    }

    // Find the project directory
    let benchmark_dir = devbench_cache.join("benchmark");
    let mut project_dir: Option<PathBuf> = None;
    let mut language = String::new();

    if benchmark_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&benchmark_dir) {
            for entry in entries.flatten() {
                let lang_dir = entry.path();
                if lang_dir.is_dir() {
                    let candidate = lang_dir.join(project_name);
                    if candidate.is_dir() {
                        language = entry
                            .file_name()
                            .to_string_lossy()
                            .to_string();
                        project_dir = Some(candidate);
                        break;
                    }
                }
            }
        }
    }

    let project_dir = project_dir.ok_or_else(|| {
        anyhow::anyhow!("Project '{}' not found in DevBench", project_name)
    })?;

    println!("  Found: {} (language: {})", project_name, language);

    // Copy project data to testbed
    let project_data = task_dir.join("project_data");
    if project_data.exists() {
        std::fs::remove_dir_all(&project_data).ok();
    }
    copy_dir_recursive(&project_dir, &project_data)?;
    println!();

    // --- Step 2: Extract PRD and test criteria ---
    println!("[2/4] Extracting PRD and acceptance criteria...");

    let prd_candidates = [
        project_data.join("PRD.md"),
        project_data.join("docs/PRD.md"),
        project_data.join("prd.md"),
    ];
    let prd_file = prd_candidates.iter().find(|p| p.exists());

    let prd_dest = task_dir.join("prd.md");
    if let Some(prd) = prd_file {
        std::fs::copy(prd, &prd_dest).context("copy PRD")?;
        let size = std::fs::metadata(&prd_dest).map(|m| m.len()).unwrap_or(0);
        println!("  PRD: {} chars", size);
    } else {
        println!("  WARNING: No PRD found. Agent will receive project description only.");
        std::fs::write(
            &prd_dest,
            format!(
                "# {}\n\nBuild the {} project in {}.\n",
                project_name, project_name, language
            ),
        )
        .context("write fallback PRD")?;
    }

    // Copy supplementary docs
    for doc in &[
        "UML_class.md",
        "UML_sequence.md",
        "Architecture.md",
        "architecture.md",
    ] {
        let src = project_data.join(doc);
        if src.exists() {
            std::fs::copy(&src, task_dir.join(doc)).ok();
            println!("  Supplementary: {}", doc);
        }
    }

    // Copy acceptance tests
    let acceptance_src = project_data.join("acceptance_tests");
    let acceptance_dest = task_dir.join("acceptance_tests");
    if acceptance_src.is_dir() {
        copy_dir_recursive(&acceptance_src, &acceptance_dest)?;
        let count = count_test_files(&acceptance_dest);
        println!("  Acceptance tests: {} files", count);
    } else {
        std::fs::create_dir_all(&acceptance_dest).ok();
        println!("  No acceptance tests found.");
    }

    // Save metadata
    let meta = serde_json::json!({
        "benchmark": "devbench",
        "project_name": project_name,
        "language": language,
        "has_prd": prd_dest.exists(),
        "has_architecture": task_dir.join("Architecture.md").exists() || task_dir.join("architecture.md").exists(),
        "has_uml_class": task_dir.join("UML_class.md").exists(),
        "has_uml_sequence": task_dir.join("UML_sequence.md").exists(),
    });
    std::fs::write(
        task_dir.join("meta.json"),
        serde_json::to_string_pretty(&meta).unwrap(),
    )
    .context("write meta.json")?;
    println!();

    // --- Step 3: Create build scaffold ---
    println!("[3/4] Creating build scaffold...");
    std::fs::create_dir_all(&repo_dir).context("create repo dir")?;

    if !repo_dir.join(".git").exists() {
        Command::new("git")
            .args(["init"])
            .current_dir(&repo_dir)
            .stdout(std::process::Stdio::null())
            .status()
            .context("git init")?;
        Command::new("git")
            .args([
                "-c", "user.name=synodic-eval",
                "-c", "user.email=eval@synodic-eval",
                "commit",
                "--allow-empty",
                "-m",
                &format!("Initial empty commit for DevBench: {}", project_name),
            ])
            .current_dir(&repo_dir)
            .stdout(std::process::Stdio::null())
            .status()
            .context("git initial commit")?;
    }
    println!("  Scaffold created.");
    println!();

    // --- Step 4: Write agent prompt ---
    println!("[4/4] Generating agent prompt (skill: {})...", skill);
    let prd_content =
        std::fs::read_to_string(&prd_dest).context("read PRD")?;

    let mut prompt = generate_devbench_prompt(skill, &prd_content, &language, &repo_dir);

    // Append supplementary docs
    for doc in &[
        "Architecture.md",
        "architecture.md",
        "UML_class.md",
        "UML_sequence.md",
    ] {
        let path = task_dir.join(doc);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let heading = doc.replace(".md", "").replace('_', " ");
                prompt.push_str(&format!("\n\n## {}\n\n{}", heading, content));
            }
        }
    }

    // Skill-specific footer
    match skill {
        "fractal" => {
            prompt.push_str(&format!(
                "\n\n## Configuration\n\n\
                 ```\n\
                 Config: output_mode=code, max_depth=3, max_children=6, solve_mode=parallel\n\
                 Language: {}\n\
                 Repo root: {}\n\
                 Benchmark: DevBench\n\
                 ```\n\n\
                 Follow the full orchestration protocol from SKILL.md.\n",
                language,
                repo_dir.display()
            ));
        }
        "factory" => {
            prompt.push_str(&format!(
                "\n\n## Configuration\n\n\
                 ```\n\
                 Language: {}\n\
                 Repo root: {}\n\
                 Benchmark: DevBench\n\
                 ```\n\n\
                 Follow the full orchestration protocol from SKILL.md.\n",
                language,
                repo_dir.display()
            ));
        }
        _ => {
            prompt.push_str(&format!(
                "\n\n## Configuration\n\n\
                 ```\n\
                 Language: {}\n\
                 Repo root: {}\n\
                 Benchmark: DevBench\n\
                 ```\n",
                language,
                repo_dir.display()
            ));
        }
    }

    let prompt_file = task_dir.join("agent_prompt.md");
    std::fs::write(&prompt_file, &prompt).context("write agent prompt")?;
    println!("  Agent prompt written to: {}", prompt_file.display());
    println!();

    // --- Summary ---
    println!("=== Setup Complete ===");
    println!();
    println!("Benchmark:        DevBench");
    println!("Skill:            {}", skill);
    println!("Project:          {} ({})", project_name, language);
    println!("Testbed:          {}", testbed_dir);
    println!("Repo:             {}", repo_dir.display());
    println!("PRD:              {} chars", prd_content.len());
    println!("Agent prompt:     {}", prompt_file.display());
    println!();

    Ok(())
}

fn generate_devbench_prompt(
    skill: &str,
    prd: &str,
    _language: &str,
    _repo_dir: &Path,
) -> String {
    let mut prompt = String::new();

    match skill {
        "fractal" => {
            prompt.push_str(
                "# DevBench E2E Evaluation — Fractal Decomposition\n\n\
                 You have the fractal decomposition skill loaded.\n\n\
                 ## Instructions\n\n\
                 You are building a complete project from a Product Requirements Document (PRD).\n\
                 This is NOT a bug fix or feature addition — you are building from scratch.\n\n\
                 1. Read the PRD below carefully.\n\
                 2. Use `/fractal decompose` with `output_mode=code` to build the project.\n\
                    - The decomposer should split the PRD into orthogonal modules/components.\n\
                    - Each SOLVE agent builds one module in an isolated worktree.\n\
                    - REUNIFY integrates all modules into a working project.\n\
                 3. All code must be written to the repo directory.\n\
                 4. The project must compile/run and pass acceptance tests.\n\n\
                 ## Product Requirements Document\n\n",
            );
        }
        "factory" => {
            prompt.push_str(
                "# DevBench E2E Evaluation — Factory Skill\n\n\
                 You have the factory skill loaded.\n\n\
                 ## Instructions\n\n\
                 You are building a complete project from a Product Requirements Document (PRD).\n\n\
                 1. Read the PRD below carefully.\n\
                 2. Create a spec from the PRD, then run `/factory run` on it.\n\
                 3. All code must be written to the repo directory.\n\
                 4. The project must compile/run and pass acceptance tests.\n\n\
                 ## Product Requirements Document\n\n",
            );
        }
        "baseline" => {
            prompt.push_str(
                "# DevBench E2E Evaluation — Baseline (No Skill)\n\n\
                 ## Instructions\n\n\
                 You are building a complete project from a Product Requirements Document (PRD).\n\n\
                 1. Read the PRD below carefully.\n\
                 2. Implement the project directly.\n\
                 3. The project must compile/run and pass acceptance tests.\n\n\
                 ## Product Requirements Document\n\n",
            );
        }
        other => {
            prompt.push_str(&format!(
                "# DevBench E2E Evaluation — {}\n\n\
                 ## Instructions\n\n\
                 Build the project described in the PRD below.\n\n\
                 ## Product Requirements Document\n\n",
                other
            ));
        }
    }

    prompt.push_str(prd);
    prompt
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

fn count_test_files(dir: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".py") || name.ends_with(".js") || name.ends_with(".sh") {
                count += 1;
            }
        }
    }
    count
}
