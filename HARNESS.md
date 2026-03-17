---
version: 0.1.0
last_amended: 2026-03-17
changelog:
  - "0.1.0: Initial governance protocol"
---

# Harness Governance Protocol

## §1 — Preamble

This document is the governance protocol for all Synodic agent operations.
It is mandatory reading for any skill or agent operating within this repository.
It defines the control plane that is orthogonal to execution topologies.

**"Code what you can. Judge what you must. Escalate what you can't."**

## §2 — Three-Layer Evaluation Model

Every governance checkpoint uses this evaluation chain. Resolve at the lowest
layer possible. Never invoke Layer 2 for what Layer 1 can catch. Never invoke
Layer 3 for what Layer 2 can decide.

### Layer 1 — Static Rules

Deterministic, zero AI cost, microsecond latency.

- Schema validation (types, required fields, contracts)
- Lint and style enforcement (clippy, eslint, ruff — per-language)
- Dependency boundary checks (no illegal cross-layer imports)
- Crystallized rules from `.harness/rules/`
- Any check expressible as a pure function of the code diff

### Layer 2 — AI Judge

Independent LLM, ~1s latency.

- Semantic review: does the output match the intent?
- Adversarial inspection: logic flaws, security issues, spec violations
- MUST use a separate subagent from the producer (context isolation)
- Judge prompt MUST reference applicable sections of this document

### Layer 3 — Human Escalation

Async, minutes-to-hours latency.

- Triggered when: AI judge confidence is low, operation is destructive +
  irreversible, or rework limit is exhausted
- Implementation: pause execution, persist thread state, surface to user
  with full context
- Human decision is final for that run but becomes input to future
  crystallization

## §3 — Governance Checkpoint Protocol

Universal interface for all checkpoints, regardless of skill or topology:

```
evaluate(action, context, state) → Allow | Deny(reason) | Modify(revised_action)
```

- `action`: what is about to happen (code diff, decomposition plan, merge result)
- `context`: the spec, parent scope, sibling outputs, prior feedback
- `state`: which phase of which topology is requesting evaluation
- `Allow`: proceed as-is
- `Deny(reason)`: reject with explanation; caller decides rework or escalate
- `Modify(revised_action)`: proceed with corrected version

This protocol is the SAME regardless of topology. Factory's INSPECT, Fractal's
DECOMPOSE GATE, and any future topology's quality gates all use this interface.

## §4 — Checkpoint Placement Rules

Every execution topology MUST place checkpoints at these points:

1. **Post-production checkpoint.** After any subagent produces output (code,
   plan, merge result), before that output is used downstream.
2. **Pre-delivery checkpoint.** Before any output leaves the system (PR
   creation, final output). Last line of defense.
3. **Rework-limit checkpoint.** When rework attempts are exhausted, escalate
   rather than loop forever.

Default rework limits (overridable per-skill via config):

- Linear topologies (Factory-like): max 3 rework cycles at the review stage
- Tree topologies (Fractal-like): max 1 rework cycle per node

Skills MUST document their checkpoints and Layer assignments in their SKILL.md
header.

## §5 — Structured Feedback Protocol

All governance feedback MUST be:

1. **Categorized** — tagged with a category from the taxonomy below
2. **Actionable** — specific enough that the rework agent knows what to fix
3. **Recorded** — written to the manifest AND the persistent governance log

### Category Taxonomy

**Code-producing checkpoints (Factory BUILD, Fractal SOLVE):**

- `[completeness]` — missing functionality described in spec
- `[correctness]` — logic errors, bugs, off-by-one
- `[security]` — injection, secrets, unsafe operations
- `[conformance]` — doesn't match spec intent (works but wrong approach)
- `[quality]` — maintainability, structure, naming

**Decomposition checkpoints (Fractal DECOMPOSE):**

- `[orthogonality]` — scope overlap between children
- `[coverage]` — parent requirements not covered
- `[granularity]` — too fine or too coarse
- `[budget]` — node/depth budget pressure

**Integration checkpoints (Fractal REUNIFY):**

- `[interface]` — contract mismatches between components
- `[boundary]` — component exceeded its declared scope
- `[redundancy]` — duplicate solutions across components
- `[gap]` — missing integration glue

Skills MAY extend this taxonomy but MUST NOT redefine existing categories.

## §6 — Persistence and Learning

### Tier 1 — Manifest (per-run, local)

