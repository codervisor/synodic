# Synodic — AI Coding Factory

> *synodic* (adj.) — from Greek *synodos*, "meeting, conjunction." The period when orbiting bodies align into the same configuration.

**Synodic** is a skill package for [Claude Code](https://docs.anthropic.com/en/docs/claude-code) that implements structured AI coding workflows — transforming specs into reviewed PRs via adversarial BUILD → INSPECT pipelines.

## Why Synodic?

A single AI agent can write code, but it can't objectively review its own output. Synodic implements a **factory model**: one agent builds, a separate agent inspects with fresh context (no builder bias). This adversarial review catches bugs and spec violations that self-review misses.

The core thesis: **adversarial review by a separate agent instance produces measurably better results than a single agent run, with acceptable overhead.**

## Quick Start

### Prerequisites

- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) installed and configured
- `gh` CLI authenticated (for PR creation)
- Git repository with a spec to implement

### Install Skills

```bash
# Install Synodic skills globally
npx skills add codervisor/synodic -g -y
```

### Run the Factory

```bash
# Inside Claude Code, invoke the factory skill on a spec
/factory run specs/044-factory-skill-mvp/README.md
```

The factory reads your spec, implements code in an isolated worktree, runs an adversarial review, and creates a PR — all automatically.

## Skills

Each skill is backed by a declarative pipeline YAML in `.harness/pipelines/`. Skills are invoked via Claude Code; orchestration is handled by the pipeline engine, not inline prose.

| Skill | What it does | Invoke with |
|-------|-------------|-------------|
| **Factory** | Implements a spec as a reviewed PR via BUILD → INSPECT pipeline | `/factory run <spec-path>` |
| **Fractal** | Decomposes complex tasks into sub-specs, solves leaves in parallel, reunifies bottom-up | `/fractal decompose <task-or-spec-path>` |
| **Swarm** | Forks N agents on divergent strategies, prunes convergent branches, fuses best fragments | `/swarm run <spec-path>` |
| **Adversarial** | Locks generator + critic in escalating quality loop for deep hardening | `/adversarial run <spec-path>` |

### Factory — BUILD → INSPECT Pipeline

The factory pipeline (`factory.yml`) runs adversarial review with a bounded rework loop:

```
agent: build    →  run: preflight gate  →  agent: inspect
                                                │
                                           branch: verdict
                                           ├─ approve → run: create-pr
                                           ├─ rework  → loop back to build
                                           └─ exhaust → governance log (human review)
```

Key properties:
- **Isolation** — BUILD runs in a git worktree, can't pollute the main branch
- **Adversarial** — INSPECT has fresh context, no builder bias
- **Bounded** — max 3 rework cycles, then governance escalation
- **Governed** — every cycle logged to `.harness/factory.governance.jsonl`

### Fractal — Recursive Decomposition

The fractal pipeline (`fractal.yml`) handles tasks too complex for a single agent pass. Deterministic algorithms handle structure; AI handles semantics:

```
agent: decompose  →  run: schedule (DAG sort)  →  fan: solve leaves (parallel)
                                                         │
                                                    agent: reunify  →  run: prune
```

The algorithmic spine (`synodic fractal complexity|schedule|reunify|prune`) is deterministic — TF-IDF orthogonality, topological sort, greedy set cover. AI is invoked only for semantic work.

Set `solve_mode: factory` to run each leaf through the full Factory pipeline.

## Governance

All agent operations follow the [Harness governance protocol](./HARNESS.md):

- **Layer 1 — Static rules** (zero cost): linters, formatters, structural checks
- **Layer 2 — AI judge** (fresh context): adversarial review against spec requirements
- **Layer 3 — Human escalation**: when rework cycles are exhausted

Feedback is categorized (completeness, correctness, security, conformance, quality) and persisted to `.harness/` for cross-run analysis. Recurring patterns are crystallized into Layer 1 rules.

## Project Structure

```
synodic/
├── skills/
│   ├── factory/             # BUILD → INSPECT pipeline skill
│   ├── fractal/             # Recursive decomposition skill
│   ├── swarm/               # Speculative swarm skill
│   └── adversarial/         # Generative-adversarial skill
├── .harness/
│   ├── pipelines/           # Pipeline YAML definitions (factory, fractal, swarm, adversarial)
│   ├── gates.yml            # Preflight gate definitions
│   └── *.governance.jsonl   # Governance logs (append-only)
├── cli/                     # Rust workspace (eval framework + governance CLI)
│   ├── synodic-eval/        # Standalone eval framework (SWE-bench, FeatureBench, DevBench)
│   └── synodic/             # Governance CLI (harness + eval integration)
├── specs/                   # Actionable requirements (LeanSpec format)
├── docs/                    # Project documentation
│   ├── architecture/
│   │   └── scope-review.md      # Scope review — rationale for skill-first approach
│   ├── design/
│   │   └── fractal-design.md    # Fractal design — algorithmic spine, protocol, CLI reference
│   └── evaluation/
│       └── evaluation-strategy.md # Evaluation strategy — SWE-bench Pro, FeatureBench, DevBench
├── evals/                   # Evaluation tasks and benchmarks
├── .harness/                # Governance infrastructure and logs
├── .lean-spec/              # LeanSpec configuration
├── HARNESS.md               # Governance protocol
├── CLAUDE.md                # Claude Code project instructions
└── AGENTS.md                # AI agent instructions
```

## Documentation

| Document | Description |
|----------|-------------|
| [factory-design.md](./docs/design/factory-design.md) | Factory skill design — BUILD → INSPECT pipeline, adversarial review, metrics |
| [fractal-design.md](./docs/design/fractal-design.md) | Fractal decomposition design — algorithmic spine, 8-step protocol, CLI reference |
| [scope-review.md](./docs/architecture/scope-review.md) | March 2026 scope review — rationale for the skill-first pivot |
| [evaluation-strategy.md](./docs/evaluation/evaluation-strategy.md) | Evaluation strategy — SWE-bench Pro as primary benchmark |
| [HARNESS.md](./HARNESS.md) | Governance protocol — evaluation model, feedback taxonomy, rule crystallization |
| [specs/](./specs/) | Actionable specifications in LeanSpec format |

## Evaluation

Synodic includes a Rust-based eval framework (`cli/synodic-eval/`) for measuring agent coding performance:

```bash
cd cli && cargo build          # build eval framework
cd cli && cargo test           # run all tests
```

**Supported benchmarks:**
- **SWE-bench Pro** — 1,865 real-world GitHub issues (primary benchmark)
- **FeatureBench** — Feature implementation from PRDs
- **DevBench** — Full-project development from requirements

See [evaluation-strategy.md](./docs/evaluation/evaluation-strategy.md) for the full evaluation strategy.

## License

MIT
