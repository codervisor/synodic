use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

/// Prepare a FeatureBench testbed for e2e evaluation.
///
/// This replaces setup/featurebench.sh.
pub fn setup(
    instance_id: &str,
    testbed_dir: &str,
    skill: &str,
    _repo_root: &Path,
) -> Result<()> {
    let testbed = PathBuf::from(testbed_dir);
    let task_dir = testbed.join(".featurebench");
    let repo_dir = testbed.join("repo");
    let venv_dir = testbed.join("venv");

    println!("=== FeatureBench Testbed Setup ===");
    println!("Instance: {}", instance_id);
    println!("Skill:    {}", skill);
    println!("Testbed:  {}", testbed_dir);
    println!();

    // --- Step 1: Download task data ---
    println!("[1/6] Downloading task data from HuggingFace...");
    std::fs::create_dir_all(&task_dir).context("create task data dir")?;

    let download_script = r#"
import json, os, sys
try:
    from huggingface_hub import hf_hub_download
except ImportError:
    print('ERROR: huggingface_hub not installed. Run: pip install huggingface_hub', file=sys.stderr)
    sys.exit(1)

from datasets import load_dataset

instance_id = os.environ['SYNODIC_INSTANCE_ID']
task_dir = os.environ['SYNODIC_TASK_DIR']

ds = load_dataset('LiberCoders/FeatureBench', split='lite')

matches = [row for row in ds if row['instance_id'] == instance_id]
if not matches:
    matches = [row for row in ds if instance_id in row['instance_id']]

if not matches:
    print(f'ERROR: Instance {instance_id} not found in FeatureBench dataset', file=sys.stderr)
    sys.exit(1)

task = matches[0]
print(f'Found: {task["instance_id"]}')
print(f'  Repo: {task["repo"]}')
print(f'  Base commit: {task["base_commit"]}')

with open(f'{task_dir}/task.json', 'w') as f:
    json.dump(dict(task), f, indent=2, default=str)

with open(f'{task_dir}/problem_statement.txt', 'w') as f:
    f.write(task['problem_statement'])

if task.get('test_patch'):
    with open(f'{task_dir}/test_patch.diff', 'w') as f:
        f.write(task['test_patch'])

if task.get('patch'):
    with open(f'{task_dir}/patch.diff', 'w') as f:
        f.write(task['patch'])

for key in ['FAIL_TO_PASS', 'PASS_TO_PASS']:
    if task.get(key):
        with open(f'{task_dir}/{key.lower()}.json', 'w') as f:
            f.write(task[key] if isinstance(task[key], str) else json.dumps(task[key]))

meta = {
    'instance_id': task['instance_id'],
    'repo': task['repo'],
    'base_commit': task['base_commit'],
    'environment_setup_commit': task.get('environment_setup_commit', ''),
    'problem_statement_length': len(task['problem_statement']),
    'has_patch': bool(task.get('patch')),
    'has_test_patch': bool(task.get('test_patch')),
    'has_hints': bool(task.get('hints_text')),
}
with open(f'{task_dir}/meta.json', 'w') as f:
    json.dump(meta, f, indent=2)

print('Task data saved.')
"#;

    let status = Command::new("python3")
        .args(["-c", download_script])
        .env("SYNODIC_INSTANCE_ID", instance_id)
        .env("SYNODIC_TASK_DIR", task_dir.to_string_lossy().as_ref())
        .status()
        .context("run HuggingFace download")?;
    if !status.success() {
        bail!("Failed to download FeatureBench task data");
    }
    println!();

    // --- Step 2: Clone the target repo ---
    println!("[2/6] Cloning target repo...");
    let meta = load_meta(&task_dir)?;

    if repo_dir.join(".git").exists() {
        println!("  Repo already cloned, resetting...");
        let status = Command::new("git")
            .args(["checkout", "-f", &meta.base_commit])
            .current_dir(&repo_dir)
            .status();
        if status.map(|s| !s.success()).unwrap_or(true) {
            println!("  Checkout failed, fetching from origin...");
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

    // --- Step 3: Strip implementation code ---
    println!("[3/6] Stripping implementation code (applying gold patch)...");
    let impl_patch = task_dir.join("patch.diff");
    if impl_patch.exists()
        && std::fs::metadata(&impl_patch)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
    {
        let check = Command::new("git")
            .args(["apply", "--check", &impl_patch.to_string_lossy()])
            .current_dir(&repo_dir)
            .output()
            .context("git apply --check")?;
        if check.status.success() {
            Command::new("git")
                .args(["apply", &impl_patch.to_string_lossy()])
                .current_dir(&repo_dir)
                .status()
                .context("git apply")?;
            println!("  Implementation stripped.");
        } else {
            println!("  WARNING: Patch doesn't apply cleanly, trying with --3way...");
            let status = Command::new("git")
                .args(["apply", "--3way", &impl_patch.to_string_lossy()])
                .current_dir(&repo_dir)
                .status();
            if status.map(|s| !s.success()).unwrap_or(true) {
                bail!("Could not strip implementation code. Gold patch failed to apply.");
            }
        }
    } else {
        bail!("No gold patch (patch.diff) found. Cannot create testbed.");
    }
    println!();

    // --- Step 4: Sanity check ---
    println!("[4/6] Sanity check — verifying F2P tests fail without implementation...");
    // Best-effort sanity check; don't fail setup if check fails
    let f2p_file = task_dir.join("fail_to_pass.json");
    if f2p_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&f2p_file) {
            let tests = super::super::score::parser::parse_test_list(&content);
            if let Some(first) = tests.first() {
                println!("  Checking: {}", first);
                let python = if venv_dir.join("bin/python").exists() {
                    venv_dir.join("bin/python").to_string_lossy().to_string()
                } else {
                    "python3".into()
                };
                let status = Command::new(&python)
                    .args(["-m", "pytest", first, "--tb=no", "-q", "--no-header"])
                    .current_dir(&repo_dir)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
                match status {
                    Ok(s) if s.success() => {
                        println!("  WARNING: F2P tests PASS without implementation!");
                    }
                    _ => {
                        println!("  OK — F2P tests fail as expected.");
                    }
                }
            }
        }
    } else {
        println!("  No F2P test list found, skipping sanity check.");
    }
    println!();

    // --- Step 5: Install dependencies ---
    println!("[5/6] Installing dependencies...");
    install_python_deps(&repo_dir, &venv_dir)?;
    println!();

    // --- Step 6: Write agent prompt ---
    println!("[6/6] Generating agent prompt (skill: {})...", skill);
    let problem_stmt = std::fs::read_to_string(task_dir.join("problem_statement.txt"))
        .context("read problem statement")?;

    let prompt = generate_featurebench_prompt(skill, &problem_stmt, &repo_dir);
    let prompt_file = task_dir.join("agent_prompt.md");
    std::fs::write(&prompt_file, &prompt).context("write agent prompt")?;
    println!("  Agent prompt written to: {}", prompt_file.display());
    println!();

    // --- Summary ---
    println!("=== Setup Complete ===");
    println!();
    println!("Testbed:          {}", testbed_dir);
    println!("Repo:             {}", repo_dir.display());
    println!("Skill:            {}", skill);
    println!("Problem statement: {} chars", problem_stmt.len());
    println!("Agent prompt:     {}", prompt_file.display());
    println!("Venv:             {}", venv_dir.display());
    println!();

    Ok(())
}

