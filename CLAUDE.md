# CLAUDE.md — Claude Code project instructions

## Project: Synodic

Open-source AI agent event governance platform — monitor, audit, and enforce governance rules on AI coding agent sessions.

**Core identity:** The tool that watches the AI agents.

## Build & Test

```bash
cd rust && cargo build          # debug build (all crates)
cd rust && cargo test           # run all tests
cd rust && cargo build --release # release build
pnpm install                   # install node deps (spec validation tooling)
```

## Architecture

Cargo workspace (`rust/`) with three crates following the LeanSpec pattern (core/cli/http).

```
synodic/
├── rust/
│   ├── Cargo.toml                     # [workspace] members = ["harness-core", "harness-cli", "harness-http"]
│   ├── harness-core/                  # Event types, detection rules, storage, log parsers
│   │   └── src/
│   │       ├── lib.rs                 # Public API: events, storage, rules, parsers
│   │       ├── events.rs              # EventType, Severity, Event
│   │       ├── storage/
│   │       │   ├── mod.rs             # EventStore trait, EventFilter, Stats
│   │       │   └── sqlite.rs          # SQLite backend (default)
│   │       ├── rules/
│   │       │   └── mod.rs             # Rule, RuleEngine, crystallization
│   │       └── parsers/
│   │           └── mod.rs             # Claude Code, Copilot log parsers
│   ├── harness-cli/                   # CLI: submit, collect, query, resolve, watch, serve
│   │   └── src/
│   │       ├── main.rs                # CLI entry: top-level subcommands
│   │       ├── cmd/harness.rs         # Governance run, eval, log, rules, meta
│   │       ├── harness/               # Governance loop
│   │       │   ├── run.rs             # L1 static rules + L2 AI judge + rework loop
│   │       │   ├── log.rs             # Governance log display
│   │       │   └── rules.rs           # Crystallized rules list
│   │       ├── meta/                  # AI meta-testing framework
│   │       │   ├── mod.rs             # Orchestration + phase logic + rework loop
│   │       │   ├── consult.rs         # AI project analysis + diagnosis
│   │       │   ├── execute.rs         # Test execution pipeline
│   │       │   └── validate.rs        # Result reliability assessment
│   │       └── util.rs                # find_repo_root(), exec_script()
│   └── harness-http/                  # Axum REST API + dashboard static files
│       └── src/
│           └── main.rs                # HTTP server (PR 6)
├── packages/
│   ├── cli/                           # npm wrapper for Rust binary
│   └── ui/                            # Vite React dashboard
├── skills/
│   └── harness-governance/            # Agent self-reporting skill
├── docs-site/                         # Docusaurus documentation
├── docker/                            # Multi-stage Dockerfile
├── deploy/                            # Fly.io, Railway, Render configs
├── specs/                             # LeanSpec specs (harness-scoped)
├── .harness/                          # Governance config
│   ├── gates.yml                      # Preflight gates
│   ├── harness.governance.jsonl       # Governance log
│   ├── rules/                         # Crystallized rules
│   └── scripts/                       # Utility scripts
└── HARNESS.md                         # Governance protocol
```

### Extracted repositories

- **[codervisor/eval](https://github.com/codervisor/eval)** — Standalone eval framework (SWE-bench, FeatureBench, DevBench)
- **[codervisor/orchestra](https://github.com/codervisor/orchestra)** — Coordination patterns (pipeline engine, fractal, swarm, skills)

### Event types

- `tool_call_error` — tool execution failures
- `hallucination` — references to nonexistent files/APIs
- `compliance_violation` — secrets, dangerous commands, prod access
- `misalignment` — agent actions diverge from user intent

### Two-layer governance (from HARNESS.md)

- **L1**: Static/deterministic rules (zero AI cost, fast)
- **L2**: AI judge (independent LLM, fresh context, semantic analysis)

## Claude Code Cloud Environment

The cloud container (Ubuntu 24.04, root, 16GB RAM, 4 CPU, 250GB disk) comes pre-installed with:

**Available out of the box:** git, docker, docker-compose, docker-buildx, curl, wget, jq, node 22, pnpm, npm, bun, cargo/rustc, python 3.10–3.13, go, ruby, java, maven, gradle

**Not pre-installed (installed by SessionStart hook):** `gh` (GitHub CLI)

**Networking:** All HTTP/HTTPS goes through a JWT-authenticated egress proxy. Proxy env vars (`HTTP_PROXY`, `HTTPS_PROXY`, `npm_config_proxy`, `JAVA_TOOL_OPTIONS`, etc.) are pre-set. Git uses a local proxy at 127.0.0.1:37671.

**GitHub API access:**
- `curl` to `api.github.com` works through the egress proxy (unauthenticated: 60 req/hr)
- Set `GH_TOKEN` env var in Claude Code settings for authenticated access (5000 req/hr + private repos + writes)
- `gh` CLI installed by `.github/setup-env.sh` SessionStart hook

## Conventions

- **Specs first**: Create a spec before starting non-trivial work
- **LeanSpec format**: All specs use YAML frontmatter (status, created, tags, priority)
- **Governance**: All agent operations follow [HARNESS.md](./HARNESS.md)

## CLI commands

```bash
# Governance run (L1 + L2)
synodic harness run -- <agent_cmd>

# Log and rules
synodic harness log [--json] [--tail N]
synodic harness rules

# Meta-testing
synodic harness meta [--spec <path>] [--dry-run]
```

### Skill installation

```bash
npx skills add codervisor/lean-spec@leanspec-sdd -g -y
```
