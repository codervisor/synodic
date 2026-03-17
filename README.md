# Synodic — AI Coding Factory

> *synodic* (adj.) — from Greek *synodos*, "meeting, conjunction." The period when orbiting bodies align into the same configuration.

**Synodic** is a skill package for [Claude Code](https://docs.anthropic.com/en/docs/claude-code) that implements structured AI coding workflows — transforming specs into reviewed PRs via adversarial BUILD → INSPECT pipelines.

## Why Synodic?

A single AI agent can write code, but it can't objectively review its own output. Synodic implements a **factory model**: one agent builds, a separate agent inspects with fresh context (no builder bias). This adversarial review catches bugs and spec violations that self-review misses.

The core thesis: **adversarial review by a separate agent instance produces measurably better results than a single agent run, with acceptable overhead.**

## Skills

| Skill | Description | Usage |
|-------|-------------|-------|
| **factory** | Transforms a spec into a reviewed PR via BUILD → INSPECT pipeline | `/factory run <spec-path>` |
| **fractal** | Recursively splits complex tasks into sub-specs, solves leaves independently, reunifies bottom-up | `/fractal decompose <task-or-spec-path>` |

### Factory Pipeline

```
/factory run specs/044-factory-skill-mvp/README.md

Main conversation (orchestrator)
  │
  ├─→ BUILD subagent (worktree-isolated)
  │     • Reads spec, implements code
  │     • Runs tests, commits to branch
  │
  ├─→ INSPECT subagent (fresh context, adversarial)
  │     • Reviews diff against spec
  │     • Returns APPROVE or REWORK with items
  │
  └─→ Orchestration loop (max 3 cycles)
        • REWORK → re-invoke BUILD with feedback
        • APPROVE → create PR via gh
```

## Governance

All agent operations are governed by [HARNESS.md](./HARNESS.md) — a three-layer evaluation model (static rules → AI judge → human escalation) with structured feedback and rule crystallization.

## Project Structure

```
synodic/
├── skills/
│   ├── factory/          # BUILD → INSPECT pipeline skill
│   └── fractal/          # Recursive decomposition skill
├── specs/                # Actionable requirements (LeanSpec format)
├── docs/                 # Project documentation and guidance
├── .harness/             # Governance infrastructure
├── .lean-spec/           # LeanSpec configuration
├── HARNESS.md            # Governance protocol
└── AGENTS.md             # AI agent instructions
```

## Status

Active development. The factory and fractal skills are functional. See [`specs/`](./specs/) for design specifications and [`docs/`](./docs/) for project documentation.

## License

MIT