- Location: `.factory/{work-id}/manifest.json` or `.fractal/{work-id}/manifest.json`
- Content: full run state, attempt history, checkpoint results
- Lifecycle: created at run start, finalized at run end, gitignored
- Purpose: operational state for the current run

### Tier 2 — GovernanceLog (cross-run, persistent)

- Location: `.harness/factory.governance.jsonl` and `.harness/fractal.governance.jsonl`
- Content: one summary record per run (status, categorized feedback, metrics)
- Lifecycle: append-only, version-controlled (NOT gitignored)
- Purpose: learning substrate for rule crystallization

Both tiers are required. A skill that runs governance checkpoints but does not
persist results is non-compliant.

## §7 — Rule Crystallization

Governance feedback becomes static rules through this lifecycle:

```
Layer 2/3 feedback → GovernanceLog → Pattern Detection → Rule Synthesis → Backtest → Human Review → Layer 1 rule
```

- **Pattern Detection**: aggregate GovernanceLog entries by category +
  description similarity. A pattern is "crystallization-ready" when it
  appears in ≥3 independent runs.
- **Rule Synthesis**: generate a deterministic check (script or config)
  that catches the pattern without AI.
- **Backtest**: replay the candidate rule against historical GovernanceLog
  entries. Require >90% precision (few false positives) and >70% recall.
- **Human Review**: candidate rules are surfaced as PRs. A human approves
  or rejects.
- **Deployment**: approved rules go into `.harness/rules/` and are
  automatically loaded by Layer 1 in all topologies.

This process is not yet automated — documented protocol for manual execution
now, with hooks for future automation.

## §8 — Cross-Topology Shared Infrastructure

| Resource            | Location               | Shared by                        |
|---------------------|------------------------|----------------------------------|
| Static rules        | `.harness/rules/`      | All topologies (Layer 1)         |
| GovernanceLog       | `.harness/*.governance.jsonl` | All topologies (separate files, shared schema) |
| Category taxonomy   | This document (§5)     | All checkpoints in all topologies |
| Checkpoint protocol | This document (§3)     | All governance checkpoints       |

`.harness/rules/` is the canonical location for crystallized rules. Rules
apply to all topologies regardless of which topology first triggered their
crystallization.

## §9 — Compliance Checklist for New Skills

Any new execution topology skill added to Synodic MUST:

- [ ] Reference `HARNESS.md` in its SKILL.md header
- [ ] Place governance checkpoints per §4 (post-production, pre-delivery, rework-limit)
- [ ] Use the `evaluate()` protocol from §3 at each checkpoint
- [ ] Categorize all feedback per §5 taxonomy
- [ ] Persist to both Manifest (Tier 1) and GovernanceLog (Tier 2) per §6
- [ ] Load and apply rules from `.harness/rules/` in its Layer 1 checks
- [ ] Document its checkpoint map in SKILL.md

## §10 — Amending This Document

This document is versioned (see frontmatter). It evolves through two mechanisms:

### Crystallization-driven amendments

When `.harness/*.governance.jsonl` accumulates feedback that cannot be classified
under the existing §5 taxonomy, a new category MAY be proposed:

1. Open an issue titled `harness: propose category [{new-category}]` with:
   - ≥3 governance log entries that demonstrate the gap
   - Proposed category name and definition
   - Which checkpoint type it applies to (code-producing / decomposition / integration)
2. PR the taxonomy change to HARNESS.md with version bump (minor).
3. PR MUST be human-reviewed. Agents MUST NOT auto-merge HARNESS.md changes.

### Protocol-level amendments

Changes to §2 (evaluation model), §3 (checkpoint protocol), or §4 (placement rules)
are breaking changes (major version bump). These require:

1. A spec in `specs/` describing the rationale and migration path.
2. All existing skills must be updated to comply before merge.
3. Explicit human approval.

### Non-breaking changes

Clarifications, examples, and wording improvements (patch) can be merged via
normal PR process but MUST still bump the version.

## §11 — Template and Portability

This document is designed to be portable across projects.

When adopting HARNESS.md in a new project:

1. Copy this file to the project root.
2. Create `.harness/` with `rules/`, `scripts/`, and `templates/` subdirectories.
3. Customize §5 taxonomy if the project's domain requires additional categories.
4. Existing categories MUST NOT be redefined — only extended.
5. Update `AGENTS.md` (or equivalent) to reference HARNESS.md.

The governance protocol (§2–§4) is project-agnostic. The taxonomy (§5) has a
universal base that projects extend. The persistence model (§6) and crystallization
process (§7) are implementation-specific and may be adapted.
