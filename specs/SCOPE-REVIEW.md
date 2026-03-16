# Synodic Scope Review — March 2026

## Problem Statement

Synodic has 43 specs (~8,140 lines of design) and ~4 commits of implementation.
The spec-to-code ratio is unsustainable. We need to trim scope, archive premature
specs, and focus on proving the core thesis before designing the cathedral.

## Key Insight: Claude Code + Skills Already Covers 80%

Before deciding what to build, we asked: **is an AI coding tool like Claude Code
with proper skills setup enough to enable factory-style coding delivery?**

The answer is mostly yes:

| Factory Capability | Claude Code + Skills | Needs Orchestration? |
|--------------------|---------------------|---------------------|
| Read spec, write code | Built-in | No |
| Run tests, fix failures | Built-in (iterates naturally) | No |
| Quality gates (lint, type-check) | Pre-commit hooks, CI | No |
| Multi-step workflows | Skills define sequences | No |
| Self-review | `/simplify` skill exists | Weak — self-review has blind spots |
| **Independent review** | **Needs second agent** | **Yes** |
| **Parallel execution** | **One instance = one thread** | **Yes** |
| **Cost-aware routing** | **Can't mix models in-session** | **Yes** |
| **Continuous operation** | **Sessions end** | **Yes — but this is infra, not orchestration** |

The real gaps are narrow: adversarial review, parallelism, and cost routing.
These don't require a platform — they require a skill that can spawn a second
Claude Code instance.

## Decision: Skill-First Approach

**Synodic becomes a skill package, not a standalone binary.**

The Rust codebase (`crates/syn-cli`, `syn-engine`, `syn-types`) and the Node.js
platform wrapper (`packages/cli`) are unnecessary overhead. A skill can instruct
Claude Code to:

