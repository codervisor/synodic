# CLAUDE.md — Claude Code project instructions

## Project: Synodic

Open-source AI agent governance via hooks — enforce rules on AI coding agent sessions.

**Core identity:** The tool that watches the AI agents.

## Build & Test

```bash
cd rust && cargo build          # debug build
cd rust && cargo test           # run all tests
cd rust && cargo build --release # release build
pnpm install                   # install node deps + activate git hooks
```

## Architecture

Two-layer governance, no custom databases or log files — just hooks.

```
synodic/
├── rust/
│   ├── Cargo.toml                     # [workspace] members = ["harness-core", "harness-cli"]
│   ├── harness-core/                  # L2 interception engine
│   │   └── src/
│   │       ├── lib.rs                 # Public API
│   │       └── intercept.rs           # InterceptEngine, rules, allow/block decisions
│   └── harness-cli/                   # CLI: init + intercept
│       └── src/
│           ├── main.rs                # CLI entry: init, intercept
│           ├── cmd/
│           │   ├── init.rs            # Setup L1 git hooks + L2 Claude Code hooks
│           │   └── intercept.rs       # PreToolUse hook backend
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
- **[codervisor/orchestra](https://github.com/codervisor/orchestra)** — Pipeline engine, fractal/swarm algorithms, coordination skills

### Two-layer governance

- **L1: Git hooks + CI** — deterministic, fast, tool-agnostic
  - `pre-commit`: `cargo fmt --check`
  - `pre-push`: fmt + clippy + test
  - CI: GitHub Actions (same checks, enforced)
- **L2: Claude Code hooks** — pattern-based real-time blocking
  - `PreToolUse` → `synodic intercept` evaluates tool calls against rules
  - 5 default rules: destructive-git, secrets-in-args, writes-outside-project, writes-to-system, dangerous-rm
  - Exit 0 = allow, Exit 2 = block

**No databases, no jsonl files, no custom event stores.** Governance is enforced through standard hook mechanisms.

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
- **Governance**: L1 via git hooks, L2 via Claude Code hooks (see `.githooks/` and `.claude/`)

## CLI commands

```bash
synodic init                    # Setup L1 git hooks + L2 Claude Code hooks
synodic intercept --tool <name> --input '<json>'  # Evaluate tool call (called by hooks)
```

### Skill installation

```bash
npx skills add codervisor/lean-spec@leanspec-sdd -g -y
```