struct TaskMeta {
    repo: String,
    base_commit: String,
}

fn load_meta(task_dir: &Path) -> Result<TaskMeta> {
    let content =
        std::fs::read_to_string(task_dir.join("meta.json")).context("read meta.json")?;
    let v: serde_json::Value = serde_json::from_str(&content).context("parse meta.json")?;
    Ok(TaskMeta {
        repo: v["repo"].as_str().context("meta.repo")?.to_string(),
        base_commit: v["base_commit"]
            .as_str()
            .context("meta.base_commit")?
            .to_string(),
    })
}

fn install_python_deps(repo_dir: &Path, venv_dir: &Path) -> Result<()> {
    let has_python = repo_dir.join("setup.py").exists()
        || repo_dir.join("pyproject.toml").exists()
        || repo_dir.join("setup.cfg").exists();

    if !has_python {
        println!("  WARNING: Unknown project type. Skipping dependency install.");
        return Ok(());
    }

    println!("  Python project detected.");

    if !venv_dir.exists() {
        Command::new("python3")
            .args(["-m", "venv", &venv_dir.to_string_lossy()])
            .status()
            .context("create venv")?;
    }

    let pip = venv_dir.join("bin/pip").to_string_lossy().to_string();

    let mut installed = false;
    for spec in &[".[dev]", ".", ".[test]"] {
        let status = Command::new(&pip)
            .args(["install", "-e", spec])
            .current_dir(repo_dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if status.map(|s| s.success()).unwrap_or(false) {
            installed = true;
            break;
        }
    }
    if !installed {
        println!("  WARNING: Could not install package. Tests may fail.");
    }

    let _ = Command::new(&pip)
        .args(["install", "pytest"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    println!("  Dependencies installed in venv: {}", venv_dir.display());
    Ok(())
}

fn generate_featurebench_prompt(skill: &str, problem_stmt: &str, repo_dir: &Path) -> String {
    let mut prompt = String::new();

    match skill {
        "fractal" => {
            prompt.push_str(
                "# FeatureBench E2E Evaluation — Fractal Decomposition\n\n\
                 You have the fractal decomposition skill loaded.\n\n\
                 ## Instructions\n\n\
                 1. Read the FULL problem statement below carefully — including all Interface Descriptions.\n\
                 2. Use `/fractal decompose` with `output_mode=code` to implement the feature.\n\
                 3. All code must be written to the testbed repo (paths are relative to the repo root).\n\
                 4. After implementation, run the test suite to verify.\n\n\
                 ## Problem Statement\n\n",
            );
        }
        "factory" => {
            prompt.push_str(
                "# FeatureBench E2E Evaluation — Factory Skill\n\n\
                 You have the factory skill loaded.\n\n\
                 ## Instructions\n\n\
                 1. Read the FULL problem statement below carefully — including all Interface Descriptions.\n\
                 2. Create a spec from the problem statement, then run `/factory run` on it.\n\
                 3. All code must be written to the testbed repo (paths are relative to the repo root).\n\
                 4. After implementation, run the test suite to verify.\n\n\
                 ## Problem Statement\n\n",
            );
        }
        "baseline" => {
            prompt.push_str(
                "# FeatureBench E2E Evaluation — Baseline (No Skill)\n\n\
                 ## Instructions\n\n\
                 1. Read the FULL problem statement below carefully — including all Interface Descriptions.\n\
                 2. Implement the feature directly. Write code to the testbed repo.\n\
                 3. After implementation, run the test suite to verify.\n\n\
                 ## Problem Statement\n\n",
            );
        }
        other => {
            prompt.push_str(&format!(
                "# FeatureBench E2E Evaluation — {}\n\n\
                 ## Instructions\n\n\
                 1. Read the FULL problem statement below carefully.\n\
                 2. Implement the feature. All code goes in the testbed repo.\n\
                 3. Run the test suite to verify.\n\n\
                 ## Problem Statement\n\n",
                other
            ));
        }
    }

    prompt.push_str(problem_stmt);

    match skill {
        "fractal" => {
            prompt.push_str(&format!(
                "\n\n## Configuration\n\n\
                 ```\n\
                 Config: output_mode=code, max_depth=3, max_children=6, solve_mode=parallel\n\
                 Repo root: {}\n\
                 ```\n\n\
                 Follow the full orchestration protocol from SKILL.md.\n",
                repo_dir.display()
            ));
        }
        "factory" => {
            prompt.push_str(&format!(
                "\n\n## Configuration\n\n\
                 ```\n\
                 Repo root: {}\n\
                 ```\n\n\
                 Follow the full orchestration protocol from SKILL.md.\n",
                repo_dir.display()
            ));
        }
        _ => {
            prompt.push_str(&format!(
                "\n\n## Configuration\n\n\
                 ```\n\
                 Repo root: {}\n\
                 ```\n",
                repo_dir.display()
            ));
        }
    }

    prompt
}
