#!/usr/bin/env bash
# devbench.sh — Prepare a DevBench testbed for e2e evaluation
#
# Usage: ./devbench.sh <project-name> [--testbed-dir <path>] [--skill <name>]
#
# DevBench is fundamentally different from SWE-bench/FeatureBench:
#   - Instead of fixing bugs or implementing features in existing repos,
#     the agent builds an entire project from a Product Requirements Document (PRD).
#   - Scoring uses acceptance tests + LLM-judge evaluation.
#
# This script:
#   1. Clones the DevBench repo and extracts the target project
#   2. Prepares the PRD and acceptance test criteria
#   3. Creates a scaffold directory for the agent to build in
#   4. Writes the agent prompt (skill-specific)
#
# Prerequisites:
#   - git, python3

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# --- Argument parsing ---
PROJECT_NAME="${1:?Usage: devbench.sh <project-name> [--testbed-dir <path>] [--skill <name>]}"
shift

TESTBED_DIR=""
SKILL="fractal"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --testbed-dir) TESTBED_DIR="$2"; shift 2 ;;
    --skill) SKILL="$2"; shift 2 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

if [[ -z "$TESTBED_DIR" ]]; then
  TESTBED_DIR="/tmp/devbench-testbed/${PROJECT_NAME}"
fi

echo "=== DevBench Testbed Setup ==="
echo "Project:  $PROJECT_NAME"
echo "Skill:    $SKILL"
echo "Testbed:  $TESTBED_DIR"
echo ""

# --- Step 1: Get DevBench data ---
echo "[1/4] Fetching DevBench project data..."

DEVBENCH_CACHE="/tmp/devbench-repo"
TASK_DATA_DIR="${TESTBED_DIR}/.devbench"
mkdir -p "$TASK_DATA_DIR"

# Clone DevBench repo if not cached
if [[ ! -d "$DEVBENCH_CACHE/.git" ]]; then
  echo "  Cloning DevBench repository..."
  git clone --depth 1 "https://github.com/open-compass/DevBench.git" "$DEVBENCH_CACHE"
else
  echo "  Using cached DevBench repo."
fi

# Find the project directory
PROJECT_DIR=""
for lang_dir in "$DEVBENCH_CACHE"/benchmark/*/; do
  if [[ -d "${lang_dir}${PROJECT_NAME}" ]]; then
    PROJECT_DIR="${lang_dir}${PROJECT_NAME}"
    break
  fi
done

if [[ -z "$PROJECT_DIR" ]]; then
  echo "ERROR: Project '$PROJECT_NAME' not found in DevBench." >&2
  echo "Available projects:" >&2
  for lang_dir in "$DEVBENCH_CACHE"/benchmark/*/; do
    lang=$(basename "$lang_dir")
    echo "  [$lang]:" >&2
    for proj in "$lang_dir"*/; do
      echo "    $(basename "$proj")" >&2
    done
  done
  exit 1
fi

LANGUAGE=$(basename "$(dirname "$PROJECT_DIR")")
echo "  Found: $PROJECT_NAME (language: $LANGUAGE)"

# Copy project data to testbed
cp -r "$PROJECT_DIR" "$TASK_DATA_DIR/project_data"

echo ""

# --- Step 2: Extract PRD and test criteria ---
echo "[2/4] Extracting PRD and acceptance criteria..."

PRD_FILE=""
for candidate in \
  "$TASK_DATA_DIR/project_data/PRD.md" \
  "$TASK_DATA_DIR/project_data/docs/PRD.md" \
  "$TASK_DATA_DIR/project_data/prd.md"; do
  if [[ -f "$candidate" ]]; then
    PRD_FILE="$candidate"
    break
  fi
done

if [[ -z "$PRD_FILE" ]]; then
  PRD_FILE=$(find "$TASK_DATA_DIR/project_data" -maxdepth 2 -name "*.md" -print -quit 2>/dev/null || true)
fi

if [[ -n "$PRD_FILE" ]] && [[ -f "$PRD_FILE" ]]; then
  cp "$PRD_FILE" "$TASK_DATA_DIR/prd.md"
  echo "  PRD: $(wc -c < "$PRD_FILE") chars"
