---
status: planned
created: 2026-03-24
priority: critical
tags:
- vision
- harness
- governance
- architecture
- repositioning
created_at: 2026-03-24T06:00:39.814860715Z
updated_at: 2026-03-24T06:00:39.814860715Z
---

# Synodic Repositioning — Open-Source AI Agent Governance Platform

## Overview

Reposition Synodic from an "AI coding factory" (BUILD / INSPECT pipelines, coordination patterns) to a **self-contained, self-hostable AI agent event governance platform**. Synodic becomes a focused product: monitor, audit, and enforce governance rules on AI coding agent sessions.

Coordination patterns (factory, fractal, swarm, adversarial) and the eval framework move to separate repositories. What remains is a platform that answers: **"What did the AI agent do, and was it correct?"**

## Vision

Synodic is an open-source AI agent governance platform. It collects events from AI coding tool sessions (Claude Code, GitHub Copilot, Cursor, etc.), classifies them using detection rules, surfaces issues through a dashboard and CLI, and crystallizes recurring patterns into enforceable rules.

**Core identity:** The tool that watches the AI agents.

**Product surface:**
- **CLI** — submit events, collect from session logs, query, resolve, watch live
- **Web Dashboard** — event feed, resolution queue, analytics, rules manager
- **TUI** — terminal-based live event monitoring
- **Skill** — harness-governance skill makes agents self-reporting
- **REST API** — programmatic access for integrations

**Distribution model (follows LeanSpec):**
- npm-published CLI with platform-specific Rust binaries
- Docker image for self-hosting
- Skill installable via `npx skills add`
- Deploy configs for Fly.io, Railway, Render

## Architecture

### Repository structure

```
synodic/
├── rust/
│   ├── Cargo.toml                    # workspace: harness-core, harness-cli, harness-http
│   ├── harness-core/                 # Event types, detection rules, storage, parsers
│   ├── harness-cli/                  # CLI: submit, collect, query, resolve, watch, serve
│   └── harness-http/                 # Axum REST API + serves dashboard static files
├── packages/
│   ├── cli/                          # npm wrapper for Rust binary
│   └── ui/                           # Vite React dashboard (Tailwind, Radix, TanStack Query)
├── skills/
│   └── harness-governance/           # SKILL.md + agent instructions
│       └── SKILL.md
├── docs-site/                        # Docusaurus documentation
├── docker/                           # Multi-stage Dockerfile
├── deploy/                           # Fly.io, Railway, Render configs
├── schemas/                          # Event JSON schemas
├── specs/                            # LeanSpec specs (harness-scoped only)
├── HARNESS.md                        # Governance protocol
└── package.json                      # pnpm monorepo
```

### Rust crate mapping (LeanSpec-aligned)

| Crate | Role | LeanSpec equivalent |
|-------|------|---------------------|
| `harness-core` | Event types, detection rules, storage abstraction, log parsers | `leanspec-core` |
| `harness-cli` | CLI commands: submit, collect, query, resolve, rules, watch, serve | `leanspec-cli` |
| `harness-http` | Axum REST API, WebSocket for live events, serves dashboard | `leanspec-http` |

### Storage

- **SQLite** — default for local development and single-developer use. Zero-config, works out of the box.
- **PostgreSQL** — optional for team/org deployments. Same schema, storage abstraction switches based on `DATABASE_URL`.
- Storage abstraction trait in `harness-core` — implementations for both backends.

### CLI commands

```bash
# Setup
synodic init                              # initialize .harness/ in project
synodic auth login                        # configure API key (for remote mode)
synodic serve                             # start API + dashboard (localhost:3000)

# Event submission
synodic submit --type <type> --title "<title>" [--severity <level>] [--metadata '<json>']

# Log collection (passive)
synodic collect [--source claude|copilot|auto] [--since <duration>] [--dry-run]

# Query
synodic list [--type <type>] [--severity <level>] [--unresolved]
synodic search "<query>"
synodic stats [--since <duration>]

# Resolution
synodic resolve <id> [--notes "<notes>"]

# Rules
synodic rules list
synodic rules test <rule> --against <log>
synodic rules add <pattern>

# Live monitoring
synodic watch [--filter "<expr>"]         # TUI: live event stream
```

## Core capabilities

### Event types
- `tool_call_error` — tool execution failures
- `hallucination` — references to nonexistent files/APIs
- `compliance_violation` — secrets, dangerous commands, prod access
- `misalignment` — agent actions diverge from user intent

### Detection rules engine
- Pattern-based matching (regex, structural)
- Source-specific parsers (Claude Code JSONL, Copilot events.jsonl)
- Configurable severity thresholds
- Rule crystallization: pattern detected 3+ times -> candidate L1 rule -> backtest -> promote

### Two-layer governance (from HARNESS.md)
- **L1**: Static/deterministic rules (zero AI cost, fast)
- **L2**: AI judge (independent LLM, fresh context, semantic analysis)

### harness-governance skill
- SKILL.md teaches agents about event types and governance
- Agent self-reports events it notices (active)
- Agent runs `synodic collect` to scan its own logs (passive)
- Self-audit checklist at end of major tasks

## Scope reduction

### Extract to `codervisor/eval` (standalone)
- Entire `synodic-eval` crate
- `evals/` directory (tasks, setup, results)
- Already has clean separation boundary — zero governance deps

