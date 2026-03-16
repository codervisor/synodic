# Synodic Scope Review — March 2026

## Problem Statement

Synodic has 43 specs (~8,140 lines of design) and ~4 commits of implementation.
The spec-to-code ratio is unsustainable. We need to trim scope, archive premature
specs, and focus on proving the core thesis before designing the cathedral.

## Key Insight: Claude Code Already Has Orchestration Built In

As of March 2026, Claude Code ships with multi-agent orchestration features that
overlap heavily with what Synodic was designing:

| Synodic Concept | Claude Code Built-in | Status |
|----------------|---------------------|--------|
| BUILD station | Core agent loop | Production |
| INSPECT station | `/simplify` (3 parallel review agents) | Production |
| Rework loops | Native test-fix iteration | Production |
| Parallel builds | `/batch` (5-30 worktree agents) | Production |
| Fleet coordination | Agent Teams (lead + N teammates) | Experimental |
| Process supervisor | Built into Agent Teams | Experimental |
| Message bus | Agent Teams mailbox + shared task list | Experimental |
| Cost routing | `--model` flag per spawned instance | Production |
| Metrics capture | `--output-format json` (tokens, turns) | Production |
| Subprocess mgmt | Subagents (Explore, Plan, general-purpose) | Production |
| Git isolation | `claude -w branch` (worktree sessions) | Production |

### What Claude Code Still Lacks (Synodic's Real Niche)

The gaps that remain are narrow but real:

1. **Assembly line semantics** — `/batch` is parallel, not sequential-staged.
   No concept of "this unit must pass BUILD → INSPECT → HARDEN in order."
2. **Adversarial review by design** — `/simplify` reviews within the same
   context. The builder's blind spots leak through. True adversarial review
   requires a separate agent with no builder context.
3. **Spec-to-PR pipeline** — No "give me a spec, produce a PR" one-shot
   workflow with structured verdict protocol.
4. **Cross-session metrics** — Each session tracks its own tokens, but no
   unified metrics across a factory batch.
5. **Structured rework protocol** — No APPROVE/REWORK verdict parsing with
   specific rework items routed back to the builder.

## Decision: Skill-First Approach

**Synodic becomes a skill package, not a standalone binary.**

The Rust codebase and Node.js wrapper are unnecessary — Claude Code already has
subprocess management, worktree isolation, parallel agents, and structured output.
Synodic's value is the **workflow definition**, not the runtime.

A skill composes existing Claude Code primitives:

1. Read a spec (skill context)
2. Create a branch and implement (Claude Code's core loop)
3. Spawn reviewer via `claude -p --output-format json` (adversarial review)
4. Parse structured verdict and route rework (skill protocol)
5. Record metrics to JSON manifest (file I/O)
6. Create PR via `gh` (native CLI)

For parallel execution, compose with `/batch` or Agent Teams rather than
reimplementing subprocess management.

**What changes:**

| Before | After |
|--------|-------|
| Rust binary orchestrating Claude Code | Claude Code skill composing built-in primitives |
| npm platform packages for distribution | `npx skills add` for distribution |
| Custom message bus, state persistence | Agent Teams mailbox + file manifests |
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

**Compose with existing Claude Code parallelism:**
- Use `/batch` for independent specs (5-30 worktree agents)
- Or Agent Teams for coordinated multi-spec work
- Aggregate metrics from `--output-format json` across sessions
- Report throughput (specs/hour)

No custom parallel execution needed — Claude Code already has this.

### Phase 3 — Cost Routing (if Phase 2 validates)

- Use `--model haiku` for simple BUILD tasks, `--model opus` for INSPECT
- Measure cost-per-spec vs quality tradeoff
- Leverage `--max-budget-usd` for per-task cost caps

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

## Claude Code Orchestration Features (March 2026 Reference)

Key CLI flags for factory skill implementation:

| Flag | Purpose |
|------|---------|
| `-p` / `--print` | Non-interactive mode (scriptable) |
| `--output-format json` | Structured output with metadata |
| `--json-schema '{...}'` | Schema-validated structured output |
| `--model sonnet\|opus\|haiku` | Per-instance model selection |
| `--max-turns N` | Limit agent iterations |
| `--max-budget-usd N` | Cost cap per session |
| `--allowedTools "..."` | Tool whitelist per instance |
| `-w branch` | Worktree isolation |
| `--append-system-prompt` | Inject reviewer/builder role |
| `-r session-id` | Resume specific session |

Key built-in skills to compose with:

| Skill | What It Does |
|-------|-------------|
| `/batch` | Parallel worktree agents (5-30 units) |
| `/simplify` | 3-agent parallel code review |
| `/loop` | Recurring execution on interval |

Agent Teams (experimental, `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`):
- Lead + N teammates with shared task list
- Mailbox for direct inter-agent messaging
- Each teammate has independent context

## Risks

**Skill-first risks:**
- Skills cannot spawn subagents directly — must use Bash to invoke `claude -p`
- Agent Teams is experimental — API may change
- Skill format may not support complex control flow (conditional rework routing)

**Mitigations:**
- `claude -p --output-format json` is production-stable for scripted orchestration
- If Agent Teams changes, fall back to direct `claude -p` subprocess spawning
- Keep archived specs as reference if we need to escalate complexity later
- Thin shell script wrapper is always a fallback — still not a Rust platform
