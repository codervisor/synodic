#!/usr/bin/env bash
# setup-swebench.sh — Prepare a SWE-bench (Pro/Verified/Lite) testbed for e2e evaluation
#
# Usage: ./setup-swebench.sh <instance-id> [--testbed-dir <path>] [--split <verified|lite|pro>]
#
# This script:
#   1. Downloads the SWE-bench task from HuggingFace
#   2. Clones the target repo at the base commit
#   3. Applies the test patch
#   4. Installs dependencies
#   5. Writes the full problem statement for the agent
#
# Prerequisites:
#   - git, python3, pip
#   - datasets: pip install datasets

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# --- Argument parsing ---
INSTANCE_ID="${1:?Usage: setup-swebench.sh <instance-id> [--testbed-dir <path>] [--split <verified|lite|pro>]}"
shift

TESTBED_DIR=""
SPLIT="verified"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --testbed-dir) TESTBED_DIR="$2"; shift 2 ;;
    --split) SPLIT="$2"; shift 2 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

if [[ -z "$TESTBED_DIR" ]]; then
  TESTBED_DIR="/tmp/swebench-testbed/${INSTANCE_ID}"
fi

# Map split name to HuggingFace dataset
case "$SPLIT" in
  verified) HF_DATASET="princeton-nlp/SWE-bench_Verified"; HF_SPLIT="test" ;;
  lite)     HF_DATASET="princeton-nlp/SWE-bench_Lite"; HF_SPLIT="test" ;;
  pro)      HF_DATASET="scale-labs/SWE-bench_Pro_public"; HF_SPLIT="test" ;;
  full)     HF_DATASET="princeton-nlp/SWE-bench"; HF_SPLIT="test" ;;
  *)        echo "Unknown split: $SPLIT (use verified, lite, pro, or full)" >&2; exit 1 ;;
esac

echo "=== SWE-bench Testbed Setup ==="
echo "Instance: $INSTANCE_ID"
echo "Split:    $SPLIT ($HF_DATASET)"
echo "Testbed:  $TESTBED_DIR"
echo ""

# --- Step 1: Download task data ---
echo "[1/5] Downloading task data from HuggingFace..."

TASK_DATA_DIR="${TESTBED_DIR}/.swebench"
mkdir -p "$TASK_DATA_DIR"

python3 -c "
import json, sys
try:
    from datasets import load_dataset
except ImportError:
    print('ERROR: datasets not installed. Run: pip install datasets', file=sys.stderr)
    sys.exit(1)

instance_id = '${INSTANCE_ID}'
task_dir = '${TASK_DATA_DIR}'

ds = load_dataset('${HF_DATASET}', split='${HF_SPLIT}')

# Find our instance
matches = [row for row in ds if row['instance_id'] == instance_id]
if not matches:
    matches = [row for row in ds if instance_id in row['instance_id']]

if not matches:
    print(f'ERROR: Instance {instance_id} not found in ${HF_DATASET}', file=sys.stderr)
    print(f'Available instances ({len(ds)}). Showing first 20:')
    for row in list(ds)[:20]:
        print(f'  {row[\"instance_id\"]}')
    sys.exit(1)

task = matches[0]
print(f'Found: {task[\"instance_id\"]}')
print(f'  Repo: {task[\"repo\"]}')
print(f'  Base commit: {task[\"base_commit\"]}')
print(f'  Problem statement: {len(task[\"problem_statement\"])} chars')

# Save task data
with open(f'{task_dir}/task.json', 'w') as f:
    json.dump(dict(task), f, indent=2, default=str)

# Save problem statement
with open(f'{task_dir}/problem_statement.txt', 'w') as f:
    f.write(task['problem_statement'])

# Save test patch
if task.get('test_patch'):
    with open(f'{task_dir}/test_patch.diff', 'w') as f:
        f.write(task['test_patch'])

# Save test lists — SWE-bench stores these as JSON strings
for key in ['FAIL_TO_PASS', 'PASS_TO_PASS']:
    val = task.get(key)
    if val:
        with open(f'{task_dir}/{key.lower()}.json', 'w') as f:
            f.write(val if isinstance(val, str) else json.dumps(val))

# Save hints if available
if task.get('hints_text'):
    with open(f'{task_dir}/hints.txt', 'w') as f:
        f.write(task['hints_text'])

# Save metadata
meta = {
    'benchmark': 'swebench',
    'split': '${SPLIT}',
    'instance_id': task['instance_id'],
    'repo': task['repo'],
    'base_commit': task['base_commit'],
    'problem_statement_length': len(task['problem_statement']),
    'has_test_patch': bool(task.get('test_patch')),
    'has_hints': bool(task.get('hints_text')),
    'created_at': task.get('created_at', ''),
    'version': task.get('version', ''),
}
with open(f'{task_dir}/meta.json', 'w') as f:
    json.dump(meta, f, indent=2)

print('Task data saved.')
"

echo ""

# --- Step 2: Clone the target repo ---
echo "[2/5] Cloning target repo..."

META_FILE="${TASK_DATA_DIR}/meta.json"
REPO=$(python3 -c "import json; print(json.load(open('${META_FILE}'))['repo'])")
BASE_COMMIT=$(python3 -c "import json; print(json.load(open('${META_FILE}'))['base_commit'])")

REPO_DIR="${TESTBED_DIR}/repo"

if [[ -d "$REPO_DIR/.git" ]]; then
  echo "  Repo already cloned, resetting..."
  cd "$REPO_DIR"
  git checkout -f "$BASE_COMMIT" 2>/dev/null || (git fetch origin && git checkout -f "$BASE_COMMIT")
