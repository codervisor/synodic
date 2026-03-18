---
version: 0.2.0
last_amended: 2026-03-18
changelog:
  - "0.2.0: Refactor to post-session governance model — review after session, not inline"
  - "0.1.0: Initial governance protocol"
---

# Harness Governance Protocol

## §1 — Preamble

This document is the governance protocol for all Synodic agent operations.
It is mandatory reading for any skill or agent operating within this repository.
It defines the control plane that is orthogonal to execution topologies.

**"Observe what happened. Judge the output. Learn for next time."**

Governance is **non-intrusive**. It does not wrap, interrupt, or inject feedback
into agent sessions. Instead, it runs **after** a session completes, analyzing
the output (diffs, logs) and producing governance records that feed into rule
crystallization.

## §2 — Two-Layer Evaluation Model

Every governance review uses this evaluation chain. Resolve at the lowest
layer possible. Never invoke Layer 2 for what Layer 1 can catch.

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
- MUST use a separate context from the producer (context isolation)
- Judge prompt MUST reference applicable sections of this document

### Human Review

Governance logs surface issues for human attention when needed. Humans review
governance reports at their own pace — there is no automated escalation or
blocking. Governance findings inform future sessions, not the current one.

## §3 — Governance Review Protocol

Post-session review interface:

```
review(diff, context) → Clean | Issues(items)
```

- `diff`: the git diff between the session's base ref and HEAD
- `context`: the spec being implemented, governance rules in effect
- `Clean`: no issues found — session output is acceptable
- `Issues(items)`: categorized findings recorded for learning

This protocol is the SAME regardless of topology. Factory, Fractal, and any
future topology's post-session reviews all use this interface.

## §4 — Review Placement

Governance reviews happen at these points:

1. **Post-session review.** After any agent session completes, review the
   diff against the base ref. This is the primary governance mechanism.
2. **Pre-merge review.** Before a PR is merged, governance review confirms
   the final state. This is the last line of defense.
3. **Periodic review.** Governance logs are periodically analyzed for
   patterns that should be crystallized into Layer 1 rules.

## §5 — Structured Feedback Protocol

All governance feedback MUST be:

1. **Categorized** — tagged with a category from the taxonomy below
2. **Actionable** — specific enough that a developer knows what to fix
3. **Recorded** — written to the persistent governance log

### Category Taxonomy

**Code-producing reviews (Factory BUILD, Fractal SOLVE):**

- `[completeness]` — missing functionality described in spec
- `[correctness]` — logic errors, bugs, off-by-one
- `[security]` — injection, secrets, unsafe operations
- `[conformance]` — doesn't match spec intent (works but wrong approach)
- `[quality]` — maintainability, structure, naming

**Decomposition reviews (Fractal DECOMPOSE):**

- `[orthogonality]` — scope overlap between children
- `[coverage]` — parent requirements not covered
- `[granularity]` — too fine or too coarse
- `[budget]` — node/depth budget pressure

**Integration reviews (Fractal REUNIFY):**

- `[interface]` — contract mismatches between components
- `[boundary]` — component exceeded its declared scope
- `[redundancy]` — duplicate solutions across components
- `[gap]` — missing integration glue

Skills MAY extend this taxonomy but MUST NOT redefine existing categories.

## §6 — Persistence and Learning

### Tier 1 — Manifest (per-review, local)

- Location: `.harness/.runs/{review-id}/manifest.json`
- Content: review results, L1/L2 reports, diff snapshot
- Lifecycle: created at review start, finalized at review end, gitignored
- Purpose: detailed record of a single review

### Tier 2 — GovernanceLog (cross-review, persistent)

- Location: `.harness/harness.governance.jsonl` (and per-topology variants)
- Content: one summary record per review (status, categorized issues, metrics)
- Lifecycle: append-only, version-controlled (NOT gitignored)
- Purpose: learning substrate for rule crystallization

Both tiers are required. A skill that runs governance reviews but does not
persist results is non-compliant.

## §7 — Rule Crystallization

Governance feedback becomes static rules through this lifecycle:

```
Layer 2 feedback → GovernanceLog → Pattern Detection → Rule Synthesis → Backtest → Human Review → Layer 1 rule
```

- **Pattern Detection**: aggregate GovernanceLog entries by category +
  description similarity. A pattern is "crystallization-ready" when it
  appears in ≥3 independent reviews.
- **Rule Synthesis**: generate a deterministic check (script or config)
  that catches the pattern without AI.
- **Backtest**: replay the candidate rule against historical GovernanceLog
  entries. Require >90% precision (few false positives) and >70% recall.
- **Human Review**: candidate rules are surfaced as PRs. A human approves
  or rejects.
- **Deployment**: approved rules go into `.harness/rules/` and are
  automatically loaded by Layer 1 in all reviews.

This process is not yet automated — documented protocol for manual execution
now, with hooks for future automation.

## §8 — Cross-Topology Shared Infrastructure

| Resource            | Location               | Shared by                        |
|---------------------|------------------------|----------------------------------|
| Static rules        | `.harness/rules/`      | All topologies (Layer 1)         |
| GovernanceLog       | `.harness/*.governance.jsonl` | All topologies (separate files, shared schema) |
| Category taxonomy   | This document (§5)     | All reviews in all topologies    |
| Review protocol     | This document (§3)     | All governance reviews           |

`.harness/rules/` is the canonical location for crystallized rules. Rules
apply to all topologies regardless of which topology first triggered their
crystallization.

## §9 — Compliance Checklist for New Skills

Any new execution topology skill added to Synodic MUST:

- [ ] Reference `HARNESS.md` in its SKILL.md header
- [ ] Run post-session governance review after execution completes
- [ ] Use the `review()` protocol from §3
- [ ] Categorize all feedback per §5 taxonomy
- [ ] Persist to both Manifest (Tier 1) and GovernanceLog (Tier 2) per §6
- [ ] Load and apply rules from `.harness/rules/` in its Layer 1 checks
- [ ] Document its review integration in SKILL.md

## §10 — Amending This Document

This document is versioned (see frontmatter). It evolves through two mechanisms:

### Crystallization-driven amendments

When `.harness/*.governance.jsonl` accumulates feedback that cannot be classified
under the existing §5 taxonomy, a new category MAY be proposed:

1. Open an issue titled `harness: propose category [{new-category}]` with:
   - ≥3 governance log entries that demonstrate the gap
   - Proposed category name and definition
   - Which review type it applies to (code-producing / decomposition / integration)
2. PR the taxonomy change to HARNESS.md with version bump (minor).
3. PR MUST be human-reviewed. Agents MUST NOT auto-merge HARNESS.md changes.

### Protocol-level amendments

Changes to §2 (evaluation model), §3 (review protocol), or §4 (review placement)
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
