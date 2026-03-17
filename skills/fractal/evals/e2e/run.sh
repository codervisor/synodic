#!/usr/bin/env bash
# run.sh — End-to-end FeatureBench evaluation for the fractal decomposition skill
#
# Usage: ./run.sh <instance-id-or-alias> [options]
#
# Aliases:
#   mlflow-tracing  → mlflow__mlflow.93dab383.test_trace.17fde8b0.lv1
#   sympy-nullspace → sympy__sympy.c1097516.test_nullspace.f14fc970.lv1
#   seaborn-regr    → mwaskom__seaborn.7001ebe7.test_regression.ce8c62e2.lv1
#
# Options:
#   --testbed-dir <path>   Override testbed location (default: /tmp/featurebench-testbed/<id>)
#   --skip-setup           Skip testbed setup (assume already done)
#   --skip-agent           Skip agent invocation (just score existing code)
#   --agent-cmd <cmd>      Command to invoke the agent (default: claude)
#   --output <path>        Score report output path
#   --dry-run              Print the agent prompt without running anything
#
# Example:
#   ./run.sh mlflow-tracing                          # Full e2e run
#   ./run.sh mlflow-tracing --skip-setup             # Re-run agent + score
#   ./run.sh mlflow-tracing --skip-agent             # Just score existing code
#   ./run.sh mlflow-tracing --dry-run                # Print the prompt

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# --- Alias resolution ---
resolve_alias() {
  case "$1" in
    mlflow-tracing|mlflow)   echo "mlflow__mlflow.93dab383.test_trace.17fde8b0.lv1" ;;
    sympy-nullspace|sympy)   echo "sympy__sympy.c1097516.test_nullspace.f14fc970.lv1" ;;
    seaborn-regr|seaborn)    echo "mwaskom__seaborn.7001ebe7.test_regression.ce8c62e2.lv1" ;;
    *)                       echo "$1" ;;  # Treat as literal instance ID
  esac
}

# --- Argument parsing ---
RAW_ID="${1:?Usage: run.sh <instance-id-or-alias> [options]}"
shift

INSTANCE_ID=$(resolve_alias "$RAW_ID")
TESTBED_DIR=""
SKIP_SETUP=false
SKIP_AGENT=false
AGENT_CMD="claude"
OUTPUT_FILE=""
DRY_RUN=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --testbed-dir) TESTBED_DIR="$2"; shift 2 ;;
    --skip-setup) SKIP_SETUP=true; shift ;;
    --skip-agent) SKIP_AGENT=true; shift ;;
    --agent-cmd) AGENT_CMD="$2"; shift 2 ;;
    --output) OUTPUT_FILE="$2"; shift 2 ;;
    --dry-run) DRY_RUN=true; shift ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

if [[ -z "$TESTBED_DIR" ]]; then
  TESTBED_DIR="/tmp/featurebench-testbed/${INSTANCE_ID}"
fi

TASK_DIR="${TESTBED_DIR}/.featurebench"
REPO_DIR="${TESTBED_DIR}/repo"

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║        FeatureBench E2E — Fractal Decomposition            ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo "Instance: $INSTANCE_ID"
echo "Testbed:  $TESTBED_DIR"
echo ""

# --- Phase 1: Setup ---
if [[ "$SKIP_SETUP" == "false" ]]; then
  echo "━━━ Phase 1: Testbed Setup ━━━"
  "$SCRIPT_DIR/setup-testbed.sh" "$INSTANCE_ID" --testbed-dir "$TESTBED_DIR"
else
  echo "━━━ Phase 1: Testbed Setup (skipped) ━━━"
  if [[ ! -d "$TASK_DIR" ]]; then
    echo "ERROR: Testbed not found at $TESTBED_DIR. Run without --skip-setup first." >&2
    exit 1
  fi
fi

echo ""

# --- Phase 2: Agent Invocation ---
PROMPT_FILE="${TASK_DIR}/agent_prompt.md"

if [[ "$DRY_RUN" == "true" ]]; then
  echo "━━━ Phase 2: Agent Prompt (dry run) ━━━"
  echo ""
  echo "--- BEGIN PROMPT ---"
  cat "$PROMPT_FILE"
  echo "--- END PROMPT ---"
  echo ""
  echo "To run manually:"
  echo "  cd $REPO_DIR"
  echo "  $AGENT_CMD --print \"$(head -1 "$PROMPT_FILE")\""
  exit 0
fi

if [[ "$SKIP_AGENT" == "false" ]]; then
  echo "━━━ Phase 2: Agent Invocation ━━━"
  echo ""
  echo "Starting agent in testbed repo..."
  echo "  Agent command: $AGENT_CMD"
  echo "  Working dir:   $REPO_DIR"
  echo "  Prompt:        $PROMPT_FILE"
  echo ""

  # Record start time
  START_TIME=$(date +%s)

  # Run the agent with the prompt
  # The agent needs to:
  #   1. Read the full problem statement
  #   2. Run /fractal decompose with output_mode=code
  #   3. Write code to the repo
  cd "$REPO_DIR"

  # Use --print to pass the prompt, or pipe it
  if command -v "$AGENT_CMD" &>/dev/null; then
    "$AGENT_CMD" --print "$(cat "$PROMPT_FILE")" \
      2>&1 | tee "${TASK_DIR}/agent_output.log" || true
  else
    echo "WARNING: Agent command '$AGENT_CMD' not found."
    echo "Please run the agent manually:"
    echo ""
    echo "  cd $REPO_DIR"
    echo "  cat ${PROMPT_FILE} | claude"
    echo ""
    echo "Then re-run with --skip-agent to score."
    exit 1
  fi

  END_TIME=$(date +%s)
  DURATION=$((END_TIME - START_TIME))
  echo ""
  echo "Agent completed in ${DURATION}s"
  echo ""
else
  echo "━━━ Phase 2: Agent Invocation (skipped) ━━━"
fi

# --- Phase 3: Scoring ---
echo "━━━ Phase 3: Scoring ━━━"
echo ""

SCORE_ARGS=("$INSTANCE_ID" --testbed-dir "$TESTBED_DIR")
if [[ -n "$OUTPUT_FILE" ]]; then
  SCORE_ARGS+=(--output "$OUTPUT_FILE")
fi

"$SCRIPT_DIR/score.sh" "${SCORE_ARGS[@]}"

echo ""
echo "━━━ Done ━━━"