else
  echo "  Cloning https://github.com/${REPO}..."
  git clone --no-checkout "https://github.com/${REPO}.git" "$REPO_DIR"
  cd "$REPO_DIR"
  git checkout -f "$BASE_COMMIT"
fi

echo "  Checked out at ${BASE_COMMIT:0:12}"
echo ""

# --- Step 3: Apply test patch ---
echo "[3/5] Applying test patch..."

TEST_PATCH="${TASK_DATA_DIR}/test_patch.diff"
if [[ -f "$TEST_PATCH" ]] && [[ -s "$TEST_PATCH" ]]; then
  cd "$REPO_DIR"
  if git apply --check "$TEST_PATCH" 2>/dev/null; then
    git apply "$TEST_PATCH"
    echo "  Test patch applied."
  else
    echo "  WARNING: Test patch doesn't apply cleanly, trying with --3way..."
    git apply --3way "$TEST_PATCH" || echo "  ERROR: Could not apply test patch" >&2
  fi
else
  echo "  No test patch to apply."
fi

echo ""

# --- Step 4: Install dependencies ---
echo "[4/5] Installing dependencies..."

cd "$REPO_DIR"

if [[ -f "setup.py" ]] || [[ -f "pyproject.toml" ]] || [[ -f "setup.cfg" ]]; then
  echo "  Python project detected."

  VENV_DIR="${TESTBED_DIR}/venv"
  if [[ ! -d "$VENV_DIR" ]]; then
    python3 -m venv "$VENV_DIR"
  fi
  source "${VENV_DIR}/bin/activate"

  # SWE-bench repos often need specific install procedures
  # Try common patterns in order of preference
  if [[ -f "pyproject.toml" ]]; then
    pip install -e ".[dev,test]" 2>/dev/null || \
      pip install -e ".[dev]" 2>/dev/null || \
      pip install -e ".[test]" 2>/dev/null || \
      pip install -e "." 2>/dev/null || \
      echo "  WARNING: Could not install package."
  else
    pip install -e ".[dev]" 2>/dev/null || \
      pip install -e "." 2>/dev/null || \
      echo "  WARNING: Could not install package."
  fi

  pip install pytest 2>/dev/null
  echo "  Dependencies installed in venv: $VENV_DIR"

elif [[ -f "package.json" ]]; then
  echo "  Node.js project detected."
  if command -v pnpm &>/dev/null; then
    pnpm install 2>/dev/null || npm install 2>/dev/null
  else
    npm install 2>/dev/null
  fi
  echo "  Dependencies installed."

elif [[ -f "Cargo.toml" ]]; then
  echo "  Rust project detected."
  cargo build 2>/dev/null || echo "  WARNING: cargo build failed."

else
  echo "  WARNING: Unknown project type. Skipping dependency install."
fi

echo ""

# --- Step 5: Write agent prompt ---
echo "[5/5] Generating agent prompt..."

PROMPT_FILE="${TASK_DATA_DIR}/agent_prompt.md"
PROBLEM_STMT="${TASK_DATA_DIR}/problem_statement.txt"
HINTS_FILE="${TASK_DATA_DIR}/hints.txt"

cat > "$PROMPT_FILE" << 'PROMPT_HEADER'
# SWE-bench E2E Evaluation — Fractal Decomposition

You have the fractal decomposition skill loaded.

## Instructions

1. Read the issue description below carefully.
2. Analyze the codebase to understand the relevant code.
3. Use `/fractal decompose` with `output_mode=code` to implement the fix.
   - If the task is simple enough (1-2 files), the decomposer should detect it as a LEAF.
   - If the task spans multiple files/concerns, it will decompose accordingly.
4. All code must be written to the testbed repo.
5. After implementation, run the test suite to verify.

## Issue Description

PROMPT_HEADER

cat "$PROBLEM_STMT" >> "$PROMPT_FILE"

# Include hints if available
if [[ -f "$HINTS_FILE" ]] && [[ -s "$HINTS_FILE" ]]; then
  echo "" >> "$PROMPT_FILE"
  echo "## Hints" >> "$PROMPT_FILE"
  echo "" >> "$PROMPT_FILE"
  cat "$HINTS_FILE" >> "$PROMPT_FILE"
fi

cat >> "$PROMPT_FILE" << PROMPT_FOOTER

## Configuration

\`\`\`
Config: output_mode=code, max_depth=3, max_children=5, solve_mode=parallel
Repo root: ${REPO_DIR}
Benchmark: SWE-bench (${SPLIT})
\`\`\`

Follow the full orchestration protocol from SKILL.md.
PROMPT_FOOTER

echo "  Agent prompt written to: $PROMPT_FILE"
echo ""

# --- Summary ---
echo "=== Setup Complete ==="
echo ""
echo "Benchmark:        SWE-bench ($SPLIT)"
echo "Testbed:          $TESTBED_DIR"
echo "Repo:             $REPO_DIR"
echo "Problem statement: $(wc -c < "$PROBLEM_STMT") chars"
echo "Agent prompt:     $PROMPT_FILE"
echo "Venv:             ${VENV_DIR:-N/A}"
echo ""
echo "Next steps:"
echo "  1. Feed the agent prompt to Claude Code with the fractal skill"
echo "  2. Run scoring: ./score.sh ${INSTANCE_ID} --testbed-dir ${TESTBED_DIR}"
echo ""