else
  echo "  WARNING: No PRD found. Agent will receive project description only."
  echo "# $PROJECT_NAME" > "$TASK_DATA_DIR/prd.md"
  echo "" >> "$TASK_DATA_DIR/prd.md"
  echo "Build the $PROJECT_NAME project in $LANGUAGE." >> "$TASK_DATA_DIR/prd.md"
fi

# Copy supplementary docs
for doc in UML_class.md UML_sequence.md Architecture.md architecture.md; do
  src="$TASK_DATA_DIR/project_data/$doc"
  if [[ -f "$src" ]]; then
    cp "$src" "$TASK_DATA_DIR/"
    echo "  Supplementary: $doc"
  fi
done

# Extract acceptance tests
ACCEPTANCE_DIR="$TASK_DATA_DIR/project_data/acceptance_tests"
if [[ -d "$ACCEPTANCE_DIR" ]]; then
  cp -r "$ACCEPTANCE_DIR" "$TASK_DATA_DIR/acceptance_tests"
  TEST_COUNT=$(find "$TASK_DATA_DIR/acceptance_tests" -name "*.py" -o -name "*.js" -o -name "*.sh" | wc -l)
  echo "  Acceptance tests: $TEST_COUNT files"
else
  mkdir -p "$TASK_DATA_DIR/acceptance_tests"
  echo "  No acceptance tests found."
fi

# Save metadata
python3 -c "
import json, os

meta = {
    'benchmark': 'devbench',
    'project_name': '${PROJECT_NAME}',
    'language': '${LANGUAGE}',
    'has_prd': os.path.exists('${TASK_DATA_DIR}/prd.md'),
    'has_architecture': os.path.exists('${TASK_DATA_DIR}/Architecture.md') or os.path.exists('${TASK_DATA_DIR}/architecture.md'),
    'has_uml_class': os.path.exists('${TASK_DATA_DIR}/UML_class.md'),
    'has_uml_sequence': os.path.exists('${TASK_DATA_DIR}/UML_sequence.md'),
    'acceptance_test_count': len([f for f in os.listdir('${TASK_DATA_DIR}/acceptance_tests') if f.endswith(('.py', '.js', '.sh'))]) if os.path.isdir('${TASK_DATA_DIR}/acceptance_tests') else 0,
}
with open('${TASK_DATA_DIR}/meta.json', 'w') as f:
    json.dump(meta, f, indent=2)
"

echo ""

# --- Step 3: Create build scaffold ---
echo "[3/4] Creating build scaffold..."

REPO_DIR="${TESTBED_DIR}/repo"
mkdir -p "$REPO_DIR"
cd "$REPO_DIR"

if [[ ! -d ".git" ]]; then
  git init
  git commit --allow-empty -m "Initial empty commit for DevBench: $PROJECT_NAME"
fi

REF_DIR="$TASK_DATA_DIR/project_data/ground_truth"
if [[ -d "$REF_DIR" ]]; then
  cd "$REF_DIR"
  find . -type d -exec mkdir -p "$REPO_DIR/{}" \; 2>/dev/null || true
  cd "$REPO_DIR"
  echo "  Directory scaffold created from reference structure."
else
  echo "  Empty scaffold (no reference structure available)."
fi

echo ""

# --- Step 4: Write agent prompt ---
echo "[4/4] Generating agent prompt (skill: $SKILL)..."

PROMPT_FILE="${TASK_DATA_DIR}/agent_prompt.md"

# Skill-specific prompt header
case "$SKILL" in
  fractal)
    cat > "$PROMPT_FILE" << 'PROMPT_HEADER'
# DevBench E2E Evaluation — Fractal Decomposition

You have the fractal decomposition skill loaded.

## Instructions

You are building a complete project from a Product Requirements Document (PRD).
This is NOT a bug fix or feature addition — you are building from scratch.

