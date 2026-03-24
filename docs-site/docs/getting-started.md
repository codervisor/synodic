---
sidebar_position: 2
---

# Getting Started

## Installation

### From npm (recommended)

```bash
npm install -g @codervisor/synodic
```

### From source

```bash
git clone https://github.com/codervisor/synodic.git
cd synodic/rust
cargo build --release
# Binary at target/release/synodic
```

### Docker

```bash
docker pull ghcr.io/codervisor/synodic
docker run -p 3000:3000 ghcr.io/codervisor/synodic
```

## Quick start

### 1. Initialize

```bash
cd your-project
synodic init
```

This creates a `.harness/` directory with a SQLite database and default configuration.

### 2. Collect events from agent logs

```bash
# Auto-detect and parse Claude Code + Copilot logs
synodic collect --source auto

# Only Claude Code
synodic collect --source claude

# Only Copilot
synodic collect --source copilot

# Dry run — see what would be collected
synodic collect --dry-run

# Only events from the last hour
synodic collect --since 1h
```

### 3. View events

```bash
# List all events
synodic list

# Filter by type
synodic list --type hallucination

# Filter by severity
synodic list --severity critical

# Only unresolved
synodic list --unresolved

# Search by text
synodic search "file not found"

# Aggregate statistics
synodic stats
```

### 4. Resolve events

```bash
synodic resolve <event-id> --notes "Fixed in PR #42"
```

### 5. Live monitoring

```bash
# Terminal UI (Ratatui)
synodic watch

# Filter the live view
synodic watch --filter compliance_violation
```

### 6. Start the dashboard

```bash
synodic serve
# API:       http://localhost:3000/api
# Dashboard: http://localhost:3000
```

## Manual event submission

```bash
synodic submit \
  --type compliance_violation \
  --title "API key found in output" \
  --severity critical \
  --metadata '{"file": "config.py", "line": 42}'
```

## Install the governance skill

The `harness-governance` skill teaches AI agents to self-report governance events:

```bash
npx skills add codervisor/synodic@harness-governance -g -y
```

Once installed, the agent will:
- Self-report events it notices during operation
- Run `synodic collect` to scan its own logs
- Perform a self-audit checklist at end of major tasks
