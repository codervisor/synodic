#!/usr/bin/env bash
# score.sh — Run FeatureBench F2P and P2P tests and produce a score report
#
# Usage: ./score.sh <instance-id> [--testbed-dir <path>] [--output <path>]
#
# This script:
#   1. Loads F2P and P2P test lists from the task data
#   2. Runs pytest for F2P tests (must go from FAIL → PASS)
#   3. Runs pytest for P2P tests (must stay PASS)
#   4. Produces a JSON score report
#
# Prerequisites:
#   - pytest installed in the testbed venv
#   - Testbed set up via setup-testbed.sh

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

if [[ -z "$OUTPUT_FILE" ]]; then
  OUTPUT_FILE="${TESTBED_DIR}/.featurebench/score_report.json"
fi

TASK_DIR="${TESTBED_DIR}/.featurebench"
REPO_DIR="${TESTBED_DIR}/repo"
VENV_DIR="${TESTBED_DIR}/venv"

echo "=== FeatureBench Scoring ==="
echo "Instance: $INSTANCE_ID"
echo "Testbed:  $TESTBED_DIR"
echo ""

# Activate venv if it exists
if [[ -d "$VENV_DIR" ]]; then
  source "${VENV_DIR}/bin/activate"
fi

cd "$REPO_DIR"

# --- Parse test lists ---
F2P_FILE="${TASK_DIR}/fail_to_pass.json"
P2P_FILE="${TASK_DIR}/pass_to_pass.json"

