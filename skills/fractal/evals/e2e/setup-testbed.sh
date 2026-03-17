#!/usr/bin/env bash
# setup-testbed.sh — Prepare a FeatureBench testbed for e2e evaluation
#
# Usage: ./setup-testbed.sh <instance-id> [--testbed-dir <path>]
#
# This script:
#   1. Downloads the FeatureBench task from HuggingFace
#   2. Clones the target repo at the base commit
#   3. Strips the implementation (applies the gold patch to remove feature code)
#   4. Verifies F2P tests fail and P2P tests pass (sanity check)
#   5. Installs dependencies
#   6. Writes the full problem statement to a file for the agent
#
# FeatureBench patch format:
#   - base_commit: the commit AFTER the feature was merged (has feature + tests)
#   - patch: a reverse diff that REMOVES the implementation code
#   - test_patch: a reverse diff that REMOVES the test file(s)
#   We apply `patch` to strip the implementation, leaving tests in place.
#   We do NOT apply `test_patch` — the F2P tests must remain so they can
#   be used to verify the agent's re-implementation.
#
# Prerequisites:
#   - git, python3, pip, jq
#   - huggingface_hub: pip install huggingface_hub

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
EVALS_DIR="$(dirname "$SCRIPT_DIR")"

# --- Argument parsing ---
INSTANCE_ID="${1:?Usage: setup-testbed.sh <instance-id> [--testbed-dir <path>]}"
shift

TESTBED_DIR=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --testbed-dir) TESTBED_DIR="$2"; shift 2 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

# Default testbed location
if [[ -z "$TESTBED_DIR" ]]; then
  TESTBED_DIR="/tmp/featurebench-testbed/${INSTANCE_ID}"
fi

echo "=== FeatureBench Testbed Setup ==="
echo "Instance: $INSTANCE_ID"
echo "Testbed:  $TESTBED_DIR"
echo ""

# --- Step 1: Download task data from HuggingFace ---
echo "[1/6] Downloading task data from HuggingFace..."

TASK_DATA_DIR="${TESTBED_DIR}/.featurebench"
mkdir -p "$TASK_DATA_DIR"

python3 -c "
import json, sys
try:
    from huggingface_hub import hf_hub_download
except ImportError:
    print('ERROR: huggingface_hub not installed. Run: pip install huggingface_hub', file=sys.stderr)
    sys.exit(1)

from datasets import load_dataset

instance_id = '${INSTANCE_ID}'
task_dir = '${TASK_DATA_DIR}'

# Load the FeatureBench dataset
ds = load_dataset('LiberCoders/FeatureBench', split='lite')

# Find our instance
matches = [row for row in ds if row['instance_id'] == instance_id]
if not matches:
    # Try partial match
    matches = [row for row in ds if instance_id in row['instance_id']]

if not matches:
    print(f'ERROR: Instance {instance_id} not found in FeatureBench dataset', file=sys.stderr)
    print(f'Available instances ({len(ds)}):')
    for row in ds:
        print(f'  {row[\"instance_id\"]}')
    sys.exit(1)

task = matches[0]
print(f'Found: {task[\"instance_id\"]}')
print(f'  Repo: {task[\"repo\"]}')
print(f'  Base commit: {task[\"base_commit\"]}')
print(f'  Problem statement: {len(task[\"problem_statement\"])} chars')
print(f'  F2P tests: {len(task.get(\"FAIL_TO_PASS\", \"[]\"))} chars')
print(f'  P2P tests: {len(task.get(\"PASS_TO_PASS\", \"[]\"))} chars')

# Save task data
with open(f'{task_dir}/task.json', 'w') as f:
    # Convert to serializable dict
    task_dict = dict(task)
    json.dump(task_dict, f, indent=2, default=str)

# Save problem statement separately for easy consumption
with open(f'{task_dir}/problem_statement.txt', 'w') as f:
    f.write(task['problem_statement'])

# Save patches
if task.get('test_patch'):
    with open(f'{task_dir}/test_patch.diff', 'w') as f:
        f.write(task['test_patch'])

if task.get('patch'):
    with open(f'{task_dir}/patch.diff', 'w') as f:
        f.write(task['patch'])
    print(f'  Gold patch: {len(task[\"patch\"])} chars')

# Save test lists
for key in ['FAIL_TO_PASS', 'PASS_TO_PASS']:
    if task.get(key):
        with open(f'{task_dir}/{key.lower()}.json', 'w') as f:
            f.write(task[key] if isinstance(task[key], str) else json.dumps(task[key]))

# Save metadata for scripts
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
"

echo ""

# --- Step 2: Clone the target repo ---
echo "[2/6] Cloning target repo..."

META_FILE="${TASK_DATA_DIR}/meta.json"
REPO=$(python3 -c "import json; print(json.load(open('${META_FILE}'))['repo'])")
BASE_COMMIT=$(python3 -c "import json; print(json.load(open('${META_FILE}'))['base_commit'])")
ENV_COMMIT=$(python3 -c "import json; print(json.load(open('${META_FILE}'))['environment_setup_commit'])")

REPO_DIR="${TESTBED_DIR}/repo"

if [[ -d "$REPO_DIR/.git" ]]; then
  echo "  Repo already cloned, resetting..."
  cd "$REPO_DIR"
  git checkout -f "$BASE_COMMIT" 2>/dev/null || git fetch origin && git checkout -f "$BASE_COMMIT"
else
  echo "  Cloning https://github.com/${REPO}..."
  git clone --no-checkout "https://github.com/${REPO}.git" "$REPO_DIR"
  cd "$REPO_DIR"
  git checkout -f "$BASE_COMMIT"
