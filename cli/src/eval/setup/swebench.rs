use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

/// Prepare a SWE-bench testbed for e2e evaluation.
///
/// This replaces setup/swebench.sh.
///
/// Steps:
///   1. Download task data from HuggingFace (via Python)
///   2. Clone the target repo at the base commit
///   3. Apply the test patch
///   4. Install dependencies
///   5. Write the agent prompt (skill-specific)
pub fn setup(
    instance_id: &str,
    testbed_dir: &str,
    split: &str,
    skill: &str,
    _repo_root: &Path,
) -> Result<()> {
    let testbed = PathBuf::from(testbed_dir);
    let task_dir = testbed.join(".swebench");
    let repo_dir = testbed.join("repo");
    let venv_dir = testbed.join("venv");

    // Map split to HuggingFace dataset
    let (hf_dataset, hf_split) = match split {
        "verified" => ("princeton-nlp/SWE-bench_Verified", "test"),
        "lite" => ("princeton-nlp/SWE-bench_Lite", "test"),
        "pro" => ("ScaleAI/SWE-bench_Pro", "test"),
        "full" => ("princeton-nlp/SWE-bench", "test"),
        _ => bail!("Unknown split: {} (use verified, lite, pro, or full)", split),
    };

    println!("=== SWE-bench Testbed Setup ===");
    println!("Instance: {}", instance_id);
    println!("Split:    {} ({})", split, hf_dataset);
    println!("Skill:    {}", skill);
    println!("Testbed:  {}", testbed_dir);
    println!();

    // --- Step 1: Download task data ---
    println!("[1/5] Downloading task data from HuggingFace...");
    std::fs::create_dir_all(&task_dir).context("create task data dir")?;

    let download_script = format!(
        r#"
import json, sys
try:
    from datasets import load_dataset
except ImportError:
    print('ERROR: datasets not installed. Run: pip install datasets', file=sys.stderr)
    sys.exit(1)

instance_id = '{instance_id}'
task_dir = '{task_dir}'

ds = load_dataset('{hf_dataset}', split='{hf_split}')

matches = [row for row in ds if row['instance_id'] == instance_id]
if not matches:
    matches = [row for row in ds if instance_id in row['instance_id']]

if not matches:
    print(f'ERROR: Instance {{instance_id}} not found in {hf_dataset}', file=sys.stderr)
    sys.exit(1)

task = matches[0]
print(f'Found: {{task["instance_id"]}}')
print(f'  Repo: {{task["repo"]}}')
print(f'  Base commit: {{task["base_commit"]}}')

with open(f'{{task_dir}}/task.json', 'w') as f:
    json.dump(dict(task), f, indent=2, default=str)

with open(f'{{task_dir}}/problem_statement.txt', 'w') as f:
    f.write(task['problem_statement'])

if task.get('test_patch'):
    with open(f'{{task_dir}}/test_patch.diff', 'w') as f:
        f.write(task['test_patch'])

for key in ['FAIL_TO_PASS', 'PASS_TO_PASS']:
    val = task.get(key)
    if val:
        with open(f'{{task_dir}}/{{key.lower()}}.json', 'w') as f:
            f.write(val if isinstance(val, str) else json.dumps(val))

if task.get('hints_text'):
    with open(f'{{task_dir}}/hints.txt', 'w') as f:
        f.write(task['hints_text'])

meta = {{
    'benchmark': 'swebench',
    'split': '{split}',
    'instance_id': task['instance_id'],
    'repo': task['repo'],
    'base_commit': task['base_commit'],
    'problem_statement_length': len(task['problem_statement']),
    'has_test_patch': bool(task.get('test_patch')),
    'has_hints': bool(task.get('hints_text')),
    'created_at': task.get('created_at', ''),
    'version': task.get('version', ''),
}}
with open(f'{{task_dir}}/meta.json', 'w') as f:
    json.dump(meta, f, indent=2)

print('Task data saved.')
"#,
        instance_id = instance_id,
        task_dir = task_dir.display(),
        hf_dataset = hf_dataset,
        hf_split = hf_split,
        split = split,
    );

    let status = Command::new("python3")
        .args(["-c", &download_script])
        .status()
        .context("run HuggingFace download")?;
    if !status.success() {
        bail!("Failed to download task data from HuggingFace");
    }
    println!();

    // --- Step 2: Clone the target repo ---
    println!("[2/5] Cloning target repo...");
    let meta = load_meta(&task_dir)?;
    let repo = meta.repo;
    let base_commit = meta.base_commit;

    if repo_dir.join(".git").exists() {
        println!("  Repo already cloned, resetting...");
        let status = Command::new("git")
            .args(["checkout", "-f", &base_commit])
            .current_dir(&repo_dir)
            .status();
        if status.map(|s| !s.success()).unwrap_or(true) {
            Command::new("git")
                .args(["fetch", "origin"])
                .current_dir(&repo_dir)
                .status()
                .context("git fetch")?;
            Command::new("git")
                .args(["checkout", "-f", &base_commit])
                .current_dir(&repo_dir)
                .status()
                .context("git checkout after fetch")?;
        }
    } else {
        println!("  Cloning https://github.com/{}...", repo);
        Command::new("git")
            .args([
                "clone",
                "--no-checkout",
                &format!("https://github.com/{}.git", repo),
                &repo_dir.to_string_lossy(),
            ])
            .status()
            .context("git clone")?;
        Command::new("git")
            .args(["checkout", "-f", &base_commit])
            .current_dir(&repo_dir)
            .status()
            .context("git checkout")?;
    }
    println!("  Checked out at {}", &base_commit[..12.min(base_commit.len())]);
    println!();

    // --- Step 3: Apply test patch ---
    println!("[3/5] Applying test patch...");
    let test_patch = task_dir.join("test_patch.diff");
    if test_patch.exists() && std::fs::metadata(&test_patch).map(|m| m.len() > 0).unwrap_or(false) {
        let check = Command::new("git")
            .args(["apply", "--check", &test_patch.to_string_lossy()])
            .current_dir(&repo_dir)
            .output()
            .context("git apply --check")?;
        if check.status.success() {
            Command::new("git")
                .args(["apply", &test_patch.to_string_lossy()])
                .current_dir(&repo_dir)
                .status()
                .context("git apply")?;
            println!("  Test patch applied.");
        } else {
            println!("  WARNING: Test patch doesn't apply cleanly, trying with --3way...");
            let status = Command::new("git")
                .args(["apply", "--3way", &test_patch.to_string_lossy()])
                .current_dir(&repo_dir)
                .status();
            if status.map(|s| !s.success()).unwrap_or(true) {
                eprintln!("  ERROR: Could not apply test patch");
            }
        }
    } else {
        println!("  No test patch to apply.");
    }
    println!();

    // --- Step 4: Install dependencies ---
    println!("[4/5] Installing dependencies...");
    install_python_deps(&repo_dir, &venv_dir)?;
    println!();

    // --- Step 5: Write agent prompt ---
    println!("[5/5] Generating agent prompt (skill: {})...", skill);
    let problem_stmt = std::fs::read_to_string(task_dir.join("problem_statement.txt"))
        .context("read problem statement")?;
    let hints = std::fs::read_to_string(task_dir.join("hints.txt")).ok();

    let prompt = generate_swebench_prompt(skill, &problem_stmt, hints.as_deref(), &repo_dir, split);
    let prompt_file = task_dir.join("agent_prompt.md");
    std::fs::write(&prompt_file, &prompt).context("write agent prompt")?;
    println!("  Agent prompt written to: {}", prompt_file.display());
    println!();

    // --- Summary ---
    println!("=== Setup Complete ===");
    println!();
    println!("Benchmark:        SWE-bench ({})", split);
    println!("Skill:            {}", skill);
    println!("Testbed:          {}", testbed_dir);
    println!("Repo:             {}", repo_dir.display());
    println!("Problem statement: {} chars", problem_stmt.len());
    println!("Agent prompt:     {}", prompt_file.display());
    println!("Venv:             {}", venv_dir.display());
    println!();

    Ok(())
}

