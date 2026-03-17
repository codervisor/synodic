#!/usr/bin/env bash
# score-devbench.sh — Run DevBench acceptance tests and produce a score report
#
# Usage: ./score-devbench.sh <project-name> [--testbed-dir <path>] [--output <path>]
#
# DevBench scoring differs from SWE-bench/FeatureBench:
#   - No F2P/P2P distinction — uses acceptance tests
#   - Tests whether the built project compiles, runs, and meets requirements
#   - Optionally uses LLM-judge for design quality evaluation
#
# Scoring dimensions:
#   1. Build success (does it compile/install?)
#   2. Acceptance tests (do they pass?)
#   3. Code quality (optional LLM-judge)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# --- Argument parsing ---
PROJECT_NAME="${1:?Usage: score-devbench.sh <project-name> [--testbed-dir <path>] [--output <path>]}"
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
  TESTBED_DIR="/tmp/devbench-testbed/${PROJECT_NAME}"
fi

if [[ -z "$OUTPUT_FILE" ]]; then
  OUTPUT_FILE="${TESTBED_DIR}/.devbench/score_report.json"
fi

TASK_DIR="${TESTBED_DIR}/.devbench"
REPO_DIR="${TESTBED_DIR}/repo"

echo "=== DevBench Scoring ==="
echo "Project:  $PROJECT_NAME"
echo "Testbed:  $TESTBED_DIR"
echo ""

# Read metadata
LANGUAGE=$(python3 -c "import json; print(json.load(open('${TASK_DIR}/meta.json'))['language'])")
echo "Language: $LANGUAGE"
echo ""

cd "$REPO_DIR"

# --- Phase 1: Build check ---
echo "[1/3] Build check..."

BUILD_SUCCESS=false
BUILD_OUTPUT=""

case "$LANGUAGE" in
  python|Python)
    if [[ -f "setup.py" ]] || [[ -f "pyproject.toml" ]] || [[ -f "setup.cfg" ]]; then
      if pip install -e "." > /tmp/build_output.txt 2>&1; then
        BUILD_SUCCESS=true
        echo "  PASS — pip install succeeded"
      else
        BUILD_OUTPUT=$(tail -10 /tmp/build_output.txt)
        echo "  FAIL — pip install failed"
      fi
    elif [[ -f "requirements.txt" ]]; then
      if pip install -r requirements.txt > /tmp/build_output.txt 2>&1; then
        BUILD_SUCCESS=true
        echo "  PASS — requirements installed"
      else
        BUILD_OUTPUT=$(tail -10 /tmp/build_output.txt)
        echo "  FAIL — pip install failed"
      fi
    else
      # Python projects without setup files — check for syntax errors
      SYNTAX_ERRORS=$(find . -name "*.py" -exec python3 -m py_compile {} \; 2>&1 | head -20 || true)
      if [[ -z "$SYNTAX_ERRORS" ]]; then
        BUILD_SUCCESS=true
        echo "  PASS — all .py files compile"
      else
        BUILD_OUTPUT="$SYNTAX_ERRORS"
        echo "  FAIL — syntax errors found"
      fi
    fi
    ;;

  javascript|JavaScript|JS|typescript|TypeScript|TS)
    if [[ -f "package.json" ]]; then
      if npm install > /tmp/build_output.txt 2>&1; then
        if [[ -f "tsconfig.json" ]]; then
          if npx tsc --noEmit >> /tmp/build_output.txt 2>&1; then
            BUILD_SUCCESS=true
            echo "  PASS — npm install + tsc succeeded"
          else
            BUILD_OUTPUT=$(tail -10 /tmp/build_output.txt)
            echo "  FAIL — TypeScript compilation failed"
          fi
        else
          BUILD_SUCCESS=true
          echo "  PASS — npm install succeeded"
        fi
      else
        BUILD_OUTPUT=$(tail -10 /tmp/build_output.txt)
        echo "  FAIL — npm install failed"
      fi
    else
      BUILD_SUCCESS=true
      echo "  SKIP — no package.json (standalone scripts)"
    fi
    ;;

  java|Java)
    if [[ -f "pom.xml" ]]; then
      if mvn compile -q > /tmp/build_output.txt 2>&1; then
        BUILD_SUCCESS=true
        echo "  PASS — mvn compile succeeded"
      else
        BUILD_OUTPUT=$(tail -10 /tmp/build_output.txt)
        echo "  FAIL — mvn compile failed"
      fi
    elif [[ -f "build.gradle" ]] || [[ -f "build.gradle.kts" ]]; then
      if gradle build -q > /tmp/build_output.txt 2>&1; then
        BUILD_SUCCESS=true
        echo "  PASS — gradle build succeeded"
      else
        BUILD_OUTPUT=$(tail -10 /tmp/build_output.txt)
        echo "  FAIL — gradle build failed"
      fi
    else
      BUILD_SUCCESS=true
      echo "  SKIP — no build system detected"
    fi
    ;;

  c|C|cpp|C++)
    if [[ -f "Makefile" ]] || [[ -f "makefile" ]]; then
      if make > /tmp/build_output.txt 2>&1; then
        BUILD_SUCCESS=true
        echo "  PASS — make succeeded"
      else
        BUILD_OUTPUT=$(tail -10 /tmp/build_output.txt)
        echo "  FAIL — make failed"
      fi
    elif [[ -f "CMakeLists.txt" ]]; then
      mkdir -p build && cd build
      if cmake .. > /tmp/build_output.txt 2>&1 && make >> /tmp/build_output.txt 2>&1; then
        BUILD_SUCCESS=true
        echo "  PASS — cmake + make succeeded"
      else
        BUILD_OUTPUT=$(tail -10 /tmp/build_output.txt)
        echo "  FAIL — cmake build failed"
      fi
      cd "$REPO_DIR"
    else
      BUILD_SUCCESS=true
      echo "  SKIP — no build system detected"
    fi
    ;;

  *)
    echo "  SKIP — unknown language: $LANGUAGE"
    BUILD_SUCCESS=true
    ;;
