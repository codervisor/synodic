#!/usr/bin/env bash
# static_gate.sh — Detect languages in a git diff, run appropriate checkers.
#
# Usage: ./static_gate.sh [base-ref] [head-ref]
#   base-ref: base branch (default: main)
#   head-ref: head branch (default: HEAD)
#
# Outputs a JSON report of failures. Exit 0 = all pass, non-zero = failures found.

set -euo pipefail

BASE_REF="${1:-main}"
HEAD_REF="${2:-HEAD}"

# Collect changed files
CHANGED_FILES=$(git diff --name-only "$BASE_REF"..."$HEAD_REF" 2>/dev/null || true)

if [ -z "$CHANGED_FILES" ]; then
  echo '{"passed": true, "failures": []}'
  exit 0
fi

FAILURES=()

has_ext() {
  echo "$CHANGED_FILES" | grep -qE "\.$1$"
}

run_check() {
  local name="$1"
  shift
  if output=$("$@" 2>&1); then
    return 0
  else
    FAILURES+=("{\"checker\": \"$name\", \"output\": $(echo "$output" | head -20 | python3 -c 'import sys,json; print(json.dumps(sys.stdin.read()))' 2>/dev/null || echo '""')}")
    return 1
  fi
}

# Rust checks (cargo fmt handled by crystallized rule in .harness/rules/)
if has_ext rs; then
  run_check "cargo_check" cargo check --quiet 2>/dev/null || true
  run_check "cargo_clippy" cargo clippy --quiet -- -D warnings 2>/dev/null || true
fi

# TypeScript/JavaScript checks
if has_ext ts || has_ext tsx || has_ext js || has_ext jsx; then
  if command -v tsc &>/dev/null; then
    run_check "tsc" tsc --noEmit 2>/dev/null || true
  fi
  if command -v eslint &>/dev/null; then
    run_check "eslint" eslint --quiet $CHANGED_FILES 2>/dev/null || true
  fi
fi

# Python checks
if has_ext py; then
  if command -v pyright &>/dev/null; then
    run_check "pyright" pyright 2>/dev/null || true
  fi
  if command -v ruff &>/dev/null; then
    run_check "ruff" ruff check 2>/dev/null || true
  fi
fi

# Custom rules from .harness/rules/
RULES_DIR="$(git rev-parse --show-toplevel 2>/dev/null)/.harness/rules"
if [ -d "$RULES_DIR" ]; then
  for rule in "$RULES_DIR"/*; do
    if [ -x "$rule" ] && [ -f "$rule" ] && [ "$(basename "$rule")" != ".gitkeep" ]; then
      DIFF=$(git diff "$BASE_REF"..."$HEAD_REF")
      if ! echo "$DIFF" | "$rule" >/dev/null 2>&1; then
        rule_output=$(echo "$DIFF" | "$rule" 2>&1 | head -10 || true)
        FAILURES+=("{\"checker\": \"rule:$(basename "$rule")\", \"output\": $(echo "$rule_output" | python3 -c 'import sys,json; print(json.dumps(sys.stdin.read()))' 2>/dev/null || echo '""')}")
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
