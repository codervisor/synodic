# CLAUDE.md — Claude Code project instructions

## Project: Synodic

Open-source AI agent governance and orchestration — enforce rules on AI coding agent sessions and automate Build→Inspect→PR pipelines.

**Core identity:** The tool that watches the AI agents.

## Build & Test

```bash
cd rust && cargo build          # debug build
cd rust && cargo test           # run all tests
cd rust && cargo build --release # release build
pnpm install                   # install node deps + activate git hooks
```

## Architecture

Two-layer governance with persistent storage (PostgreSQL / SQLite).

```
synodic/
├── rust/
│   ├── Cargo.toml                     # [workspace] members = ["harness-core", "harness-cli"]
│   ├── harness-core/                  # L2 interception engine + storage
│   │   ├── src/
│   │   │   ├── lib.rs                 # Public API
│   │   │   ├── intercept.rs           # InterceptEngine, rules, allow/block decisions
│   │   │   └── storage/
│   │   │       ├── mod.rs             # Storage trait, domain types (Rule, ThreatCategory, etc.)
│   │   │       ├── pool.rs            # Connection factory (auto-detects PG/SQLite)
│   │   │       └── sqlite.rs          # SQLite implementation
│   │   └── migrations/
│   │       ├── 001_initial_schema.sql # Tables: rules, threat_categories, feedback_events, etc.
│   │       └── 002_seed_data.sql      # 10 threat categories + 5 default rules
│   └── harness-cli/                   # CLI: init, intercept, feedback, rules, status, orchestrate
│       └── src/
│           ├── main.rs                # CLI entry (async via tokio)
│           ├── cmd/
│           │   ├── init.rs            # Setup governance + orchestration (hooks + pipeline)
│           │   ├── intercept.rs       # PreToolUse hook backend
│           │   ├── feedback.rs        # Record override/confirmed/ci_failure/incident
│           │   ├── orchestrate.rs     # Scaffold Build→Inspect→PR pipeline (workflow + config)
│           │   ├── rules.rs           # List/show rules with Beta stats
│           │   └── status.rs          # Coverage scores, gaps, recommendations
│           └── util.rs                # find_repo_root()
├── .githooks/                         # L1: Git hooks (deterministic, fast)
│   ├── pre-commit                     # cargo fmt --check
│   └── pre-push                       # fmt + clippy + test
├── .claude/                           # L2: Claude Code hooks (pattern-based blocking)
│   ├── settings.json                  # PreToolUse → intercept.sh
│   └── hooks/
│       └── intercept.sh               # stdin JSON → synodic intercept → exit 0/2
├── skills/
│   └── harness-governance/            # Agent self-reporting skill
├── packages/
│   ├── cli/                           # npm wrapper for Rust binary
│   └── ui/                            # Vite React dashboard
├── docs/
│   └── orchestration-patterns/        # Concept reference for pipeline topologies
├── docs-site/                         # Docusaurus documentation
└── specs/                             # LeanSpec specs
```

### Extracted repositories

- **[codervisor/eval](https://github.com/codervisor/eval)** — Standalone eval framework (SWE-bench, FeatureBench, DevBench)

### Two-layer governance

- **L1: Git hooks + CI** — deterministic, fast, tool-agnostic
  - `pre-commit`: `cargo fmt --check`
  - `pre-push`: fmt + clippy + test
  - CI: GitHub Actions (same checks, enforced)
- **L2: Claude Code hooks** — pattern-based real-time blocking
  - `PreToolUse` → `synodic intercept` evaluates tool calls against rules
  - 5 default rules: destructive-git, secrets-in-args, writes-outside-project, writes-to-system, dangerous-rm
  - Exit 0 = allow, Exit 2 = block

**Storage**: PostgreSQL for production, SQLite for local/demo. Rules, feedback events, and telemetry are persisted in DB. The intercept engine itself is stateless (reads rules from cache, <100ms).

### Orchestration

Synodic provides both governance and orchestration. `synodic init` sets up a Build→Inspect→PR pipeline alongside governance hooks, with language-specific quality gates.

Four coordination topologies documented in `docs/orchestration-patterns/`:

- **Factory** (implemented): Linear BUILD → INSPECT → route → PR. Best for clear, spec-driven tasks.
- **Adversarial**: Generate-attack loop with escalating critic modes. Best for security hardening.
- **Fractal**: Recursive decompose → parallel solve → reunify. Best for large, complex tasks.
- **Swarm**: Speculative parallel exploration → checkpoint → prune → merge. Best for ambiguous tasks.

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
- **Governance**: L1 via git hooks, L2 via Claude Code hooks (see `.githooks/` and `.claude/`)

## CLI commands

Three user-facing commands:

```bash
# Setup — one command for governance + orchestration
synodic init                    # Governance hooks + pipeline workflow (auto-detects language)
synodic init --no-orchestration # Governance only
synodic init --lang node        # Force language (rust, node, python, go)

# Monitor — health dashboard
synodic status [--json]         # Safety, friction, coverage scores + convergence

# Manage — all rule operations in one place
synodic rules list [--all]           # List rules with precision stats
synodic rules show <id>              # Rule details + recent feedback
synodic rules promote <id>           # Candidate → active
synodic rules crystallize <id>       # Tuned → L1 git hook
synodic rules deprecate <id>         # Disable rule
synodic rules check                  # Auto-transition active rules
synodic rules probe [--rule <id>]    # Test rules against evasion variants
synodic rules optimize [--dry-run]   # Propose rule candidates from patterns
```

Internal commands (called by hooks, hidden from `--help`): `intercept`, `feedback`.

**Environment**: Set `DATABASE_URL` for storage (default: `sqlite://~/.synodic/synodic.db`).

### What `synodic init` configures and generates

| File | Action | Purpose |
|---|---|---|
| `.githooks/pre-commit` | configures `core.hooksPath` | L1: cargo fmt check (must already exist) |
| `.githooks/pre-push` | configures `core.hooksPath` | L1: fmt + clippy + test (must already exist) |
| `.claude/settings.json` | generates | L2: PreToolUse → intercept hook |
| `.claude/hooks/intercept.sh` | generates | L2: Evaluates tool calls against rules |
| `.github/workflows/synodic-pipeline.yml` | generates | Build→Inspect→PR pipeline workflow |
| `.harness/pipeline.yml` | generates | Pipeline config (language, checks, max_rework) |
| `.harness/scripts/static_gate.sh` | generates | Custom quality gate hook (user-editable) |

**Supported languages** (auto-detected): Rust, Node (npm/pnpm/yarn/bun), Python, Go, Generic.

**Workflow requires**: `ANTHROPIC_API_KEY` secret in GitHub repo settings.

### Skill installation

```bash
npx skills add codervisor/lean-spec@leanspec-sdd -g -y
```
