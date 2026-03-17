# Embed Harness Core Principles into Synodic

## Context

Synodic is an AI-native agent orchestration platform. It currently has two execution topology skills — Factory (linear pipeline) and Fractal (tree decomposition) — both of which are being upgraded with governance checkpoints, classified feedback, and persistent learning.

The governance logic is currently scattered: Factory defines its own STATIC GATE and INSPECT protocol, Fractal defines its own DECOMPOSE GATE and REUNIFY REWORK. As more topologies are added (swarm, pipeline, etc.), this duplication will grow.

## Goal

Create a `HARNESS.md` at the project root that serves as the **single source of truth** for Synodic’s governance principles and protocols. All skills reference this document instead of independently defining governance behavior. This is NOT a skill — it’s a project-level foundational document, similar in authority to `AGENTS.md`.

## File: `HARNESS.md`

Create this file at the repo root with the following content and structure. The writing style should be terse, technical, and imperative — this is a protocol spec, not a blog post. Agent-readable means: unambiguous, structured, parseable.

### Required Sections

#### 1. Preamble

State what this document is:

- The governance protocol for all Synodic agent operations
- Mandatory reading for any skill or agent operating within this repo
- Defines the control plane that is orthogonal to execution topologies

One-line philosophy: **“Code what you can. Judge what you must. Escalate what you can’t.”**

#### 2. Three-Layer Evaluation Model

Define the evaluation chain that every governance checkpoint uses. Be precise:

**Layer 1 — Static Rules (deterministic, zero AI cost, μs latency)**

- Schema validation (types, required fields, contracts)
- Lint and style enforcement (clippy, eslint, ruff — per-language)
- Dependency boundary checks (no illegal cross-layer imports)
- Crystallized rules from `.factory/rules/` directory
- Any check that can be expressed as a pure function of the code diff

**Layer 2 — AI Judge (independent LLM, ~1s latency)**

- Semantic review: does the output match the intent?
- Adversarial inspection: are there logic flaws, security issues, or spec violations?
- MUST use a separate subagent from the one that produced the work (context isolation)
- Judge prompt MUST reference applicable sections of this document and GOLDEN_PRINCIPLES if they exist

**Layer 3 — Human Escalation (async, minutes-to-hours latency)**

- Triggered when: AI judge confidence is low, operation is destructive + irreversible, or rework limit is exhausted
- Implementation: pause execution, persist thread state, surface to user with full context
- Human decision is final for that run but becomes input to future crystallization

State the core routing principle: **“Resolve at the lowest layer possible. Never invoke Layer 2 for what Layer 1 can catch. Never invoke Layer 3 for what Layer 2 can decide.”**

#### 3. Governance Checkpoint Protocol

Define the universal interface that all checkpoints use, regardless of which skill or topology invokes them:

```
evaluate(action, context, state) → Allow | Deny(reason) | Modify(revised_action)
```

Specify:

- `action`: what is about to happen (a code diff, a decomposition plan, a merge result)
- `context`: the spec, parent scope, sibling outputs, and any prior feedback
- `state`: which phase of which topology is requesting evaluation
- `Allow`: proceed as-is
- `Deny(reason)`: reject with explanation; caller decides whether to rework or escalate
- `Modify(revised_action)`: proceed with a corrected version (e.g., DECOMPOSE GATE revises a split plan)

State that the checkpoint protocol is the SAME regardless of topology. Factory’s INSPECT, Fractal’s DECOMPOSE GATE, and any future topology’s quality gates all use this interface.

#### 4. Checkpoint Placement Rules

Define where checkpoints MUST be placed in any execution topology:

1. **Post-production checkpoint**: After any subagent produces output (code, plan, merge result), before that output is used downstream. This catches errors at the source.
1. **Pre-delivery checkpoint**: Before any output leaves the system (PR creation, final output). This is the last line of defense.
1. **Rework-limit checkpoint**: When rework attempts are exhausted, escalate rather than loop forever. Define default limits:
- Linear topologies (Factory-like): max 3 rework cycles at the review stage
- Tree topologies (Fractal-like): max 1 rework cycle per node (cost compounds exponentially with tree size)
- These defaults can be overridden per-skill via config

