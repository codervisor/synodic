---
sidebar_position: 3
---

# CLI Reference

## Global options

```
synodic [command] [options]
```

## Commands

### `synodic init`

Initialize `.harness/` directory and SQLite database in the current project.

```bash
synodic init
```

Creates:
- `.harness/synodic.db` — SQLite event database
- `.harness/rules/` — directory for crystallized rules
- `.harness/harness.governance.jsonl` — governance audit log

---

### `synodic submit`

Submit a governance event manually.

```bash
synodic submit --type <type> --title "<title>" [options]
```

| Option | Description | Default |
|--------|-------------|---------|
| `--type` | Event type: `tool_call_error`, `hallucination`, `compliance_violation`, `misalignment` | required |
| `--title` | Event title/description | required |
| `--severity` | `low`, `medium`, `high`, `critical` | `medium` |
| `--source` | Source identifier | `manual` |
| `--metadata` | JSON metadata | `{}` |

---

### `synodic collect`

Collect events from AI agent session logs.

```bash
synodic collect [options]
```

| Option | Description | Default |
|--------|-------------|---------|
| `--source` | Log source: `claude`, `copilot`, `auto` | `auto` |
| `--since` | Time window: `30m`, `1h`, `7d`, etc. | all time |
| `--dry-run` | Show what would be collected without inserting | `false` |

**Supported sources:**
- **Claude Code** — parses `~/.claude/projects/*/session-*.jsonl`
- **Copilot** — parses `~/.config/github-copilot/` event logs and VS Code extension data

---

### `synodic list`

List governance events.

```bash
synodic list [options]
```

| Option | Description |
|--------|-------------|
| `--type` | Filter by event type |
| `--severity` | Filter by severity |
| `--unresolved` | Only show unresolved events |
| `--source` | Filter by source |
| `--limit` | Max results |

---

### `synodic search`

Full-text search across events.

```bash
synodic search "<query>"
```

---

### `synodic stats`

Show aggregate statistics.

```bash
synodic stats [--since <duration>]
```

---

### `synodic resolve`

Resolve a governance event.

```bash
synodic resolve <id> [--notes "<notes>"]
```

---

### `synodic rules`

Manage detection rules.

```bash
synodic rules list            # List all rules
synodic rules test <rule>     # Test a rule
synodic rules add <pattern>   # Add a custom rule
```

---

### `synodic watch`

Live event monitoring with a terminal UI.

```bash
synodic watch [options]
```

| Option | Description | Default |
|--------|-------------|---------|
| `--filter` | Filter by event type | none |
| `--interval` | Poll interval in seconds | `2` |

**Keybindings:**
- `q` / `Esc` — quit
- `r` — force refresh

---

### `synodic serve`

Start the API server and dashboard.

```bash
synodic serve [options]
```

| Option | Description | Default |
|--------|-------------|---------|
| `--port` | Port number | `3000` |

Starts:
- REST API at `http://localhost:3000/api`
- WebSocket at `ws://localhost:3000/api/ws`
- Dashboard at `http://localhost:3000`

---

### `synodic harness`

Legacy governance harness with L1/L2 evaluation loop.

```bash
synodic harness run -- <agent_cmd>
synodic harness log [--json] [--tail N]
synodic harness rules
```
