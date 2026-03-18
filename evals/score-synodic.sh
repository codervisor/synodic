#!/usr/bin/env bash
# score-synodic.sh — Score a completed Synodic dogfood eval run
#
# Usage: ./score-synodic.sh <instance-alias> [--testbed-dir <path>] [--output <path>]
#
# Scoring: runs cargo test in the testbed's score_dir.
# resolved = true if cargo test exits 0 (all tests pass).
#
# Prerequisites:
#   - cargo, rust

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# --- Argument parsing ---
INSTANCE_ALIAS="${1:?Usage: score-synodic.sh <instance-alias> [--testbed-dir <path>] [--output <path>]}"
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
  TESTBED_DIR="/tmp/synodic-testbed/${INSTANCE_ALIAS}"
fi

TASK_DATA_DIR="${TESTBED_DIR}/.synodic"
META_FILE="${TASK_DATA_DIR}/meta.json"

if [[ ! -f "$META_FILE" ]]; then
  echo "ERROR: Testbed metadata not found: $META_FILE" >&2
  echo "Run setup first: evals/setup/synodic.sh $INSTANCE_ALIAS" >&2
  exit 1
fi

SCORE_DIR=$(python3 -c "import json; print(json.load(open('${META_FILE}'))['score_dir'])")
INSTANCE_ID=$(python3 -c "import json; print(json.load(open('${META_FILE}'))['id'])")
REPO_DIR="${TESTBED_DIR}/repo"

if [[ -z "$OUTPUT_FILE" ]]; then
  OUTPUT_FILE="${TASK_DATA_DIR}/score_report.json"
fi

echo "=== Synodic Dogfood Scoring ==="
echo "Instance: $INSTANCE_ALIAS"
echo "Score dir: ${REPO_DIR}/${SCORE_DIR}"
echo "Output:   $OUTPUT_FILE"
echo ""

# --- Run cargo test ---
echo "[1/1] Running cargo test..."
echo ""

CARGO_OUTPUT_FILE="${TASK_DATA_DIR}/cargo_test_output.txt"

cd "${REPO_DIR}/${SCORE_DIR}"

START_TIME=$(date +%s)

if cargo test 2>&1 | tee "$CARGO_OUTPUT_FILE"; then
  RESOLVED=true
  EXIT_CODE=0
else
  RESOLVED=false
  EXIT_CODE=${PIPESTATUS[0]}
fi

END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

# Parse test counts from cargo output
PASSED=$(grep -oP 'test result: ok\. \K\d+(?= passed)' "$CARGO_OUTPUT_FILE" | awk '{s+=$1} END {print s+0}')
FAILED=$(grep -oP '\d+(?= failed)' "$CARGO_OUTPUT_FILE" | awk '{s+=$1} END {print s+0}')
IGNORED=$(grep -oP '\d+(?= ignored)' "$CARGO_OUTPUT_FILE" | awk '{s+=$1} END {print s+0}')

echo ""
echo "=== Final Verdict ==="
if [[ "$RESOLVED" == "true" ]]; then
  echo "RESOLVED — All cargo tests pass."
else
  echo "FAILED — cargo test exited with code $EXIT_CODE"
fi
echo ""
echo "  Passed:  $PASSED"
echo "  Failed:  $FAILED"
echo "  Ignored: $IGNORED"
echo "  Time:    ${DURATION}s"
echo ""

# Write score report (JSON)
python3 - << SCORE_REPORT
import json, datetime

report = {
    "instance_id": "${INSTANCE_ID}",
    "benchmark": "synodic",
    "resolved": ${RESOLVED},
    "exit_code": ${EXIT_CODE},
    "score": {
        "passed": ${PASSED},
        "failed": ${FAILED},
        "errors": 0,
        "skipped": ${IGNORED}
    },
    "duration_seconds": ${DURATION},
    "scored_at": datetime.datetime.utcnow().isoformat() + "Z"
}

with open("${OUTPUT_FILE}", "w") as f:
    json.dump(report, f, indent=2)

print(f"Score report written to: ${OUTPUT_FILE}")
SCORE_REPORT

echo ""

exit $([[ "$RESOLVED" == "true" ]] && echo 0 || echo 1)
