---
sidebar_position: 6
---

# Agent Integration

Synodic integrates with AI coding agents in two ways:

1. **Passive collection** — parse agent session logs after the fact
2. **Active self-reporting** — agents report events during operation via the governance skill

## Supported agents

### Claude Code

Claude Code writes session logs as JSONL files in `~/.claude/projects/<project>/session-*.jsonl`.

Synodic parses these logs to detect:
- **Tool errors** — `tool_result` entries with `is_error: true`
- **Rule violations** — secrets, dangerous commands, file-not-found errors in assistant output and tool results

```bash
synodic collect --source claude
```

### GitHub Copilot

Synodic looks for Copilot event logs in:
- `~/.config/github-copilot/` — global event logs
- VS Code extensions (`~/.vscode/extensions/github.copilot-*`) — conversation and event logs
- `.copilot-events/` — project-local event directory

Detected events include:
- **Tool errors** — `tool_error`, `command_error` events
- **Outcome failures** — completion/suggestion events with `outcome: "error"`
- **Content filter blocks** — rejected completions due to content filtering
- **Agent action failures** — `agent_action` events with error status

```bash
synodic collect --source copilot
```

## harness-governance skill

The `harness-governance` skill teaches agents to actively participate in governance.

### Installation

```bash
npx skills add codervisor/synodic@harness-governance -g -y
```

### What it does

Once installed, the agent will:

1. **Self-report events** — when the agent notices issues (hallucinations, errors, security concerns), it submits them directly
2. **Run log collection** — periodically runs `synodic collect` to scan its own session logs
3. **Self-audit** — performs a governance checklist at the end of major tasks

### How it works

The skill provides a `SKILL.md` file that gets loaded into the agent's context. It contains:
- Event type definitions and severity guidelines
- Instructions for when and how to submit events
- A self-audit checklist template

The agent calls `synodic submit` or the REST API to report events it detects during operation.

## Custom integration

### Via CLI

```bash
# Submit an event from any script or tool
synodic submit \
  --type hallucination \
  --title "Referenced nonexistent API endpoint /api/v2/users" \
  --severity medium \
  --source "my-agent" \
  --metadata '{"endpoint": "/api/v2/users"}'
```

### Via REST API

```bash
curl -X POST http://localhost:3000/api/events \
  -H "Content-Type: application/json" \
  -d '{
    "type": "tool_call_error",
    "title": "Build failed: missing dependency",
    "severity": "high",
    "source": "my-agent"
  }'
```

### Via WebSocket

Connect to `ws://localhost:3000/api/ws` to receive real-time event notifications. See the [WebSocket API](./websocket) for details.
