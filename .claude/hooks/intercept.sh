#!/usr/bin/env bash
# L2 Interception hook for Claude Code PreToolUse events.
#
# Reads tool call JSON from stdin, evaluates against Synodic's intercept
# rules, and returns the appropriate exit code + output for Claude Code.
#
# Exit 0 = allow, Exit 2 = block (with reason on stderr).

set -euo pipefail

# Fail-open if jq is not available
if ! command -v jq &>/dev/null; then
  cat >/dev/null  # drain stdin
  exit 0
fi

PROJECT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
SYNODIC_BIN="${SYNODIC_BIN:-${PROJECT_DIR}/rust/target/release/synodic}"

# Fall back to debug build if release doesn't exist
if [[ ! -x "$SYNODIC_BIN" ]]; then
  SYNODIC_BIN="${PROJECT_DIR}/rust/target/debug/synodic"
fi

# If no binary, allow (don't block the agent on missing build)
if [[ ! -x "$SYNODIC_BIN" ]]; then
  exit 0
fi

# Read hook input from stdin
INPUT="$(cat)"

# Extract tool_name and tool_input from the hook's JSON payload (fail-open on parse error)
TOOL_NAME="$(echo "$INPUT" | jq -r '.tool_name // empty' 2>/dev/null)" || true
TOOL_INPUT="$(echo "$INPUT" | jq -c '.tool_input // {}' 2>/dev/null)" || TOOL_INPUT='{}'

# If we couldn't parse the input, allow
if [[ -z "$TOOL_NAME" ]]; then
  exit 0
fi

# Call synodic intercept
RESULT="$("$SYNODIC_BIN" intercept --tool "$TOOL_NAME" --input "$TOOL_INPUT" 2>/dev/null)" || {
  # If the command fails, allow (fail-open)
  exit 0
}

DECISION="$(echo "$RESULT" | jq -r '.decision // "allow"' 2>/dev/null)" || true

if [[ "$DECISION" == "block" ]]; then
  REASON="$(echo "$RESULT" | jq -r '.reason // "Blocked by Synodic governance rule"' 2>/dev/null)" || REASON="Blocked by Synodic governance rule"
  RULE="$(echo "$RESULT" | jq -r '.rule // "unknown"' 2>/dev/null)" || RULE="unknown"
  echo "Synodic L2 interception [$RULE]: $REASON" >&2
  exit 2
fi

exit 0