esac

echo ""

# --- Phase 2: Acceptance tests ---
echo "[2/3] Running acceptance tests..."

ACCEPTANCE_DIR="${TASK_DIR}/acceptance_tests"
ACCEPT_PASSED=0
ACCEPT_FAILED=0
ACCEPT_TOTAL=0
ACCEPT_DETAILS=()

if [[ -d "$ACCEPTANCE_DIR" ]] && [[ "$(ls -A "$ACCEPTANCE_DIR" 2>/dev/null)" ]]; then
  # Run Python test files
  for test_file in "$ACCEPTANCE_DIR"/*.py; do
    [[ -f "$test_file" ]] || continue
    ACCEPT_TOTAL=$((ACCEPT_TOTAL + 1))
    TEST_NAME=$(basename "$test_file")
    echo -n "  Running: ${TEST_NAME}... "

    RESULT_FILE=$(mktemp)
    if timeout 120 python3 -m pytest "$test_file" --tb=short -q \
        > "${RESULT_FILE}.stdout" 2>&1; then
      echo "PASS"
      ACCEPT_PASSED=$((ACCEPT_PASSED + 1))
      ACCEPT_DETAILS+=("{\"test\": \"${TEST_NAME}\", \"status\": \"PASS\"}")
    else
      echo "FAIL"
      ACCEPT_FAILED=$((ACCEPT_FAILED + 1))
      REASON=$(tail -5 "${RESULT_FILE}.stdout" | tr '\n' ' ' | sed 's/"/\\"/g')
      ACCEPT_DETAILS+=("{\"test\": \"${TEST_NAME}\", \"status\": \"FAIL\", \"reason\": \"${REASON}\"}")
    fi
    rm -f "${RESULT_FILE}" "${RESULT_FILE}.stdout"
  done

  # Run shell test scripts
  for test_file in "$ACCEPTANCE_DIR"/*.sh; do
    [[ -f "$test_file" ]] || continue
    ACCEPT_TOTAL=$((ACCEPT_TOTAL + 1))
    TEST_NAME=$(basename "$test_file")
    echo -n "  Running: ${TEST_NAME}... "

    RESULT_FILE=$(mktemp)
    if timeout 120 bash "$test_file" > "${RESULT_FILE}.stdout" 2>&1; then
      echo "PASS"
      ACCEPT_PASSED=$((ACCEPT_PASSED + 1))
      ACCEPT_DETAILS+=("{\"test\": \"${TEST_NAME}\", \"status\": \"PASS\"}")
    else
      echo "FAIL"
      ACCEPT_FAILED=$((ACCEPT_FAILED + 1))
      REASON=$(tail -5 "${RESULT_FILE}.stdout" | tr '\n' ' ' | sed 's/"/\\"/g')
      ACCEPT_DETAILS+=("{\"test\": \"${TEST_NAME}\", \"status\": \"FAIL\", \"reason\": \"${REASON}\"}")
    fi
    rm -f "${RESULT_FILE}" "${RESULT_FILE}.stdout"
  done

  # Run JS test files
  for test_file in "$ACCEPTANCE_DIR"/*.js "$ACCEPTANCE_DIR"/*.test.js; do
    [[ -f "$test_file" ]] || continue
    ACCEPT_TOTAL=$((ACCEPT_TOTAL + 1))
    TEST_NAME=$(basename "$test_file")
    echo -n "  Running: ${TEST_NAME}... "

    RESULT_FILE=$(mktemp)
    if timeout 120 node "$test_file" > "${RESULT_FILE}.stdout" 2>&1; then
      echo "PASS"
      ACCEPT_PASSED=$((ACCEPT_PASSED + 1))
      ACCEPT_DETAILS+=("{\"test\": \"${TEST_NAME}\", \"status\": \"PASS\"}")
    else
      echo "FAIL"
      ACCEPT_FAILED=$((ACCEPT_FAILED + 1))
      REASON=$(tail -5 "${RESULT_FILE}.stdout" | tr '\n' ' ' | sed 's/"/\\"/g')
      ACCEPT_DETAILS+=("{\"test\": \"${TEST_NAME}\", \"status\": \"FAIL\", \"reason\": \"${REASON}\"}")
    fi
    rm -f "${RESULT_FILE}" "${RESULT_FILE}.stdout"
  done

  if [[ $ACCEPT_TOTAL -eq 0 ]]; then
    echo "  No executable test files found."
  fi
else
  echo "  No acceptance tests available."
fi

echo ""
if [[ $ACCEPT_TOTAL -gt 0 ]]; then
  echo "  Acceptance: ${ACCEPT_PASSED}/${ACCEPT_TOTAL} passed"
fi
echo ""

# --- Phase 3: Code metrics ---
echo "[3/3] Collecting code metrics..."

cd "$REPO_DIR"

# Count files and lines
FILE_COUNT=$(find . -name "*.py" -o -name "*.js" -o -name "*.ts" -o -name "*.java" \
  -o -name "*.c" -o -name "*.cpp" -o -name "*.h" -o -name "*.rs" \
  2>/dev/null | grep -v node_modules | grep -v __pycache__ | wc -l)

LINE_COUNT=$(find . -name "*.py" -o -name "*.js" -o -name "*.ts" -o -name "*.java" \
  -o -name "*.c" -o -name "*.cpp" -o -name "*.h" -o -name "*.rs" \
  2>/dev/null | grep -v node_modules | grep -v __pycache__ | xargs wc -l 2>/dev/null | tail -1 | awk '{print $1}')

echo "  Source files: $FILE_COUNT"
echo "  Total lines:  ${LINE_COUNT:-0}"
echo ""

# --- Compute scores ---
ACCEPT_ALL_PASS=$( [[ $ACCEPT_TOTAL -gt 0 ]] && [[ $ACCEPT_PASSED -eq $ACCEPT_TOTAL ]] && echo "true" || echo "false" )
ACCEPT_RATE=$( [[ $ACCEPT_TOTAL -gt 0 ]] && python3 -c "print(round(${ACCEPT_PASSED}/${ACCEPT_TOTAL}, 3))" || echo "0" )

# DevBench resolved = builds + all acceptance tests pass
BUILD_PASS=$( [[ "$BUILD_SUCCESS" == "true" ]] && echo "true" || echo "false" )
RESOLVED=$( [[ "$BUILD_PASS" == "true" ]] && [[ "$ACCEPT_ALL_PASS" == "true" ]] && echo "true" || echo "false" )

# Handle empty details
ACCEPT_DETAILS_JSON=""
if [[ ${#ACCEPT_DETAILS[@]} -gt 0 ]]; then
  ACCEPT_DETAILS_JSON=$(printf '%s,' "${ACCEPT_DETAILS[@]}" | sed 's/,$//')
fi

# --- Write score report ---
python3 -c "
import json, datetime

report = {
    'benchmark': 'devbench',
    'project': '${PROJECT_NAME}',
    'language': '${LANGUAGE}',
    'timestamp': datetime.datetime.utcnow().isoformat() + 'Z',
    'resolved': ${RESOLVED},
    'build': {
        'success': ${BUILD_PASS},
    },
    'acceptance': {
        'total': ${ACCEPT_TOTAL},
        'passed': ${ACCEPT_PASSED},
        'failed': ${ACCEPT_FAILED},
        'all_pass': ${ACCEPT_ALL_PASS},
        'pass_rate': ${ACCEPT_RATE},
        'details': json.loads('[${ACCEPT_DETAILS_JSON}]') if '${ACCEPT_DETAILS_JSON}' else []
    },
    'metrics': {
        'source_files': ${FILE_COUNT},
        'total_lines': ${LINE_COUNT:-0},
    }
}

with open('${OUTPUT_FILE}', 'w') as f:
    json.dump(report, f, indent=2)

print(json.dumps(report, indent=2))
"

echo ""
echo "=== Final Verdict ==="
if [[ "$RESOLVED" == "true" ]]; then
  echo "RESOLVED — Build succeeds and all acceptance tests pass."
else
  echo "FAILED"
  [[ "$BUILD_PASS" != "true" ]] && echo "  Build: FAILED"
  [[ "$ACCEPT_ALL_PASS" != "true" ]] && echo "  Acceptance: ${ACCEPT_PASSED}/${ACCEPT_TOTAL}"
fi
echo ""
echo "Score report: $OUTPUT_FILE"
