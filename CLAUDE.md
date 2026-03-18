# CLAUDE.md — Claude Code project instructions

## Project: Synodic

AI coding factory — structured BUILD → INSPECT pipelines for spec-driven development.

## Build & Test

```bash
cd cli && cargo build          # debug build
cd cli && cargo test           # run all tests (29 tests covering parser, scoring, alias resolution)
cd cli && cargo build --release # release build
pnpm install                   # install node deps (spec validation tooling)
```

## Architecture

Rust CLI (`cli/src/`) with Node.js tooling for spec validation.

```
cli/src/
├── main.rs                    # CLI entry: Harness | Eval subcommands
├── cmd/eval.rs                # Eval arg parsing → delegates to eval modules
├── cmd/harness.rs             # Harness arg parsing → delegates to harness modules
├── eval/
│   ├── run.rs                 # Orchestration: setup → agent → score (replaces run.sh)
│   ├── score/                 # Scoring pipeline (replaces score_runner.py + score.sh)
│   │   ├── parser.rs          # Django/pytest output parsing — pure functions, heavily tested
│   │   ├── runner.rs          # Test subprocess execution via Command
│   │   ├── verdict.rs         # F2P/P2P verdict computation with structural invariants
│   │   └── report.rs          # JSON score report generation
│   ├── setup/                 # Testbed setup (replaces setup/*.sh)
│   │   ├── swebench.rs        # SWE-bench: HF download → clone → patch → deps → prompt
│   │   ├── featurebench.rs    # FeatureBench: same + gold patch stripping + sanity check
│   │   └── devbench.rs        # DevBench: repo fetch → PRD extraction → scaffold
│   ├── batch.rs               # Batch evaluation across task×skill matrix
│   ├── list.rs                # List tasks from evals.json
│   └── report.rs              # Report generation (table/json/csv)
├── harness/                   # Governance loop (unchanged)
└── util.rs                    # find_repo_root(), exec_script()
```

### Key types (eval/score/mod.rs)

- `TestStatus` enum: Passed, Failed, Error, Skipped — no stringly-typed status
- `TestResult`: name + status + optional reason
- `ScoreResult`: passed/failed/errors/skipped — total is structural (passed > total impossible)
- `EvalVerdict`: instance_id + F2P verdict + P2P verdict + resolved bool

### Python remnant

HuggingFace dataset downloads stay as inline Python (~40 lines per setup module) called via `Command`. No viable Rust equivalent for `datasets` library.

## Claude Code Cloud Environment

The cloud container (Ubuntu 24.04, root, 16GB RAM, 4 CPU, 250GB disk) comes pre-installed with:

**Available out of the box:** git, docker, docker-compose, docker-buildx, curl, wget, jq, node 22, pnpm, npm, bun, cargo/rustc, python 3.10–3.13, go, ruby, java, maven, gradle

**Not pre-installed (installed by SessionStart hook):** `gh` (GitHub CLI)

**Networking:** All HTTP/HTTPS goes through a JWT-authenticated egress proxy. Proxy env vars (`HTTP_PROXY`, `HTTPS_PROXY`, `npm_config_proxy`, `JAVA_TOOL_OPTIONS`, etc.) are pre-set. Git uses a local proxy at 127.0.0.1:37671.

**GitHub API access:**
- `curl` to `api.github.com` works through the egress proxy (unauthenticated: 60 req/hr)
- Set `GH_TOKEN` env var in Claude Code settings for authenticated access (5000 req/hr + private repos + writes)
- `gh` CLI installed by `.github/setup-env.sh` SessionStart hook

## Spec Management via MCP

**All spec creation and mutation in `specs/` MUST go through the LeanSpec MCP server tools.** Do NOT use Write/Edit tools to create or modify spec files in `specs/` directly. The MCP server enforces validation rules (size limits, required frontmatter, dependency integrity) that prevent invalid specs from reaching disk.

**Scope:** This applies only to project-level specs in `specs/`. Fractal's internal working artifacts (`.fractal/{work-id}/tree/**/spec.md`) are ephemeral decomposition nodes — they use direct file writes and are not LeanSpec-managed.

**Available MCP tools (from `@leanspec/mcp`):**

| Tool | Purpose |
|------|---------|
| `create` | Create a new spec — validates before writing |
| `update` | Update spec metadata — re-validates before writing |
| `validate` | Dry-run validation without writing |
| `view` | Read a spec's content |
| `list` | List specs with filtering |
| `search` | Search across specs |
| `deps` | Show dependency graph |
| `link` / `unlink` | Manage dependency relationships |
| `board` | Kanban-style project board |
| `stats` | Project statistics |
| `tokens` | Token counting for context management |

**Why MCP instead of direct file writes:**
- `.lean-spec/config.json` defines validation rules (400-line max, 5000-token max, required frontmatter) but nothing enforces them with direct writes
- MCP tools are the **gatekeeper** — agents cannot bypass validation
- Dependency links are validated (dangling `depends_on` references rejected)
- Scope overlap detection via tag similarity

**Configuration:** `.mcp.json` at repo root registers the server. No manual setup needed.

## Conventions

- **Specs first**: Create a spec before starting non-trivial work
- **LeanSpec format**: All specs use YAML frontmatter (status, created, tags, priority)
- **Spec writes via MCP**: Always use LeanSpec MCP tools (`create`, `update`) — never write spec files directly
- **Governance**: All agent operations follow [HARNESS.md](./HARNESS.md)

## Skills

| Skill | Description | Usage |
|-------|-------------|-------|
| `factory` | Coding factory — transforms a spec into a reviewed PR via BUILD → INSPECT pipeline | `/factory run <spec-path>` |
| `fractal` | Fractal decomposition — recursively splits complex tasks into sub-specs, solves leaves independently, reunifies bottom-up | `/fractal decompose <task-or-spec-path>` |

### Skill installation

```bash
npx skills add codervisor/lean-spec@leanspec-sdd -g -y
```
