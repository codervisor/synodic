# Synodic Scope Review — March 2026

## Problem Statement

Synodic has 43 specs (~8,140 lines of design) and ~4 commits of implementation.
The spec-to-code ratio is unsustainable. We need to trim scope, archive premature
specs, and focus on proving the core thesis before designing the cathedral.

## Core Thesis to Validate

> A BUILD → INSPECT pipeline that spawns Claude Code, reviews the output, and
> loops on rework actually produces better results than a single agent run.

Until this is proven with real tasks, everything else is speculative.

---

## Proposed Tiers

### Tier 1 — KEEP (Ship Now)

These specs define the immediate work. Goal: working end-to-end pipeline.

| # | Spec | Rationale |
|---|------|-----------|
| 037 | Coding Factory Vision | North star. Trim to phases 0-1 only. |
| 038 | Factory MVP — First Car | **THE milestone.** Two-station pipeline that works. |

**Exit criteria for Tier 1:**
- `synodic run specs/038-...` produces a real git branch with working code
- INSPECT station catches deliberate bugs and triggers rework
- Rework loop cap (3 attempts) is enforced
- Metrics (cycle time, tokens, first-pass yield) are recorded
- At least 3 real specs are run through the pipeline successfully

### Tier 2 — DEFER (After MVP Proves Out)

These specs are valuable but premature. Move to `status: deferred`.

| # | Spec | Why Defer |
|---|------|-----------|
| 001 | Agent Workspace Persistence | Useful but not needed for MVP validation |
| 002 | Agent Fleet Execution Layer | Multi-agent comes after single-pipeline works |
| 003 | Fleet Execution Foundation | Same — fleet implies multiple agents |
| 004 | Fleet Process Supervisor | MVP can use simple subprocess management |
| 005 | Agent Message Bus | No bus needed for two-station pipeline |
| 006 | Fleet State Persistence | File-based manifest is sufficient for now |
| 036 | Competitive Analysis | Already written, useful context, no action needed |
| 039 | Assembly Line Abstraction | Premature abstraction — extract after MVP patterns emerge |
| 040 | Factory Quality System | Design quality gates after basic pipeline works |
| 041 | Production Metrics Dashboard | Collect metrics first, dashboard later |

### Tier 3 — ARCHIVE (Premature / Over-engineered)

These specs should be archived. They represent design for problems that don't
exist yet and may never exist in their current form.

| # | Spec | Why Archive |
|---|------|-------------|
| 007-010 | Auth & RBAC (4 specs) | Identity, secrets, RBAC — zero users, zero fleet, zero need |
| 011 | Fleet Coordination & Optimization | Group spec for unvalidated patterns |
| 012 | Advanced Coordination Patterns | Patterns without implementations to pattern-match against |
| 013 | AI-Native Coordination Primitives | Theoretical framework, no grounding in practice |
| 014 | Domain Playbooks | Playbooks for a system that doesn't run yet |
| 015 | SDD AI-Native Playbook | Same |
| 016 | Nemosis Teacher-Student Distillation | Optimization before product-market fit |
| 017-021 | Coordination Model (5 specs) | Core pipeline, theory, design, validation — all theoretical |
| 022 | Visual Reference | Diagrams for unbuilt primitives |
| 023 | Roles & Limitations | Constraints on unbuilt system |
| 024-030 | Primitive Deep Dives (7 specs) | Speculative swarm, context mesh, fractal decomposition, etc. — none implemented |
| 031 | Coordination Artifacts | Lifecycle for artifacts that don't exist |
| 032-033 | Formal Theory (2 specs) | Mathematical proofs for unvalidated concepts |
| 034 | Claude Code Implementation Mapping | Mapping to unbuilt coordination model |
| 035 | Tool Capability Conformance | Conformance layer for single-tool MVP |
| 042 | Continuous Improvement Loop | A/B testing a pipeline that doesn't ship yet |
| 043 | Factory Supply Chain | Context caching for a single-station flow |

**That's 31 specs to archive.** They aren't deleted — they're preserved as
future reference if/when the project reaches a stage where they become relevant.

---

## Resulting Scope

| Tier | Specs | Action |
|------|-------|--------|
| Keep | 2 | Ship: 037, 038 |
| Defer | 10 | Revisit after MVP: 001-006, 036, 039-041 |
| Archive | 31 | Set status to `archived` |

**From 43 active specs → 2 active + 10 deferred.**

---

## Recommended Next Steps

1. **Update spec statuses** — Batch-update frontmatter to reflect tiers
2. **Trim spec 037** — Remove phases 2-4 detail, keep as concise north star
3. **Implement spec 038** — The only spec that matters right now:
   - Get `synodic run` working end-to-end
   - Write real tests
   - Run the pipeline on itself (dog-food)
4. **Delete dead dependency references** — Specs reference `clawden:*` deps that no longer exist
5. **Validate thesis** — Run 3-5 real tasks through the pipeline, measure results
6. **Decide on fleet** — Only after single-pipeline value is proven, revisit Tier 2 specs

---

## Risks of NOT Trimming

- **Analysis paralysis**: More specs → more "prerequisites" → nothing ships
- **Premature abstraction**: Building coordination primitives before knowing what coordination is needed
- **Competitive loss**: Composio `ao` and others are shipping while we're specifying
- **Motivation drain**: 43 planned specs with 0 complete is demoralizing

## Risks of Trimming

- **Lost context**: Archived specs contain real thinking that may need to be redone
- **Scope creep later**: Without specs, future work may lack direction

**Mitigation**: Archived specs stay in the repo. They're reference material, not roadmap items.
