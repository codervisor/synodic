#!/usr/bin/env bash
# run.sh — Unified e2e evaluation runner for Synodic skills
#
# Runs any benchmark task against any skill (fractal, factory, or plain baseline).
#
# Usage: ./run.sh <benchmark>:<alias-or-id> [options]
#
# FeatureBench aliases:
#   fb:mlflow-tracing   → mlflow__mlflow.93dab383.test_trace.17fde8b0.lv1
#   fb:sympy-nullspace  → sympy__sympy.c1097516.test_nullspace.f14fc970.lv1
#   fb:seaborn-regr     → mwaskom__seaborn.7001ebe7.test_regression.ce8c62e2.lv1
#
# SWE-bench aliases:
#   swe:django-16379    → django__django-16379
#   swe:astropy-14995   → astropy__astropy-14995
#   swe:<instance-id>   → literal instance ID
#
# DevBench aliases:
#   dev:TextCNN         → TextCNN project
#   dev:<project-name>  → literal project name
#
# Legacy aliases (backward-compatible):
#   mlflow-tracing      → fb:mlflow-tracing
#   sympy-nullspace     → fb:sympy-nullspace
#   seaborn-regr        → fb:seaborn-regr
#
# Options:
#   --skill <name>         Skill to invoke: fractal, factory, baseline (default: fractal)
#   --testbed-dir <path>   Override testbed location
#   --skip-setup           Skip testbed setup (assume already done)
#   --skip-agent           Skip agent invocation (just score existing code)
#   --agent-cmd <cmd>      Command to invoke the agent (default: claude)
#   --output <path>        Score report output path
#   --dry-run              Print the agent prompt without running anything
#   --split <split>        SWE-bench split: verified, lite, pro (default: verified)
#
# Examples:
#   ./run.sh fb:mlflow-tracing                        # FeatureBench e2e (fractal)
#   ./run.sh fb:seaborn-regr --skill factory          # FeatureBench e2e (factory)
#   ./run.sh swe:django__django-16379 --split pro     # SWE-bench Pro
#   ./run.sh dev:TextCNN                              # DevBench e2e
#   ./run.sh fb:mlflow-tracing --skill baseline       # Plain agent (no skill)
#   ./run.sh fb:mlflow-tracing --dry-run              # Print prompt only

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# --- Parse benchmark prefix and resolve aliases ---

resolve_target() {
  local raw="$1"
  local benchmark=""
  local instance=""

  # Check for benchmark prefix (fb:, swe:, dev:)
  if [[ "$raw" == fb:* ]]; then
    benchmark="featurebench"
    raw="${raw#fb:}"
  elif [[ "$raw" == swe:* ]]; then
    benchmark="swebench"
    raw="${raw#swe:}"
  elif [[ "$raw" == dev:* ]]; then
    benchmark="devbench"
    raw="${raw#dev:}"
  fi

  # Resolve aliases
  case "$raw" in
    # FeatureBench aliases
    mlflow-tracing|mlflow)
      benchmark="${benchmark:-featurebench}"
      instance="mlflow__mlflow.93dab383.test_trace.17fde8b0.lv1"
      ;;
    sympy-nullspace|sympy)
      benchmark="${benchmark:-featurebench}"
      instance="sympy__sympy.c1097516.test_nullspace.f14fc970.lv1"
      ;;
    seaborn-regr|seaborn)
      benchmark="${benchmark:-featurebench}"
      instance="mwaskom__seaborn.7001ebe7.test_regression.ce8c62e2.lv1"
      ;;
    # SWE-bench Verified aliases
    django-10097)
      benchmark="${benchmark:-swebench}"
      instance="django__django-10097"
      ;;
    # SWE-bench Pro aliases
    qutebrowser-f91ace)
      benchmark="${benchmark:-swebench}"
      instance="instance_qutebrowser__qutebrowser-f91ace96223cac8161c16dd061907e138fe85111-v059c6fdc75567943479b23ebca7c07b5e9a7f34c"
      ;;
    ansible-f327e6)
      benchmark="${benchmark:-swebench}"
      instance="instance_ansible__ansible-f327e65d11bb905ed9f15996024f857a95592629-vba6da65a0f3baefda7a058ebbd0a8dcafb8512f5"
      ;;
    teleport-3fa690)
      benchmark="${benchmark:-swebench}"
      instance="instance_gravitational__teleport-3fa6904377c006497169945428e8197158667910-v626ec2a48416b10a88641359a169d99e935ff037"
      ;;
    *)
      instance="$raw"
      ;;
  esac

  # Auto-detect benchmark from instance ID format if not specified
  if [[ -z "$benchmark" ]]; then
    if [[ "$instance" == instance_* ]]; then
      # SWE-bench Pro: instance_org__repo-commit-version
      benchmark="swebench"
    elif [[ "$instance" == *"."*"."*"."*"."* ]]; then
      # FeatureBench: org__repo.commit.test_module.hash.level
      benchmark="featurebench"
    elif [[ "$instance" == *"__"*"-"* ]]; then
      # SWE-bench Verified/Lite: org__repo-number
      benchmark="swebench"
    else
      # Assume DevBench project name
      benchmark="devbench"
    fi
  fi

  echo "$benchmark $instance"
}

# --- Argument parsing ---
RAW_TARGET="${1:?Usage: run.sh <benchmark>:<alias-or-id> [options]}"
shift

read -r BENCHMARK INSTANCE_ID <<< "$(resolve_target "$RAW_TARGET")"

