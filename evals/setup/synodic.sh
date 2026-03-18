#!/usr/bin/env bash
# synodic.sh — Prepare a Synodic dogfood testbed for e2e evaluation
#
# Usage: ./synodic.sh <instance-alias> [--testbed-dir <path>] [--skill <name>]
#
# A synodic dogfood instance:
#   - Clones codervisor/synodic at a base commit
#   - Uses a spec from the current Synodic install as the problem statement
#   - Scores by running cargo test in cli/
#
# Instance metadata is in evals/tasks/synodic/<alias>.meta.json
#
# Prerequisites:
#   - git, cargo, rust

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# --- Argument parsing ---
INSTANCE_ALIAS="${1:?Usage: synodic.sh <instance-alias> [--testbed-dir <path>] [--skill <name>]}"
shift

TESTBED_DIR=""
SKILL="factory"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --testbed-dir) TESTBED_DIR="$2"; shift 2 ;;
    --skill) SKILL="$2"; shift 2 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

if [[ -z "$TESTBED_DIR" ]]; then
  TESTBED_DIR="/tmp/synodic-testbed/${INSTANCE_ALIAS}"
fi

echo "=== Synodic Dogfood Testbed Setup ==="
echo "Instance: $INSTANCE_ALIAS"
echo "Skill:    $SKILL"
echo "Testbed:  $TESTBED_DIR"
echo ""

# --- Step 1: Read instance metadata ---
echo "[1/4] Reading instance metadata..."

TASK_DATA_DIR="${TESTBED_DIR}/.synodic"
mkdir -p "$TASK_DATA_DIR"

META_FILE="${REPO_ROOT}/evals/tasks/synodic/${INSTANCE_ALIAS}.meta.json"
if [[ ! -f "$META_FILE" ]]; then
  echo "ERROR: Instance metadata not found: $META_FILE" >&2
  echo "Available instances:" >&2
  for f in "${REPO_ROOT}/evals/tasks/synodic/"*.meta.json; do
    echo "  $(basename "$f" .meta.json)" >&2
  done
  exit 1
fi

REPO=$(python3 -c "import json; print(json.load(open('${META_FILE}'))['repo'])")
BASE_COMMIT=$(python3 -c "import json; print(json.load(open('${META_FILE}'))['base_commit'])")
SPEC_PATH=$(python3 -c "import json; print(json.load(open('${META_FILE}'))['spec_path'])")
SCORE_DIR=$(python3 -c "import json; print(json.load(open('${META_FILE}'))['score_dir'])")

echo "  Repo:        $REPO"
echo "  Base commit: ${BASE_COMMIT:0:12}"
echo "  Spec:        $SPEC_PATH"
echo "  Score dir:   $SCORE_DIR"

# Copy metadata to testbed
cp "$META_FILE" "$TASK_DATA_DIR/meta.json"

echo ""

# --- Step 2: Clone the Synodic repo at base commit ---
echo "[2/4] Cloning target repo..."

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

# --- Step 3: Build dependencies ---
echo "[3/4] Verifying build..."

cd "${REPO_DIR}/${SCORE_DIR}"

if cargo build --quiet 2>/dev/null; then
  echo "  Build: OK"
else
  echo "  WARNING: Initial build failed. The agent will need to fix this."
fi

echo ""

# --- Step 4: Write agent prompt ---
echo "[4/4] Generating agent prompt (skill: $SKILL)..."

PROMPT_FILE="${TASK_DATA_DIR}/agent_prompt.md"
SPEC_ABS="${REPO_ROOT}/${SPEC_PATH}"

if [[ ! -f "$SPEC_ABS" ]]; then
  echo "ERROR: Spec file not found: $SPEC_ABS" >&2
  exit 1
fi

SPEC_CONTENT=$(cat "$SPEC_ABS")

# Save problem statement (the spec content)
cp "$SPEC_ABS" "$TASK_DATA_DIR/problem_statement.txt"

# Skill-specific prompt header
case "$SKILL" in
  factory)
    cat > "$PROMPT_FILE" << 'PROMPT_HEADER'
# Synodic Dogfood Evaluation — Factory Skill

You have the factory skill loaded.

## Instructions

1. Read the spec below carefully.
2. Analyze the codebase to understand what needs to be implemented.
3. Run `/factory run` with the spec to implement the changes.
4. After implementation, run `cargo test` in the `cli/` directory to verify.

## Spec

PROMPT_HEADER
    ;;
  fractal)
    cat > "$PROMPT_FILE" << 'PROMPT_HEADER'
# Synodic Dogfood Evaluation — Fractal Decomposition

You have the fractal decomposition skill loaded.

## Instructions

1. Read the spec below carefully.
2. Analyze the codebase to understand what needs to be implemented.
3. Use `/fractal decompose` with `output_mode=code` to implement the spec.
4. After implementation, run `cargo test` in the `cli/` directory to verify.

## Spec

PROMPT_HEADER
    ;;
  baseline)
    cat > "$PROMPT_FILE" << 'PROMPT_HEADER'
# Synodic Dogfood Evaluation — Baseline (No Skill)

## Instructions

1. Read the spec below carefully.
2. Analyze the codebase to understand what needs to be implemented.
3. Implement the spec directly.
4. After implementation, run `cargo test` in the `cli/` directory to verify.

## Spec

PROMPT_HEADER
    ;;
  *)
    cat > "$PROMPT_FILE" << PROMPT_HEADER
# Synodic Dogfood Evaluation — ${SKILL}

## Instructions

1. Read the spec below carefully.
2. Implement the spec in the Synodic codebase.
3. Run \`cargo test\` in \`cli/\` to verify.

## Spec

PROMPT_HEADER
    ;;
esac

cat "$SPEC_ABS" >> "$PROMPT_FILE"

# Config footer
cat >> "$PROMPT_FILE" << PROMPT_FOOTER

## Configuration

\`\`\`
Repo root:  ${REPO_DIR}
Score dir:  ${REPO_DIR}/${SCORE_DIR}
Score cmd:  cargo test
Benchmark:  synodic (dogfood)
\`\`\`

Follow the full orchestration protocol from SKILL.md.
PROMPT_FOOTER

echo "  Agent prompt written to: $PROMPT_FILE"
echo ""

# --- Summary ---
echo "=== Setup Complete ==="
echo ""
echo "Benchmark:    Synodic (dogfood)"
echo "Skill:        $SKILL"
echo "Instance:     $INSTANCE_ALIAS"
echo "Testbed:      $TESTBED_DIR"
echo "Repo:         $REPO_DIR"
echo "Score dir:    ${REPO_DIR}/${SCORE_DIR}"
echo "Spec:         $SPEC_ABS"
echo "Agent prompt: $PROMPT_FILE"
echo ""
echo "Next steps:"
echo "  1. Feed the agent prompt to Claude Code with the $SKILL skill"
echo "  2. Run scoring: $(dirname "$SCRIPT_DIR")/score-synodic.sh ${INSTANCE_ALIAS} --testbed-dir ${TESTBED_DIR}"
echo ""