State that a skill MUST document its checkpoints and their Layer assignments in its SKILL.md header.

#### 5. Structured Feedback Protocol

All governance feedback (rework items, conflict reports, decomposition flags) MUST be:

1. **Categorized** — each item tagged with a category from a defined taxonomy
1. **Actionable** — specific enough that the rework agent knows exactly what to fix
1. **Recorded** — written to the manifest AND to the persistent governance log

Define the universal category taxonomy:

**For code-producing checkpoints (Factory BUILD, Fractal SOLVE):**

- `[completeness]` — missing functionality described in spec
- `[correctness]` — logic errors, bugs, off-by-one
- `[security]` — injection, secrets, unsafe operations
- `[conformance]` — doesn’t match spec intent (works but wrong approach)
- `[quality]` — maintainability, structure, naming

**For decomposition checkpoints (Fractal DECOMPOSE):**

- `[orthogonality]` — scope overlap between children
- `[coverage]` — parent requirements not covered
- `[granularity]` — too fine or too coarse
- `[budget]` — node/depth budget pressure

**For integration checkpoints (Fractal REUNIFY):**

- `[interface]` — contract mismatches between components
- `[boundary]` — component exceeded its declared scope
- `[redundancy]` — duplicate solutions across components
- `[gap]` — missing integration glue

Skills may extend this taxonomy but MUST NOT redefine existing categories.

#### 6. Persistence and Learning

Define the two-tier persistence model:

**Tier 1 — Manifest (per-run, local)**

- Location: `.factory/{work-id}/manifest.json` or `.fractal/{work-id}/manifest.json`
- Content: full run state, attempt history, checkpoint results
- Lifecycle: created at run start, finalized at run end, gitignored
- Purpose: operational state for the current run

**Tier 2 — GovernanceLog (cross-run, persistent)**

- Location: `.factory/governance.jsonl` and `.fractal/governance.jsonl`
- Content: one summary record per run (status, categorized feedback, metrics)
- Lifecycle: append-only, version-controlled (NOT gitignored)
- Purpose: learning substrate for rule crystallization

State that both tiers are required. A skill that runs governance checkpoints but doesn’t persist results is non-compliant.

#### 7. Rule Crystallization

Define the lifecycle of how governance feedback becomes static rules:

```
Layer 2/3 feedback → GovernanceLog → Pattern Detection → Rule Synthesis → Backtest → Human Review → Layer 1 rule
```

Specify:

- **Pattern Detection**: aggregate GovernanceLog entries by category + description similarity. A pattern is “crystallization-ready” when it appears in ≥3 independent runs.
- **Rule Synthesis**: generate a deterministic check (script or config) that catches the pattern without AI.
- **Backtest**: replay the candidate rule against historical GovernanceLog entries. Require >90% precision (few false positives) and >70% recall.
- **Human Review**: candidate rules are surfaced as PRs. A human approves or rejects.
- **Deployment**: approved rules go into `.factory/rules/` and are automatically loaded by Layer 1 in all topologies.

Note that this process is not yet automated — it’s a documented protocol for manual execution now, with hooks for future automation.

#### 8. Cross-Topology Shared Infrastructure

Define what is shared across all execution topologies:

|Resource           |Location            |Shared by                                                     |
|-------------------|--------------------|--------------------------------------------------------------|
|Static rules       |`.factory/rules/`   |Factory STATIC GATE, Fractal SOLVE GATE, all future topologies|
|GovernanceLog      |`*.governance.jsonl`|All topologies (separate files, shared schema)                |
|Category taxonomy  |This document (§5)  |All checkpoints in all topologies                             |
|Checkpoint protocol|This document (§3)  |All governance checkpoints                                    |