SKILL="fractal"
TESTBED_DIR=""
SKIP_SETUP=false
SKIP_AGENT=false
AGENT_CMD="claude"
OUTPUT_FILE=""
DRY_RUN=false
SWE_SPLIT="verified"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --skill) SKILL="$2"; shift 2 ;;
    --testbed-dir) TESTBED_DIR="$2"; shift 2 ;;
    --skip-setup) SKIP_SETUP=true; shift ;;
    --skip-agent) SKIP_AGENT=true; shift ;;
    --agent-cmd) AGENT_CMD="$2"; shift 2 ;;
    --output) OUTPUT_FILE="$2"; shift 2 ;;
    --dry-run) DRY_RUN=true; shift ;;
    --split) SWE_SPLIT="$2"; shift 2 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

# Default testbed directory per benchmark
if [[ -z "$TESTBED_DIR" ]]; then
  case "$BENCHMARK" in
    featurebench) TESTBED_DIR="/tmp/featurebench-testbed/${INSTANCE_ID}" ;;
    swebench)     TESTBED_DIR="/tmp/swebench-testbed/${INSTANCE_ID}" ;;
    devbench)     TESTBED_DIR="/tmp/devbench-testbed/${INSTANCE_ID}" ;;
  esac
fi

# Benchmark metadata directory
case "$BENCHMARK" in
  featurebench) TASK_DIR="${TESTBED_DIR}/.featurebench" ;;
  swebench)     TASK_DIR="${TESTBED_DIR}/.swebench" ;;
  devbench)     TASK_DIR="${TESTBED_DIR}/.devbench" ;;
esac
REPO_DIR="${TESTBED_DIR}/repo"

# --- Header ---
BENCH_LABEL=""
case "$BENCHMARK" in
  featurebench) BENCH_LABEL="FeatureBench" ;;
  swebench)     BENCH_LABEL="SWE-bench (${SWE_SPLIT})" ;;
  devbench)     BENCH_LABEL="DevBench" ;;
esac

SKILL_LABEL=""
case "$SKILL" in
  fractal)  SKILL_LABEL="Fractal Decomposition" ;;
  factory)  SKILL_LABEL="Factory (BUILD → INSPECT)" ;;
  baseline) SKILL_LABEL="Baseline (no skill)" ;;
  *)        SKILL_LABEL="$SKILL" ;;
esac

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║          E2E Eval — Synodic Harness                        ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo "Benchmark: $BENCH_LABEL"
echo "Skill:     $SKILL_LABEL"
echo "Instance:  $INSTANCE_ID"
echo "Testbed:   $TESTBED_DIR"
echo ""

# --- Phase 1: Setup ---
if [[ "$SKIP_SETUP" == "false" ]]; then
  echo "━━━ Phase 1: Testbed Setup ━━━"
  case "$BENCHMARK" in
    featurebench)
      "$SCRIPT_DIR/setup/featurebench.sh" "$INSTANCE_ID" --testbed-dir "$TESTBED_DIR" --skill "$SKILL"
      ;;
    swebench)
      "$SCRIPT_DIR/setup/swebench.sh" "$INSTANCE_ID" --testbed-dir "$TESTBED_DIR" --split "$SWE_SPLIT" --skill "$SKILL"
      ;;
    devbench)
      "$SCRIPT_DIR/setup/devbench.sh" "$INSTANCE_ID" --testbed-dir "$TESTBED_DIR" --skill "$SKILL"
      ;;
  esac
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
  echo "--- BEGIN PROMPT ($(wc -c < "$PROMPT_FILE") chars) ---"
  cat "$PROMPT_FILE"
  echo "--- END PROMPT ---"
  echo ""
  echo "To run manually:"
  echo "  cd $REPO_DIR"
  echo "  cat ${PROMPT_FILE} | $AGENT_CMD --print -"
  exit 0
fi

if [[ "$SKIP_AGENT" == "false" ]]; then
  echo "━━━ Phase 2: Agent Invocation ━━━"
  echo ""
  echo "Starting agent in testbed repo..."
  echo "  Agent command: $AGENT_CMD"
  echo "  Skill:         $SKILL_LABEL"
  echo "  Working dir:   $REPO_DIR"
  echo "  Prompt:        $PROMPT_FILE ($(wc -c < "$PROMPT_FILE") chars)"
  echo ""

  START_TIME=$(date +%s)

  cd "$REPO_DIR"
  if command -v "$AGENT_CMD" &>/dev/null; then
    cat "$PROMPT_FILE" | "$AGENT_CMD" --print \
      --allowedTools "Edit Write Bash Read Glob Grep Agent" \
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

echo ""

# --- Phase 3: Scoring ---
echo "━━━ Phase 3: Scoring ━━━"
echo ""

case "$BENCHMARK" in
  featurebench|swebench)
    # Both use the same F2P + P2P pytest scoring
    SCORE_ARGS=("$INSTANCE_ID" --testbed-dir "$TESTBED_DIR")
    if [[ -n "$OUTPUT_FILE" ]]; then
      SCORE_ARGS+=(--output "$OUTPUT_FILE")
    fi
    "$SCRIPT_DIR/score.sh" "${SCORE_ARGS[@]}"
    ;;
  devbench)
    SCORE_ARGS=("$INSTANCE_ID" --testbed-dir "$TESTBED_DIR")
    if [[ -n "$OUTPUT_FILE" ]]; then
      SCORE_ARGS+=(--output "$OUTPUT_FILE")
    fi
    "$SCRIPT_DIR/score-devbench.sh" "${SCORE_ARGS[@]}"
    ;;
esac

echo ""
echo "━━━ Done ━━━"
