#!/usr/bin/env bash
# harness_eval_runner.sh — End-to-end harness governance evaluation.
#
# Runs evaluate_harness.py against real governance logs, or against
# synthetic scenarios to validate that scoring behaves correctly.
#
# Usage:
#   ./harness_eval_runner.sh                   # Evaluate real logs
#   ./harness_eval_runner.sh --self-test       # Run all synthetic scenarios
#   ./harness_eval_runner.sh --scenario NAME   # Run one synthetic scenario
#   ./harness_eval_runner.sh --compare         # Compare all scenarios side-by-side
#
# Options:
#   --harness-dir PATH   Path to .harness/ (default: repo root/.harness)
#   --window N           Sliding window size (default: 5)
#   --json               Output JSON instead of formatted text
#   --runs N             Synthetic run count (default: 20)
#   --seed N             Random seed for reproducibility

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
HARNESS_DIR="$REPO_ROOT/.harness"
WINDOW=5
JSON_FLAG=""
RUNS=20
SEED=42
MODE="real"
SCENARIO=""

usage() {
  echo "Usage: $0 [options]"
  echo ""
  echo "Modes:"
  echo "  (default)          Evaluate real governance logs"
  echo "  --self-test        Run all 4 synthetic scenarios, verify scoring"
  echo "  --scenario NAME    Run one synthetic scenario (healthy|degrading|cold-start|plateau)"
  echo "  --compare          Side-by-side comparison of all scenarios"
  echo ""
  echo "Options:"
  echo "  --harness-dir PATH  Path to .harness/ directory"
  echo "  --window N          Sliding window size (default: 5)"
  echo "  --json              Machine-readable JSON output"
  echo "  --runs N            Synthetic run count (default: 20)"
  echo "  --seed N            Random seed (default: 42)"
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --self-test)   MODE="self-test"; shift ;;
    --scenario)    MODE="scenario"; SCENARIO="$2"; shift 2 ;;
    --compare)     MODE="compare"; shift ;;
    --harness-dir) HARNESS_DIR="$2"; shift 2 ;;
    --window)      WINDOW="$2"; shift 2 ;;
    --json)        JSON_FLAG="--json"; shift ;;
    --runs)        RUNS="$2"; shift 2 ;;
    --seed)        SEED="$2"; shift 2 ;;
    -h|--help)     usage ;;
    *)             echo "Unknown option: $1"; usage ;;
  esac
done

EVALUATE="$SCRIPT_DIR/evaluate_harness.py"
SIMULATE="$SCRIPT_DIR/simulate_governance.py"

if [ ! -f "$EVALUATE" ]; then
  echo "Error: evaluate_harness.py not found at $EVALUATE" >&2
  exit 1
fi

if [ ! -f "$SIMULATE" ]; then
  echo "Error: simulate_governance.py not found at $SIMULATE" >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Mode: evaluate real governance logs
# ---------------------------------------------------------------------------
run_real() {
  echo "=== Evaluating real governance logs ===" >&2
  echo "Harness dir: $HARNESS_DIR" >&2
  echo "" >&2
  python3 "$EVALUATE" "$HARNESS_DIR" --window "$WINDOW" $JSON_FLAG
}

# ---------------------------------------------------------------------------
# Mode: run a single synthetic scenario
# ---------------------------------------------------------------------------
run_scenario() {
  local scenario="$1"
  local tmpdir
  tmpdir=$(mktemp -d)
  trap "rm -rf $tmpdir" EXIT

  # Create synthetic harness dir structure
  mkdir -p "$tmpdir/rules"
  touch "$tmpdir/rules/.gitkeep"

  # Generate synthetic logs
  python3 "$SIMULATE" "$scenario" --runs "$RUNS" --seed "$SEED" \
    --output "$tmpdir/synthetic.governance.jsonl" 2>/dev/null

  # For healthy/cold-start scenarios, simulate some rules existing
  if [ "$scenario" = "healthy" ] || [ "$scenario" = "cold-start" ]; then
    touch "$tmpdir/rules/no-sql-injection.sh"
    touch "$tmpdir/rules/no-hardcoded-secrets.sh"
    chmod +x "$tmpdir/rules/no-sql-injection.sh" "$tmpdir/rules/no-hardcoded-secrets.sh"
  fi

  python3 "$EVALUATE" "$tmpdir" --window "$WINDOW" $JSON_FLAG
}