### Extract to `codervisor/orchestra` (coordination patterns)
- Pipeline engine (`pipeline/` — schema, executor, gates, vars, validate, checkpoint)
- Fractal algorithms (`fractal/` — decompose, schedule, reunify, prune)
- Swarm algorithms (`swarm/` — checkpoint, prune)
- Skills: factory, fractal, swarm, adversarial (SKILL.md + prompts + evals)
- Pipeline YAMLs (`.harness/pipelines/`)
- Schemas for pipeline outputs (build-report, inspect-verdict, etc.)

### Spec migration

**Stay in Synodic (harness scope):**

| Spec | Status | Reason |
|------|--------|--------|
| 048-post-session-governance | in-progress | Core governance model |
| 055-harness-real-world-assessment | complete | Harness validation results |
| 056-fix-harness-test-lean-spec | planned | Harness gap fixes |

**Move to `codervisor/orchestra`:**

| Spec | Status | Reason |
|------|--------|--------|
| 037-coding-factory-vision | planned | Factory concept |
| 044-factory-skill-mvp | in-progress | Factory skill |
| 049-factory-test-harness | planned | Factory testing |
| 050-fractal-algorithmic-spine | complete | Fractal algorithms |
| 052-fractal-factory-composition | planned | Coordination composition |
| 058-code-harness-orchestration | complete | Pipeline orchestration |
| 059-context-mesh | draft | Coordination primitive |
| 060-stigmergic-coordination | draft | Coordination primitive |
| 061-pipeline-engine-core | complete | Pipeline engine |
| 062-gate-system | complete | Gates |
| 063-pipeline-definitions | complete | Pipeline YAMLs |
| 064-algorithmic-commands | complete | Fractal/swarm CLI |
| 065-skill-migration | complete | Skill shims |

**Move to `codervisor/eval`:**

| Spec | Status | Reason |
|------|--------|--------|
| 046-synodic-dogfood | complete | Eval dogfood |
| 047-decouple-eval-framework | complete | Eval decoupling |
| 053-test-synodic-harness-lean-spec | complete | Eval test results |
| 066-umbrella-058-test-results | complete | Test assessment |

**Archive (superseded by repositioning):**

| Spec | Status | Reason |
|------|--------|--------|
| 045-rust-consolidation | complete | Old structure, no longer applicable |
| 051-production-roadmap | in-progress | Superseded by this spec |
| 066-ai-meta-testing-framework | in-progress | Moves with eval or gets reworked |

**Already archived (no action):** 001-043 (minus 037), 054, 057

## Migration path

### Phase 1: Extract eval (clean cut)
- [ ] Create `codervisor/eval` repo
- [ ] Move `cli/synodic-eval/` as the primary crate
- [ ] Move `evals/` directory
- [ ] Move eval-related specs (046, 047, 053, 066-umbrella)
- [ ] Remove synodic-eval from this workspace

### Phase 2: Extract coordination patterns
- [ ] Create `codervisor/orchestra` repo
- [ ] Move `pipeline/`, `fractal/`, `swarm/` modules
- [ ] Move skills (factory, fractal, swarm, adversarial) with prompts and evals
- [ ] Move `.harness/pipelines/` and pipeline output schemas
- [ ] Move coordination specs (037, 044, 049, 050, 052, 058-065)
- [ ] Orchestra consumes Synodic as a dependency for governance event submission

### Phase 3: Restructure this repo
- [ ] `cli/` -> `rust/` with new workspace (harness-core, harness-cli, harness-http)
- [ ] Add `packages/` (cli npm wrapper, ui)
- [ ] Create `skills/harness-governance/SKILL.md`
- [ ] Update CLAUDE.md, HARNESS.md, README.md for new scope
- [ ] Archive superseded specs, migrate applicable ones

### Phase 4: Build harness-core
- [ ] Event types and schemas
- [ ] Storage abstraction trait (SQLite + PostgreSQL)
- [ ] Detection rules engine
- [ ] Log parsers (Claude Code, Copilot, Cursor)
- [ ] L1 static rule evaluation

### Phase 5: Build harness-cli
- [ ] submit, collect, query, resolve commands
- [ ] rules management commands
- [ ] watch command (TUI via Ratatui)
- [ ] serve command (launches harness-http)

### Phase 6: Build harness-http + dashboard
- [ ] Axum REST API (events CRUD, rules, stats)
- [ ] WebSocket for live event streaming
- [ ] Vite React dashboard (event feed, resolution queue, analytics, rules manager)

### Phase 7: Distribution
- [ ] npm wrapper with platform-specific binaries
- [ ] Docker image (multi-stage build)
- [ ] Deploy configs (Fly.io, Railway, Render)
- [ ] Docusaurus documentation site
- [ ] harness-governance skill published

## Non-goals

- **Not a coordination orchestrator** — that is `codervisor/orchestra`
- **Not an eval framework** — that is `codervisor/eval`
- **Not a Nova Harness client** — Synodic is a standalone open-source platform
- **Not SaaS** — self-hostable first, cloud offering is a separate future decision

## Success criteria

- [ ] synodic-eval extracted to standalone repo with all tests passing
- [ ] Coordination patterns extracted to orchestra repo with all tests passing
- [ ] This repo builds and serves a working governance dashboard
- [ ] `synodic collect --source claude --dry-run` parses real session logs
- [ ] `synodic serve` starts API + dashboard on localhost
- [ ] harness-governance skill installable and functional
- [ ] SQLite works out of the box, PostgreSQL works with DATABASE_URL