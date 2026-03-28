#!/usr/bin/env bash
# static_gate.sh — Run governance-specific checks on a diff.
#
# Standard lint/test checks (cargo fmt, clippy, cargo test, tsc, eslint)
# are handled by git hooks (.githooks/) and CI (.github/workflows/ci.yml).
#
# This script runs ONLY governance-specific rules:
#   - Crystallized rules from .harness/rules/ (pattern-detected, human-approved)
#   - Future: compliance checks, secret scanning, dependency boundary enforcement
#
# Usage: ./static_gate.sh [base-ref] [head-ref]
# Output: JSON report. Exit 0 = pass, non-zero = failures found.

set -euo pipefail

BASE_REF="${1:-main}"
HEAD_REF="${2:-HEAD}"

DIFF=$(git diff "$BASE_REF"..."$HEAD_REF" 2>/dev/null || true)

if [ -z "$DIFF" ]; then
  echo '{"passed": true, "failures": []}'
  exit 0
fi

FAILURES=()

# Run crystallized rules from .harness/rules/
RULES_DIR="$(git rev-parse --show-toplevel 2>/dev/null)/.harness/rules"
if [ -d "$RULES_DIR" ]; then
  for rule in "$RULES_DIR"/*; do
    if [ -x "$rule" ] && [ -f "$rule" ] && [ "$(basename "$rule")" != ".gitkeep" ]; then
      rule_name="$(basename "$rule")"
      if ! echo "$DIFF" | "$rule" >/dev/null 2>&1; then
        rule_output=$(echo "$DIFF" | "$rule" 2>&1 | head -10 || true)
        FAILURES+=("{\"checker\": \"rule:${rule_name}\", \"output\": $(echo "$rule_output" | python3 -c 'import sys,json; print(json.dumps(sys.stdin.read()))' 2>/dev/null || echo '""')}")
      fi
    fi
  done
fi

# Output JSON report
if [ ${#FAILURES[@]} -eq 0 ]; then
  echo '{"passed": true, "failures": []}'
  exit 0
else
  echo -n '{"passed": false, "failures": ['
  first=true
  for f in "${FAILURES[@]}"; do
    if [ "$first" = true ]; then
      first=false
    else
      echo -n ","
    fi
    echo -n "$f"
  done
  echo ']}'
  exit 1
fi