State that `.factory/rules/` is the canonical location for crystallized rules even though the name contains “factory” — this is historical. Rules apply to all topologies.

Optionally: if the team prefers, rename to `.harness/rules/` for clarity. Recommend this rename but don’t require it in this PR.

#### 9. Compliance Checklist for New Skills

Any new execution topology skill added to Synodic MUST:

- [ ] Reference `HARNESS.md` in its SKILL.md header
- [ ] Place governance checkpoints per §4 (post-production, pre-delivery, rework-limit)
- [ ] Use the `evaluate()` protocol from §3 at each checkpoint
- [ ] Categorize all feedback per §5 taxonomy
- [ ] Persist to both Manifest (Tier 1) and GovernanceLog (Tier 2) per §6
- [ ] Load and apply rules from `.factory/rules/` (or `.harness/rules/`) in its Layer 1 checks
- [ ] Document its checkpoint map in SKILL.md

## Additional Changes

### Update `AGENTS.md`

Add a reference to HARNESS.md:

```markdown
## Governance

All agent operations in this repository are subject to the governance protocol
defined in [HARNESS.md](./HARNESS.md). Before executing any skill, read HARNESS.md
to understand checkpoint requirements, feedback classification, and escalation rules.
```

### Update Factory SKILL.md Header

Add at the top of the Factory skill, after the frontmatter:

```markdown
> **Governance:** This skill implements the Harness governance protocol.
> See [HARNESS.md](../../HARNESS.md) for the evaluation model, checkpoint protocol,
> and feedback taxonomy. Factory's checkpoint map:
> - Step 2.5 STATIC GATE → Layer 1
> - Step 3 INSPECT → Layer 2
> - Step 6 Escalate → Layer 3
```

### Update Fractal SKILL.md Header

Same pattern:

```markdown
> **Governance:** This skill implements the Harness governance protocol.
> See [HARNESS.md](../../HARNESS.md) for the evaluation model, checkpoint protocol,
> and feedback taxonomy. Fractal's checkpoint map:
> - Step 2.5 DECOMPOSE GATE → Layer 1
> - Step 3.5 SOLVE GATE → Layer 1 (or Layer 2 via solve_mode: factory)
> - Step 4.5 REUNIFY REWORK → Layer 2
> - Step 5 Escalate (on rework exhaustion) → Layer 3
```

### Consider Directory Rename (optional)

If desired, rename `.factory/rules/` to `.harness/rules/` and `.factory/governance.jsonl` / `.fractal/governance.jsonl` to `.harness/factory.governance.jsonl` / `.harness/fractal.governance.jsonl`. This centralizes governance artifacts:

```
.harness/
├── rules/                           # Crystallized static rules (shared)
├── factory.governance.jsonl         # Factory learning log
└── fractal.governance.jsonl         # Fractal learning log
```

This rename is recommended but optional for this PR. If deferred, add a TODO comment in HARNESS.md.

## Implementation Notes

- HARNESS.md should be concise — target 200-300 lines. It’s a protocol spec, not documentation. Every sentence should be either a definition, a rule, or a constraint.
- Use imperative mood throughout (“Skills MUST…”, “Checkpoints MUST NOT…”).
- Do not repeat content from Factory or Fractal SKILL.md files — reference them instead.
- The document should be useful to BOTH human developers reading the repo AND AI agents loading context. Optimize for parseability.

## Acceptance Criteria

- [ ] `HARNESS.md` exists at repo root with all 9 sections
- [ ] `AGENTS.md` references `HARNESS.md`
- [ ] Factory `SKILL.md` header includes governance reference and checkpoint map
- [ ] Fractal `SKILL.md` header includes governance reference and checkpoint map
- [ ] Governance artifacts directory structure is documented (either `.factory/` or `.harness/`)
- [ ] New skill compliance checklist is included in §9