# Extract test node IDs from JSON arrays
F2P_TESTS=()
if [[ -f "$F2P_FILE" ]]; then
  while IFS= read -r line; do
    F2P_TESTS+=("$line")
  done < <(python3 -c "
import json, sys
data = json.loads(open('${F2P_FILE}').read())
if isinstance(data, str):
    data = json.loads(data)
if isinstance(data, list):
    for t in data:
        print(t)
else:
    print(data)
")
fi

P2P_TESTS=()
if [[ -f "$P2P_FILE" ]]; then
  while IFS= read -r line; do
    P2P_TESTS+=("$line")
  done < <(python3 -c "
import json, sys
data = json.loads(open('${P2P_FILE}').read())
if isinstance(data, str):
    data = json.loads(data)
if isinstance(data, list):
    for t in data:
        print(t)
else:
    print(data)
")
fi

echo "F2P tests: ${#F2P_TESTS[@]}"
echo "P2P tests: ${#P2P_TESTS[@]}"
echo ""

# --- Run F2P tests ---
echo "[1/2] Running F2P tests (fail-to-pass)..."
echo "  These tests must PASS after your implementation."
echo ""

F2P_RESULTS_FILE="${TASK_DIR}/f2p_results.json"
F2P_PASSED=0
F2P_FAILED=0
F2P_ERRORS=0
F2P_TOTAL=${#F2P_TESTS[@]}
F2P_DETAILS=()

for test_id in "${F2P_TESTS[@]}"; do
  echo -n "  Running: ${test_id}... "

  # Run pytest with JSON output
  RESULT_FILE=$(mktemp)
  if timeout 300 python3 -m pytest "$test_id" \
    --tb=short --no-header -q \
    --junit-xml="${RESULT_FILE}.xml" \
    > "${RESULT_FILE}.stdout" 2>&1; then
    echo "PASS"
    F2P_PASSED=$((F2P_PASSED + 1))
    F2P_DETAILS+=("{\"test\": \"${test_id}\", \"status\": \"PASS\"}")
  else
    EXIT_CODE=$?
    if [[ $EXIT_CODE -eq 1 ]]; then
      echo "FAIL"
      F2P_FAILED=$((F2P_FAILED + 1))
      # Capture last 5 lines of output as failure reason
      REASON=$(tail -5 "${RESULT_FILE}.stdout" | tr '\n' ' ' | sed 's/"/\\"/g')
      F2P_DETAILS+=("{\"test\": \"${test_id}\", \"status\": \"FAIL\", \"reason\": \"${REASON}\"}")
    else
      echo "ERROR (exit $EXIT_CODE)"
      F2P_ERRORS=$((F2P_ERRORS + 1))
      REASON=$(tail -5 "${RESULT_FILE}.stdout" | tr '\n' ' ' | sed 's/"/\\"/g')
      F2P_DETAILS+=("{\"test\": \"${test_id}\", \"status\": \"ERROR\", \"exit_code\": ${EXIT_CODE}, \"reason\": \"${REASON}\"}")
    fi
  fi
  rm -f "${RESULT_FILE}" "${RESULT_FILE}.xml" "${RESULT_FILE}.stdout"
done

echo ""
echo "  F2P: ${F2P_PASSED}/${F2P_TOTAL} passed, ${F2P_FAILED} failed, ${F2P_ERRORS} errors"
echo ""

# --- Run P2P tests ---
echo "[2/2] Running P2P tests (pass-to-pass)..."
echo "  These tests must STILL PASS after your implementation."
echo ""

P2P_PASSED=0
P2P_FAILED=0
P2P_ERRORS=0
P2P_TOTAL=${#P2P_TESTS[@]}
P2P_DETAILS=()

for test_id in "${P2P_TESTS[@]}"; do
  echo -n "  Running: ${test_id}... "

  RESULT_FILE=$(mktemp)
  if timeout 300 python3 -m pytest "$test_id" \
    --tb=short --no-header -q \
    > "${RESULT_FILE}.stdout" 2>&1; then
    echo "PASS"
    P2P_PASSED=$((P2P_PASSED + 1))
    P2P_DETAILS+=("{\"test\": \"${test_id}\", \"status\": \"PASS\"}")
  else
    EXIT_CODE=$?
    if [[ $EXIT_CODE -eq 1 ]]; then
      echo "FAIL (regression!)"
      P2P_FAILED=$((P2P_FAILED + 1))
      REASON=$(tail -5 "${RESULT_FILE}.stdout" | tr '\n' ' ' | sed 's/"/\\"/g')
      P2P_DETAILS+=("{\"test\": \"${test_id}\", \"status\": \"FAIL\", \"reason\": \"${REASON}\"}")
    else
      echo "ERROR (exit $EXIT_CODE)"
      P2P_ERRORS=$((P2P_ERRORS + 1))
      REASON=$(tail -5 "${RESULT_FILE}.stdout" | tr '\n' ' ' | sed 's/"/\\"/g')
      P2P_DETAILS+=("{\"test\": \"${test_id}\", \"status\": \"ERROR\", \"exit_code\": ${EXIT_CODE}, \"reason\": \"${REASON}\"}")
    fi
  fi
  rm -f "${RESULT_FILE}" "${RESULT_FILE}.stdout"
done

echo ""
echo "  P2P: ${P2P_PASSED}/${P2P_TOTAL} passed, ${P2P_FAILED} failed, ${P2P_ERRORS} errors"
echo ""

# --- Compute final score ---
F2P_ALL_PASS=$( [[ $F2P_PASSED -eq $F2P_TOTAL ]] && echo "true" || echo "false" )
P2P_ALL_PASS=$( [[ $P2P_PASSED -eq $P2P_TOTAL ]] && echo "true" || echo "false" )
RESOLVED=$( [[ "$F2P_ALL_PASS" == "true" ]] && [[ "$P2P_ALL_PASS" == "true" ]] && echo "true" || echo "false" )

# --- Write score report ---
F2P_DETAILS_JSON=$(printf '%s,' "${F2P_DETAILS[@]}" | sed 's/,$//')
P2P_DETAILS_JSON=$(printf '%s,' "${P2P_DETAILS[@]}" | sed 's/,$//')

python3 -c "
import json, datetime

report = {
    'instance_id': '${INSTANCE_ID}',
    'timestamp': datetime.datetime.utcnow().isoformat() + 'Z',
    'resolved': ${RESOLVED},
    'f2p': {
        'total': ${F2P_TOTAL},
        'passed': ${F2P_PASSED},
        'failed': ${F2P_FAILED},
        'errors': ${F2P_ERRORS},
        'all_pass': ${F2P_ALL_PASS},
        'details': json.loads('[${F2P_DETAILS_JSON}]') if '${F2P_DETAILS_JSON}' else []
    },
    'p2p': {
        'total': ${P2P_TOTAL},
        'passed': ${P2P_PASSED},
        'failed': ${P2P_FAILED},
        'errors': ${P2P_ERRORS},
        'all_pass': ${P2P_ALL_PASS},
        'details': json.loads('[${P2P_DETAILS_JSON}]') if '${P2P_DETAILS_JSON}' else []
    }
}

with open('${OUTPUT_FILE}', 'w') as f:
    json.dump(report, f, indent=2)

print(json.dumps(report, indent=2))
"

echo ""
echo "=== Final Verdict ==="
if [[ "$RESOLVED" == "true" ]]; then
  echo "RESOLVED — All F2P and P2P tests pass."
else
  echo "FAILED"
  [[ "$F2P_ALL_PASS" != "true" ]] && echo "  F2P: ${F2P_PASSED}/${F2P_TOTAL} (need all to pass)"
  [[ "$P2P_ALL_PASS" != "true" ]] && echo "  P2P: ${P2P_PASSED}/${P2P_TOTAL} (regressions detected)"
fi
echo ""
echo "Score report: $OUTPUT_FILE"