fi

echo "  Checked out at ${BASE_COMMIT:0:12}"
echo ""

# --- Step 3: Strip implementation code ---
# FeatureBench stores patches in reverse: base_commit has the feature already,
# and `patch` removes it. We apply `patch` to create the "before" state where
# the feature code is missing but the tests remain.
echo "[3/6] Stripping implementation code (applying gold patch)..."

IMPL_PATCH="${TASK_DATA_DIR}/patch.diff"
if [[ -f "$IMPL_PATCH" ]] && [[ -s "$IMPL_PATCH" ]]; then
  cd "$REPO_DIR"
  if git apply --check "$IMPL_PATCH" 2>/dev/null; then
    git apply "$IMPL_PATCH"
    echo "  Implementation stripped ($(grep -c '^diff --git' "$IMPL_PATCH") files affected)."
  else
    echo "  WARNING: Patch doesn't apply cleanly, trying with --3way..."
    git apply --3way "$IMPL_PATCH" || {
      echo "  ERROR: Could not strip implementation code." >&2
      echo "  The gold patch failed to apply. F2P tests may pass trivially." >&2
      exit 1
    }
  fi
else
  echo "  ERROR: No gold patch (patch.diff) found. Cannot create testbed." >&2
  echo "  Without stripping the implementation, F2P tests pass trivially." >&2
  exit 1
fi

echo ""

# --- Step 4: Sanity check — verify F2P tests fail ---
echo "[4/6] Sanity check — verifying F2P tests fail without implementation..."

F2P_FILE="${TASK_DATA_DIR}/fail_to_pass.json"
if [[ -f "$F2P_FILE" ]]; then
  cd "$REPO_DIR"
  # Activate venv if already present (from a previous run)
  VENV_DIR="${TESTBED_DIR}/venv"
  if [[ -d "$VENV_DIR" ]]; then
    source "${VENV_DIR}/bin/activate"
  fi

  # Extract first F2P test entry for a quick sanity check
  FIRST_F2P=$(python3 -c "
import json
data = json.loads(open('${F2P_FILE}').read())
if isinstance(data, str):
    data = json.loads(data)
if isinstance(data, list):
    print(data[0])
else:
    print(data)
")

  if [[ -n "$FIRST_F2P" ]]; then
    echo "  Checking: $FIRST_F2P"
    if timeout 120 python3 -m pytest "$FIRST_F2P" --tb=line -q --no-header 2>&1 | tail -3; then
      # If pytest exits 0, the test passed — that's BAD
      if timeout 120 python3 -m pytest "$FIRST_F2P" --tb=no -q --no-header 2>/dev/null; then
        echo ""
        echo "  WARNING: F2P tests PASS without implementation!"
        echo "  The testbed may not be set up correctly."
        echo "  Expected: tests should FAIL before agent implements the feature."
      else
        echo "  OK — F2P tests fail as expected."
      fi
    else
      echo "  OK — F2P tests fail as expected."
    fi
  fi
else
  echo "  No F2P test list found, skipping sanity check."
fi

echo ""

# --- Step 5: Install dependencies ---
echo "[5/6] Installing dependencies..."

cd "$REPO_DIR"

# Detect project type and install
if [[ -f "setup.py" ]] || [[ -f "pyproject.toml" ]] || [[ -f "setup.cfg" ]]; then
  echo "  Python project detected."

  # Create a venv if it doesn't exist
  VENV_DIR="${TESTBED_DIR}/venv"
  if [[ ! -d "$VENV_DIR" ]]; then
    python3 -m venv "$VENV_DIR"
  fi
  source "${VENV_DIR}/bin/activate"

  # Install in editable mode
  pip install -e ".[dev]" 2>/dev/null || \
    pip install -e "." 2>/dev/null || \
    pip install -e ".[test]" 2>/dev/null || \
    echo "  WARNING: Could not install package. Tests may fail."

  # Also install pytest if not already present
  pip install pytest 2>/dev/null

  echo "  Dependencies installed in venv: $VENV_DIR"
else
  echo "  WARNING: Unknown project type. Skipping dependency install."
fi

echo ""

# --- Step 6: Write agent prompt ---
echo "[6/6] Generating agent prompt..."

PROMPT_FILE="${TASK_DATA_DIR}/agent_prompt.md"
PROBLEM_STMT="${TASK_DATA_DIR}/problem_statement.txt"

cat > "$PROMPT_FILE" << 'PROMPT_HEADER'
# FeatureBench E2E Evaluation — Fractal Decomposition

You have the fractal decomposition skill loaded.

## Instructions

1. Read the FULL problem statement below carefully — including all Interface Descriptions.
2. Use `/fractal decompose` with `output_mode=code` to implement the feature.
3. All code must be written to the testbed repo (paths are relative to the repo root).
4. After implementation, run the test suite to verify.

## Problem Statement

PROMPT_HEADER

cat "$PROBLEM_STMT" >> "$PROMPT_FILE"

cat >> "$PROMPT_FILE" << PROMPT_FOOTER

## Configuration

\`\`\`
Config: output_mode=code, max_depth=3, max_children=6, solve_mode=parallel
Repo root: ${REPO_DIR}
\`\`\`

Follow the full orchestration protocol from SKILL.md.
PROMPT_FOOTER

echo "  Agent prompt written to: $PROMPT_FILE"
echo ""

# --- Summary ---
echo "=== Setup Complete ==="
echo ""
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
