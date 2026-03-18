# CLAUDE.md — Claude Code project instructions

## Project: Synodic

AI coding factory — structured BUILD → INSPECT pipelines for spec-driven development.

## Build & Test

```bash
cd cli && cargo build          # debug build (both crates)
cd cli && cargo test           # run all tests (35 tests covering parser, scoring, alias resolution)
cd cli && cargo build --release # release build
cd cli/synodic-eval && cargo test  # eval tests only (standalone)
pnpm install                   # install node deps (spec validation tooling)
```

## Architecture

Cargo workspace (`cli/`) with two crates and Node.js tooling for spec validation.

**synodic-eval** is a standalone eval framework — no governance concepts. **synodic** is the governance CLI that depends on synodic-eval as a library.

```
cli/
├── Cargo.toml                     # [workspace] members = ["synodic", "synodic-eval"]
├── synodic-eval/                  # Standalone eval framework (zero synodic governance deps)
│   └── src/
│       ├── lib.rs                 # Public API: run, score, batch, list, report, setup
│       ├── main.rs                # Binary: `synodic-eval run|score|list|batch|report`
│       ├── run.rs                 # Orchestration: setup → agent → score → EvalResult
│       ├── batch.rs               # Batch evaluation across task×skill matrix
│       ├── list.rs                # List tasks from evals.json
│       ├── report.rs              # Report generation (table/json/csv)
│       ├── util.rs                # find_project_root() (EVAL_ROOT, evals/, .git)
│       ├── score/                 # Scoring pipeline
│       │   ├── mod.rs             # Types: TestStatus, TestResult, ScoreResult, EvalVerdict
│       │   ├── parser.rs          # Django/pytest output parsing — heavily tested
│       │   ├── runner.rs          # Test subprocess execution via Command
│       │   ├── verdict.rs         # F2P/P2P verdict computation
│       │   └── report.rs          # JSON score report generation
│       └── setup/                 # Testbed setup
│           ├── swebench.rs        # SWE-bench: HF download → clone → patch → deps → prompt
│           ├── featurebench.rs    # FeatureBench: same + gold patch stripping + sanity check
│           ├── devbench.rs        # DevBench: repo fetch → PRD extraction → scaffold
│           └── synodic.rs         # Synodic dogfood: clone → verify build → prompt
├── synodic/                       # Governance CLI (depends on synodic-eval)
│   └── src/
│       ├── main.rs                # CLI entry: Harness | Eval subcommands
│       ├── cmd/eval.rs            # Eval dispatch → synodic_eval + governance log
│       ├── cmd/harness.rs         # Harness dispatch → harness modules
│       ├── governance.rs          # Eval result → .harness/eval.governance.jsonl
│       ├── util.rs                # find_repo_root() (SYNODIC_ROOT, .harness/, .git)
│       └── harness/               # Governance loop
│           ├── run.rs             # L1 static rules + L2 AI judge + rework loop
│           ├── log.rs             # Governance log display
│           └── rules.rs           # Crystallized rules list
```

### Key types (synodic-eval: score/mod.rs)

- `TestStatus` enum: Passed, Failed, Error, Skipped — no stringly-typed status
- `TestResult`: name + status + optional reason
- `ScoreResult`: passed/failed/errors/skipped — total is structural (passed > total impossible)
- `EvalVerdict`: instance_id + F2P verdict + P2P verdict + resolved bool
- `EvalResult`: target + verdict + duration + resolved (returned by `run::execute()`)

### Separation boundary

Eval produces structured output (EvalResult / exit code). Synodic's governance.rs reads it and writes `.harness/eval.governance.jsonl`. Eval never writes governance logs or references `.harness/`.

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
| `create` | Create a new spec — auto-generates number + frontmatter, validates before writing |
| `update` | Update metadata/content via replacements, section updates, checklist toggles |
| `validate` | Validate frontmatter, structure, dependencies (dry-run or check existing) |
| `view` | Read a spec's full content and metadata |
| `list` | List specs with filtering by status, tags, priority |
| `search` | Full-text search with AND/OR/NOT, field filters, fuzzy matching |
| `relationships` | View/add/remove parent, child, and depends_on relationships |
| `children` | List direct child specs of a parent |
| `deps` | Show upstream/downstream dependency graph |
| `board` | Project board grouped by status, priority, assignee, or tag |
| `stats` | Project statistics and insights |
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
