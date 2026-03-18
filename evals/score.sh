#!/usr/bin/env bash
# score.sh — Run F2P and P2P tests and produce a score report
#
# Usage: ./score.sh <instance-id> [--testbed-dir <path>] [--output <path>]
#
# This script:
#   1. Loads F2P and P2P test lists from the task data
#   2. Runs the appropriate test runner (pytest or Django's test runner)
#   3. Checks which tests pass/fail
#   4. Produces a JSON score report
#
# Supports both SWE-bench and FeatureBench testbeds (auto-detected from
# the presence of .swebench/ or .featurebench/ in the testbed directory).
#
# Prerequisites:
#   - Testbed set up via setup/swebench.sh or setup/featurebench.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# --- Argument parsing ---
INSTANCE_ID="${1:?Usage: score.sh <instance-id> [--testbed-dir <path>] [--output <path>]}"
shift

TESTBED_DIR=""
OUTPUT_FILE=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --testbed-dir) TESTBED_DIR="$2"; shift 2 ;;
    --output) OUTPUT_FILE="$2"; shift 2 ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

if [[ -z "$TESTBED_DIR" ]]; then
  TESTBED_DIR="/tmp/featurebench-testbed/${INSTANCE_ID}"
fi

# Auto-detect benchmark type from testbed contents
if [[ -d "${TESTBED_DIR}/.swebench" ]]; then
  BENCH_TYPE="swebench"
  TASK_DIR="${TESTBED_DIR}/.swebench"
elif [[ -d "${TESTBED_DIR}/.featurebench" ]]; then
  BENCH_TYPE="featurebench"
  TASK_DIR="${TESTBED_DIR}/.featurebench"
else
  if [[ "$TESTBED_DIR" == *swebench* ]]; then
    BENCH_TYPE="swebench"
    TASK_DIR="${TESTBED_DIR}/.swebench"
  else
    BENCH_TYPE="featurebench"
    TASK_DIR="${TESTBED_DIR}/.featurebench"
  fi
fi

if [[ -z "$OUTPUT_FILE" ]]; then
  OUTPUT_FILE="${TASK_DIR}/score_report.json"
fi

REPO_DIR="${TESTBED_DIR}/repo"
VENV_DIR="${TESTBED_DIR}/venv"

BENCH_LABEL="SWE-bench"
[[ "$BENCH_TYPE" == "featurebench" ]] && BENCH_LABEL="FeatureBench"

echo "=== ${BENCH_LABEL} Scoring ==="
echo "Instance: $INSTANCE_ID"
echo "Testbed:  $TESTBED_DIR"
echo ""

# Activate venv if it exists
if [[ -d "$VENV_DIR" ]]; then
  source "${VENV_DIR}/bin/activate"
fi

cd "$REPO_DIR"

# --- Delegate to Python scorer ---
# The Python scorer handles:
#   - Parsing SWE-bench test ID formats (Django, pytest, etc.)
#   - Choosing the right test runner per project
#   - Running tests in batch (not one-by-one)
#   - Producing the JSON report

python3 "${SCRIPT_DIR}/score_runner.py" \
  --instance-id "$INSTANCE_ID" \
  --task-dir "$TASK_DIR" \
  --repo-dir "$REPO_DIR" \
  --output "$OUTPUT_FILE" \
  --bench-type "$BENCH_TYPE"

EXIT=$?

echo ""
echo "Score report: $OUTPUT_FILE"
exit $EXIT