1. Read the PRD below carefully.
2. Use `/fractal decompose` with `output_mode=code` to build the project.
   - The decomposer should split the PRD into orthogonal modules/components.
   - Each SOLVE agent builds one module in an isolated worktree.
   - REUNIFY integrates all modules into a working project.
3. All code must be written to the repo directory.
4. The project must compile/run and pass acceptance tests.

## Product Requirements Document

PROMPT_HEADER
    ;;
  factory)
    cat > "$PROMPT_FILE" << 'PROMPT_HEADER'
# DevBench E2E Evaluation — Factory Skill

You have the factory skill loaded.

## Instructions

You are building a complete project from a Product Requirements Document (PRD).

1. Read the PRD below carefully.
2. Create a spec from the PRD, then run `/factory run` on it.
3. All code must be written to the repo directory.
4. The project must compile/run and pass acceptance tests.

## Product Requirements Document

PROMPT_HEADER
    ;;
  baseline)
    cat > "$PROMPT_FILE" << 'PROMPT_HEADER'
# DevBench E2E Evaluation — Baseline (No Skill)

## Instructions

You are building a complete project from a Product Requirements Document (PRD).

1. Read the PRD below carefully.
2. Implement the project directly.
3. The project must compile/run and pass acceptance tests.

## Product Requirements Document

PROMPT_HEADER
    ;;
  *)
    cat > "$PROMPT_FILE" << PROMPT_HEADER
# DevBench E2E Evaluation — ${SKILL}

## Instructions

Build the project described in the PRD below.

## Product Requirements Document

PROMPT_HEADER
    ;;
esac

cat "$TASK_DATA_DIR/prd.md" >> "$PROMPT_FILE"

# Append supplementary docs
for doc in Architecture.md architecture.md UML_class.md UML_sequence.md; do
  if [[ -f "$TASK_DATA_DIR/$doc" ]]; then
    echo "" >> "$PROMPT_FILE"
    echo "## $(echo "$doc" | sed 's/.md$//' | sed 's/_/ /g')" >> "$PROMPT_FILE"
    echo "" >> "$PROMPT_FILE"
    cat "$TASK_DATA_DIR/$doc" >> "$PROMPT_FILE"
  fi
done

# Skill-specific config footer
case "$SKILL" in
  fractal)
    cat >> "$PROMPT_FILE" << PROMPT_FOOTER

## Configuration

\`\`\`
Config: output_mode=code, max_depth=3, max_children=6, solve_mode=parallel
Language: ${LANGUAGE}
Repo root: ${REPO_DIR}
Benchmark: DevBench
\`\`\`

Follow the full orchestration protocol from SKILL.md.
PROMPT_FOOTER
    ;;
  factory)
    cat >> "$PROMPT_FILE" << PROMPT_FOOTER

## Configuration

\`\`\`
Language: ${LANGUAGE}
Repo root: ${REPO_DIR}
Benchmark: DevBench
\`\`\`

Follow the full orchestration protocol from SKILL.md.
PROMPT_FOOTER
    ;;
  *)
    cat >> "$PROMPT_FILE" << PROMPT_FOOTER

## Configuration

\`\`\`
Language: ${LANGUAGE}
Repo root: ${REPO_DIR}
Benchmark: DevBench
\`\`\`
PROMPT_FOOTER
    ;;
esac

echo "  Agent prompt written to: $PROMPT_FILE"
echo ""

# --- Summary ---
echo "=== Setup Complete ==="
echo ""
echo "Benchmark:        DevBench"
echo "Skill:            $SKILL"
echo "Project:          $PROJECT_NAME ($LANGUAGE)"
echo "Testbed:          $TESTBED_DIR"
echo "Repo:             $REPO_DIR"
echo "PRD:              $(wc -c < "$TASK_DATA_DIR/prd.md") chars"
echo "Agent prompt:     $PROMPT_FILE"
echo ""
echo "Next steps:"
echo "  1. Feed the agent prompt to Claude Code with the $SKILL skill"
echo "  2. Run scoring: $(dirname "$SCRIPT_DIR")/score-devbench.sh ${PROJECT_NAME} --testbed-dir ${TESTBED_DIR}"
echo ""
