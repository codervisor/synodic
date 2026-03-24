---
sidebar_position: 1
slug: /
---

# Synodic

**The tool that watches the AI agents.**

Synodic is an open-source AI agent event governance platform. It monitors, audits, and enforces governance rules on AI coding agent sessions (Claude Code, GitHub Copilot, Cursor, and more).

## What it does

- **Collects events** from AI agent session logs automatically
- **Detects issues** using pattern-based rules (secrets, dangerous commands, hallucinations)
- **Two-layer governance** — fast L1 static rules + L2 AI judge for semantic analysis
- **Crystallizes patterns** — recurring issues become enforceable rules automatically
- **Dashboard & TUI** — visual event feed, resolution queue, and live monitoring

## Product surface

| Interface | Description |
|-----------|-------------|
| **CLI** | Submit events, collect from logs, query, resolve, watch live |
| **Web Dashboard** | Event feed, resolution queue, analytics |
| **TUI** | Terminal-based live event monitoring |
| **REST API** | Programmatic access with WebSocket streaming |
| **Skill** | `harness-governance` makes agents self-reporting |

## Event types

| Type | Description |
|------|-------------|
| `tool_call_error` | Tool execution failures |
| `hallucination` | References to nonexistent files, APIs, or symbols |
| `compliance_violation` | Secrets exposure, dangerous commands, unauthorized access |
| `misalignment` | Agent actions diverge from user intent |

## Architecture

Synodic is a Rust workspace with three crates:

```
rust/
├── harness-core    # Event types, storage, detection rules, log parsers
├── harness-cli     # CLI: submit, collect, query, resolve, watch, serve
└── harness-http    # Axum REST API + WebSocket + dashboard static files
```

### Related repositories

- **[codervisor/eval](https://github.com/codervisor/eval)** — Standalone eval framework
- **[codervisor/orchestra](https://github.com/codervisor/orchestra)** — Coordination patterns (pipeline, fractal, swarm)
