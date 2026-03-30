#!/usr/bin/env bash
# PostToolUse event logger for Synodic governance.
#
# Logs tool usage events to .harness/events.jsonl for post-session
# analysis and pattern detection (L2 audit trail).
#
# Runs async — does not block the agent.

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
HARNESS_DIR="${PROJECT_DIR}/.harness"
LOG_FILE="${HARNESS_DIR}/events.jsonl"

# Ensure .harness directory exists
mkdir -p "$HARNESS_DIR"

# Read hook input from stdin
INPUT="$(cat)"

TOOL_NAME="$(echo "$INPUT" | jq -r '.tool_name // empty')"

# Skip if we can't parse
if [[ -z "$TOOL_NAME" ]]; then
  exit 0
fi

# Build a lightweight log entry (no tool_result to keep logs small)
TIMESTAMP="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
SESSION_ID="$(echo "$INPUT" | jq -r '.session_id // "unknown"')"

jq -n \
  --arg ts "$TIMESTAMP" \
  --arg tool "$TOOL_NAME" \
  --arg session "$SESSION_ID" \
  --arg event "tool_use" \
  '{timestamp: $ts, event: $event, tool: $tool, session_id: $session}' \
  >> "$LOG_FILE"

exit 0