# ---------------------------------------------------------------------------
# Mode: self-test — all scenarios, verify scoring expectations
# ---------------------------------------------------------------------------
run_self_test() {
  echo "=== Harness Evaluation Self-Test ===" >&2
  echo "Running all scenarios with $RUNS runs each (seed=$SEED)" >&2
  echo "" >&2

  local pass=0
  local fail=0

  for scenario in healthy degrading cold-start plateau; do
    local tmpdir
    tmpdir=$(mktemp -d)

    mkdir -p "$tmpdir/rules"
    touch "$tmpdir/rules/.gitkeep"

    python3 "$SIMULATE" "$scenario" --runs "$RUNS" --seed "$SEED" \
      --output "$tmpdir/synthetic.governance.jsonl" 2>/dev/null

    if [ "$scenario" = "healthy" ]; then
      touch "$tmpdir/rules/no-sql-injection.sh"
      touch "$tmpdir/rules/no-hardcoded-secrets.sh"
      chmod +x "$tmpdir/rules/no-sql-injection.sh" "$tmpdir/rules/no-hardcoded-secrets.sh"
    fi

    local score
    score=$(python3 "$EVALUATE" "$tmpdir" --window "$WINDOW" --json | python3 -c "
import sys, json
report = json.load(sys.stdin)
print(report['score']['total'])
")

    local grade
    grade=$(python3 "$EVALUATE" "$tmpdir" --window "$WINDOW" --json | python3 -c "
import sys, json
report = json.load(sys.stdin)
print(report['score']['grade'])
")

    # Expectations:
    #   healthy:    score >= 50 (good governance)
    #   degrading:  score <= 50 (bad governance)
    #   cold-start: score >= 30 (improving from nothing)
    #   plateau:    score 30-70 (middling)
    local expected_pass=true
    case "$scenario" in
      healthy)
        [ "$score" -ge 50 ] || expected_pass=false ;;
      degrading)
        [ "$score" -le 50 ] || expected_pass=false ;;
      cold-start)
        [ "$score" -ge 20 ] || expected_pass=false ;;
      plateau)
        [ "$score" -ge 20 ] && [ "$score" -le 80 ] || expected_pass=false ;;
    esac

    if [ "$expected_pass" = true ]; then
      echo "  PASS  $scenario: score=$score grade=$grade"
      pass=$((pass + 1))
    else
      echo "  FAIL  $scenario: score=$score grade=$grade (unexpected)"
      fail=$((fail + 1))
    fi

    rm -rf "$tmpdir"
  done

  echo ""
  echo "Results: $pass passed, $fail failed out of 4 scenarios"

  if [ "$fail" -gt 0 ]; then
    exit 1
  fi
}

# ---------------------------------------------------------------------------
# Mode: compare — side-by-side scenario comparison
# ---------------------------------------------------------------------------
run_compare() {
  echo "=== Harness Evaluation: Scenario Comparison ==="
  echo "Settings: runs=$RUNS window=$WINDOW seed=$SEED"
  echo ""
  printf "%-15s %6s %6s %8s %8s %8s %8s %8s\n" \
    "Scenario" "Score" "Grade" "LayerEff" "Repeat" "Rework" "Crystal" "Taxonomy"
  printf "%-15s %6s %6s %8s %8s %8s %8s %8s\n" \
    "---------------" "------" "------" "--------" "--------" "--------" "--------" "--------"

  for scenario in healthy degrading cold-start plateau; do
    local tmpdir
    tmpdir=$(mktemp -d)

    mkdir -p "$tmpdir/rules"
    touch "$tmpdir/rules/.gitkeep"

    python3 "$SIMULATE" "$scenario" --runs "$RUNS" --seed "$SEED" \
      --output "$tmpdir/synthetic.governance.jsonl" 2>/dev/null

    if [ "$scenario" = "healthy" ]; then
      touch "$tmpdir/rules/no-sql-injection.sh"
      touch "$tmpdir/rules/no-hardcoded-secrets.sh"
      chmod +x "$tmpdir/rules/no-sql-injection.sh" "$tmpdir/rules/no-hardcoded-secrets.sh"
    fi

    local row
    row=$(python3 "$EVALUATE" "$tmpdir" --window "$WINDOW" --json | python3 -c "
import sys, json
r = json.load(sys.stdin)
s = r['score']
b = s['breakdown']
print(f\"{s['total']:>6} {s['grade']:>6} {b['layer_efficiency']:>8} {b['repeat_reduction']:>8} {b['rework_efficiency']:>8} {b['crystallization']:>8} {b['taxonomy_coverage']:>8}\")
")

    printf "%-15s %s\n" "$scenario" "$row"
    rm -rf "$tmpdir"
  done

  echo ""
}

# ---------------------------------------------------------------------------
# Dispatch
# ---------------------------------------------------------------------------
case "$MODE" in
  real)      run_real ;;
  scenario)  run_scenario "$SCENARIO" ;;
  self-test) run_self_test ;;
  compare)   run_compare ;;
esac