/// Metadata from meta.json
struct TaskMeta {
    repo: String,
    base_commit: String,
}

fn load_meta(task_dir: &Path) -> Result<TaskMeta> {
    let content = std::fs::read_to_string(task_dir.join("meta.json"))
        .context("read meta.json")?;
    let v: serde_json::Value = serde_json::from_str(&content).context("parse meta.json")?;
    Ok(TaskMeta {
        repo: v["repo"].as_str().context("meta.repo")?.to_string(),
        base_commit: v["base_commit"].as_str().context("meta.base_commit")?.to_string(),
    })
}

/// Install Python dependencies into a venv.
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

    let pip = venv_dir.join("bin/pip");
    let pip_str = pip.to_string_lossy().to_string();

    // Try installing with various extras
    let has_pyproject = repo_dir.join("pyproject.toml").exists();
    let install_attempts = if has_pyproject {
        vec![".[dev,test]", ".[dev]", ".[test]", "."]
    } else {
        vec![".[dev]", "."]
    };

    let mut installed = false;
    for spec in install_attempts {
        let status = Command::new(&pip_str)
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
        println!("  WARNING: Could not install package.");
    }

    // Always install pytest
    let _ = Command::new(&pip_str)
        .args(["install", "pytest"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    println!(
        "  Dependencies installed in venv: {}",
        venv_dir.display()
    );
    Ok(())
}

/// Generate a skill-specific agent prompt for SWE-bench.
fn generate_swebench_prompt(
    skill: &str,
    problem_stmt: &str,
    hints: Option<&str>,
    repo_dir: &Path,
    split: &str,
) -> String {
    let mut prompt = String::new();

    // Skill-specific header
    match skill {
        "fractal" => {
            prompt.push_str(
                "# SWE-bench E2E Evaluation — Fractal Decomposition\n\n\
                 You have the fractal decomposition skill loaded.\n\n\
                 ## Instructions\n\n\
                 1. Read the issue description below carefully.\n\
                 2. Analyze the codebase to understand the relevant code.\n\
                 3. Use `/fractal decompose` with `output_mode=code` to implement the fix.\n\
                    - If the task is simple enough (1-2 files), the decomposer should detect it as a LEAF.\n\
                    - If the task spans multiple files/concerns, it will decompose accordingly.\n\
                 4. All code must be written to the testbed repo.\n\
                 5. After implementation, run the test suite to verify.\n\n\
                 ## Issue Description\n\n",
            );
        }
        "factory" => {
            prompt.push_str(
                "# SWE-bench E2E Evaluation — Factory Skill\n\n\
                 You have the factory skill loaded.\n\n\
                 ## Instructions\n\n\
                 1. Read the issue description below carefully.\n\
                 2. Analyze the codebase to understand the relevant code.\n\
                 3. Create a spec from the issue, then run `/factory run` on it.\n\
                 4. All code must be written to the testbed repo.\n\
                 5. After implementation, run the test suite to verify.\n\n\
                 ## Issue Description\n\n",
            );
        }
        "baseline" => {
            prompt.push_str(
                "# SWE-bench E2E Evaluation — Baseline (No Skill)\n\n\
                 ## Instructions\n\n\
                 1. Read the issue description below carefully.\n\
                 2. Analyze the codebase to understand the relevant code.\n\
                 3. Implement the fix directly.\n\
                 4. After implementation, run the test suite to verify.\n\n\
                 ## Issue Description\n\n",
            );
        }
        other => {
            prompt.push_str(&format!(
                "# SWE-bench E2E Evaluation — {}\n\n\
                 ## Instructions\n\n\
                 1. Read the issue description below carefully.\n\
                 2. Implement the fix.\n\
                 3. Run the test suite to verify.\n\n\
                 ## Issue Description\n\n",
                other
            ));
        }
    }

    prompt.push_str(problem_stmt);

    if let Some(hints) = hints {
        if !hints.is_empty() {
            prompt.push_str("\n\n## Hints\n\n");
            prompt.push_str(hints);
        }
    }

    // Skill-specific footer
    match skill {
        "fractal" => {
            prompt.push_str(&format!(
                "\n\n## Configuration\n\n\
                 ```\n\
                 Config: output_mode=code, max_depth=3, max_children=5, solve_mode=parallel\n\
                 Repo root: {}\n\
                 Benchmark: SWE-bench ({})\n\
                 ```\n\n\
                 Follow the full orchestration protocol from SKILL.md.\n",
                repo_dir.display(),
                split
            ));
        }
        "factory" => {
            prompt.push_str(&format!(
                "\n\n## Configuration\n\n\
                 ```\n\
                 Repo root: {}\n\
                 Benchmark: SWE-bench ({})\n\
                 ```\n\n\
                 Follow the full orchestration protocol from SKILL.md.\n",
                repo_dir.display(),
                split
            ));
        }
        _ => {
            prompt.push_str(&format!(
                "\n\n## Configuration\n\n\
                 ```\n\
                 Repo root: {}\n\
                 Benchmark: SWE-bench ({})\n\
                 ```\n",
                repo_dir.display(),
                split
            ));
        }
    }

    prompt
}
