# CLAUDE.md — Claude Code project instructions

## Project: Synodic

Open-source AI agent event governance platform — monitor, audit, enforce governance rules on AI coding agent sessions.

**Core identity:** The tool that watches the AI agents.

## Build & Test

```bash
cd rust && cargo build          # debug build (all crates)
cd rust && cargo test           # run all tests
cd rust && cargo build --release # release build
pnpm install                   # install node deps (spec validation tooling)
```

## Architecture

Cargo workspace (`rust/`) with three crates focused on governance.

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
│   │       │   ├── sqlite.rs          # SQLite backend (default)
│   │       │   └── postgres.rs        # PostgreSQL backend (feature-gated)
│   │       ├── rules/
│   │       │   └── mod.rs             # Rule, RuleEngine, pattern detection
│   │       └── parsers/
│   │           ├── mod.rs             # LogParser trait
│   │           ├── claude.rs          # Claude Code JSONL log parser
│   │           └── copilot.rs         # GitHub Copilot log parser
│   ├── harness-cli/                   # CLI: submit, collect, query, resolve, watch, serve
│   │   └── src/
│   │       ├── main.rs                # CLI entry: top-level subcommands
│   │       ├── cmd/
│   │       │   └── harness_legacy.rs  # Governance run, log
│   │       ├── harness/               # Governance loop
│   │       │   ├── run.rs             # L2 AI judge + rework loop
│   │       │   └── log.rs             # Governance log display
│   │       └── util.rs                # find_repo_root()
│   └── harness-http/                  # Axum REST API + dashboard static files
│       └── src/
│           └── main.rs                # HTTP server
├── skills/
│   └── harness-governance/            # Agent self-reporting skill
├── packages/
│   ├── cli/                           # npm wrapper for Rust binary
│   └── ui/                            # Vite React dashboard
├── docs/
│   └── orchestration-patterns/        # Concept reference for pipeline topologies
├── docs-site/                         # Docusaurus documentation
├── docker/                            # Multi-stage Dockerfile
├── deploy/                            # Fly.io, Railway, Render configs
├── specs/                             # LeanSpec specs
├── .harness/                          # Governance config
│   └── README.md                      # Governance protocol reference
├── .githooks/                         # Git hooks (L1 governance)
│   ├── pre-commit                     # cargo fmt --check
│   └── pre-push                       # fmt + clippy + test
└── HARNESS.md                         # Governance protocol
```

### Extracted repositories

- **[codervisor/eval](https://github.com/codervisor/eval)** — Standalone eval framework (SWE-bench, FeatureBench, DevBench)
- **[codervisor/orchestra](https://github.com/codervisor/orchestra)** — Pipeline engine, fractal/swarm algorithms, coordination skills

### Event types

- `tool_call_error` — tool execution failures
- `hallucination` — references to nonexistent files/APIs
- `compliance_violation` — secrets, dangerous commands, prod access
- `misalignment` — agent actions diverge from user intent

### Two-layer governance (from HARNESS.md)

- **L1**: Git hooks + CI (deterministic, fast, tool-agnostic)
- **L2**: Synodic AI judge + event collection + pattern detection (semantic, unique value)

### Pipeline topologies (concept reference)

Four coordination patterns documented in `docs/orchestration-patterns/`:

- **Factory**: Linear BUILD -> INSPECT -> route -> PR. Best for clear, spec-driven tasks.
- **Adversarial**: Generate-attack loop with escalating critic modes. Best for security hardening.
- **Fractal**: Recursive decompose -> parallel solve -> reunify. Best for large, complex tasks.
- **Swarm**: Speculative parallel exploration -> checkpoint -> prune -> merge. Best for ambiguous tasks.

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
# Governance
synodic harness run -- <agent_cmd>   # L2 AI judge + rework loop
synodic harness log [--json] [--tail N]

# Event management
synodic submit --type <type> --title "<title>" [--severity <level>]
synodic collect [--source claude|copilot|auto] [--since <duration>]
synodic list [--type <type>] [--severity <level>] [--unresolved]
synodic search "<query>"
synodic stats [--since <duration>]
synodic resolve <id> [--notes "<notes>"]

# Rules
synodic rules list

# Live monitoring
synodic watch [--filter "<expr>"]

# Server
synodic serve
```

### Skill installation

```bash
npx skills add codervisor/lean-spec@leanspec-sdd -g -y
```