1. Read a spec (skill context)
2. Create a branch and implement (Claude Code's core job)
3. Spawn a separate `claude` process for independent review (shell command)
4. Parse the review verdict and loop on rework (skill logic)
5. Record metrics to a JSON manifest (file I/O)
6. Create a PR (gh CLI)

**What changes:**

| Before | After |
|--------|-------|
| Rust binary orchestrating Claude Code | Claude Code skill orchestrating Claude Code |
| npm platform packages for distribution | `npx skills add` for distribution |
| Custom message bus, state persistence | File-based manifests, git branches |
| 43 specs across 5 architectural layers | Skills + minimal supporting specs |

## Revised Core Thesis

> A Claude Code skill that implements BUILD → INSPECT with adversarial review
> (separate agent instance) produces measurably better results than a single
> agent run, with acceptable overhead.

## What to Build

### Phase 1 — Factory Skill (the MVP)

**One skill: `factory`**

```
/factory run specs/038-factory-mvp/README.md
```

The skill:
1. Reads the target spec
2. Creates a feature branch (`factory/{work-id}`)
3. Implements the spec (BUILD — Claude Code does this natively)
4. Spawns a second `claude` instance in review mode (INSPECT)
5. Parses verdict: APPROVE or REWORK with specific items
6. If REWORK: applies feedback, re-runs review (max 3 loops)
7. Records metrics: cycle time, tokens, rework count, first-pass yield
8. On APPROVE: creates PR via `gh`

**Skill structure:**
```
skills/factory/
├── SKILL.md              # Skill definition (AgentSkills.io format)
├── references/
│   └── manifest.schema.json   # Work manifest schema
├── fixtures/
│   └── sample-spec/           # Test spec for validation
└── evals/
    ├── evals.json             # Behavioral evals
    └── prompts/               # Eval prompts
```

**Exit criteria:**
- Skill runs end-to-end on a real spec
- Independent review catches deliberate bugs
- Rework loop fires and fixes issues
- Metrics are recorded to `.factory/{work-id}/manifest.json`
- At least 3 real specs processed successfully

### Phase 2 — Parallel Execution (if Phase 1 validates)

**Extend the skill to handle multiple specs concurrently:**
- Accept a list of specs or a directory
- Spawn independent `claude` instances per spec
- Aggregate metrics across the batch
- Report throughput (specs/hour)

### Phase 3 — Cost Routing (if Phase 2 validates)

- Route simple tasks to cheaper models (Haiku for boilerplate, Opus for design)
- Measure cost-per-spec vs quality tradeoff

---

## Spec Disposition

### KEEP (Rewrite for skill-first)

| # | Spec | Action |
|---|------|--------|
| 037 | Coding Factory Vision | Rewrite as skill-first vision. Trim to phases 1-2. |
| 038 | Factory MVP | Rewrite as skill spec, not Rust binary spec. |

### DEFER (After skill MVP validates)

| # | Spec | Why Defer |
|---|------|-----------|
| 001 | Workspace Persistence | Useful for cross-session memory, not needed for MVP |
| 036 | Competitive Analysis | Reference material, no action needed |
| 039 | Assembly Line Abstraction | Extract patterns from working skill, don't pre-design |
| 040 | Quality System | After basic pipeline works |
| 041 | Metrics Dashboard | After metrics are being collected |

### ARCHIVE (No longer relevant to skill-first approach)

**All 36 remaining specs.** The entire fleet execution layer (002-010), coordination
theory (011-035), and advanced factory features (042-043) were designed for a
platform architecture that we're not building.

| Range | Category | Count |
|-------|----------|-------|
| 002-006 | Fleet Execution | 5 |
| 007-010 | Auth & RBAC | 4 |
| 011-035 | Coordination Theory & Primitives | 25 |
| 042-043 | Advanced Factory | 2 |
| **Total** | | **36** |

The existing `coordination-model` skill (in spec 017) can remain as a standalone
skill if its concepts prove useful — but it's not a prerequisite for the factory
skill.

### DELETE (Dead infrastructure)

With the skill-first approach, the following become unnecessary:

| Path | Why |
|------|-----|
| `crates/` | Rust binary no longer needed |
| `packages/cli/` | npm platform wrapper no longer needed |
| `scripts/` | Publishing scripts for binary distribution |
| `.github/workflows/publish.yml` | Binary publishing pipeline |
| `publish.config.ts` | Forge publishing config |
| `Cargo.toml`, `Cargo.lock` | Rust workspace |

**Keep:** `.github/workflows/ci.yml` (adapt for skill validation), `.lean-spec/`,
`AGENTS.md`, `package.json` (simplify).

---

## Resulting Project Shape

```
synodic/
├── skills/
│   └── factory/
│       ├── SKILL.md
│       ├── references/
│       ├── fixtures/
│       └── evals/
├── specs/
│   ├── 037-coding-factory-vision/    # Rewritten
│   ├── 038-factory-mvp/              # Rewritten
│   └── (archived specs remain)
├── .github/workflows/ci.yml          # Skill validation
├── .lean-spec/
├── AGENTS.md
├── README.md                          # Rewritten
└── package.json                       # Simplified
```

**From: Rust+Node.js hybrid platform with 43 specs**
**To: One skill with 2 active specs and behavioral evals**

---

## Recommended Next Steps

1. **Write the `factory` SKILL.md** — Define the skill in AgentSkills.io format
2. **Create manifest schema** — JSON schema for work item tracking
3. **Build evals** — Behavioral evals that verify the skill works end-to-end
4. **Test on real specs** — Run the skill on 3-5 existing specs in this repo
5. **Measure** — Compare factory-produced code vs single-agent code on same tasks
6. **Decide on Rust cleanup** — Remove or archive the binary infrastructure

---

## Risks

**Skill-first risks:**
- Claude Code skills may lack the control flow needed for multi-step orchestration
- Spawning a second `claude` instance from within a skill may have limitations
- Skill format may not support the complexity of factory workflows

**Mitigations:**
- Test skill capabilities early (Phase 1 is the validation)
- If skills hit a wall, a thin shell script orchestrator is the fallback — still not a Rust platform
- Keep archived specs as reference if we need to escalate complexity later
