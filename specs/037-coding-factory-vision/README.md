---
status: planned
created: 2026-03-11
priority: critical
tags:
- vision
- strategy
- factory
- assembly-line
- core-objective
parent: null
depends_on: []
created_at: 2026-03-11T00:00:00Z
updated_at: 2026-03-11T00:00:00Z
---

# The Coding Factory — Ford Assembly Line for the AI Era

## Core Objective (System Prompt)

> **Synodic builds the software production line for the AI era.**
>
> Just as Ford's assembly line transformed automobiles from artisanal one-offs
> into predictable, high-throughput, continuously-improving industrial output,
> Synodic transforms software from craft-mode (one developer, one task, one tool)
> into **factory-mode**: heterogeneous AI agents organized into assembly-line
> stations that transform requirements into shipped, tested, production-ready
> code at industrial scale and predictable quality.
>
> The measure of success is not how smart any single agent is — it's
> **throughput, cycle time, defect rate, and cost per unit of shipped software.**

---

## The Ford Parallel

Ford didn't invent the automobile. He invented the **production system** that
made automobiles accessible at scale. The key innovations weren't in the
engine — they were in the factory:

| Ford's Innovation | Synodic's Equivalent |
|---|---|
| **Division of labor** — complex car-building decomposed into simple, repeatable steps | **Agent specialization** — each station staffed by the best-fit agent (explorer, implementer, reviewer, tester, deployer) |
| **Moving assembly line** — work flows forward continuously; workers stay at their stations | **Task conveyor** — work items flow through pipeline stages automatically; agents don't context-switch |
| **Interchangeable parts** — any Model T part fits any Model T | **Tool-agnostic stations** — any conformant AI (Claude, Codex, Gemini, Copilot) can staff any role |
| **Standardized stations** — clear inputs, tooling, and outputs at each position | **Pipeline stages** — defined input/output contracts, quality gates, and SLAs per stage |
| **Quality inspection** — dedicated inspectors at key points; defective parts rejected before assembly | **Adversarial review gates** — generator/critic loops, automated validation, and human-in-the-loop at defined checkpoints |
| **Time studies** — measure every operation; eliminate waste | **Production metrics** — throughput (PRs/day), cycle time (spec→ship), defect rate (rework %), cost per unit ($/PR) |
| **Continuous improvement (Kaizen)** — every worker can stop the line; feedback loops tighten process | **Self-improving factory** — production data feeds back to optimize coordination patterns, agent routing, and cost |
| **Vertical integration** — Ford owned the steel mills, glass plants, rubber plantations | **End-to-end ownership** — from requirement intake through deployment, monitoring, and maintenance |
| **Economies of scale** — unit cost drops as volume increases | **Nemosis distillation** — frontier models teach cheaper models; cost per unit decreases with fleet experience |

---

## The Production Line — Seven Stations

```
 ╔═══════════╗    ╔═══════════╗    ╔═══════════╗    ╔═══════════╗
 ║  STATION 1║    ║  STATION 2║    ║  STATION 3║    ║  STATION 4║
 ║   INTAKE  ║───→║  DESIGN   ║───→║   BUILD   ║───→║  INSPECT  ║
 ║           ║    ║           ║    ║           ║    ║           ║
 ║ Requirement║   ║ Spec →    ║    ║ Code →    ║    ║ Adversarial║
 ║ → Spec    ║    ║ Blueprint ║    ║ Implement ║    ║ Review +  ║
 ║           ║    ║           ║    ║           ║    ║ Auto-test ║
 ╚═══════════╝    ╚═══════════╝    ╚═══════════╝    ╚═══════════╝
                                                          │
 ╔═══════════╗    ╔═══════════╗    ╔═══════════╗          │
 ║  STATION 7║    ║  STATION 6║    ║  STATION 5║          │
 ║  MAINTAIN ║←───║  DEPLOY   ║←───║  HARDEN   ║←─────────╘
 ║           ║    ║           ║    ║           ║
 ║ Stigmergic║    ║ Ship →    ║    ║ Speculative║
 ║ Monitor + ║    ║ Release + ║    ║ Swarm →   ║
 ║ Evolve    ║    ║ Rollback  ║    ║ Fuzz/Perf ║
 ╚═══════════╝    ╚═══════════╝    ╚═══════════╝
```

### Station 1 — INTAKE (Requirement → Spec)
- **Input:** Issue, feature request, bug report, user story, or verbal description
- **Process:** Decompose into atomic units; classify complexity; estimate cost
- **Coordination:** Fractal Decomposition (spec 027) — recursively split until each unit fits one agent
- **Output:** LeanSpec-formatted spec with acceptance criteria, tagged with cost estimate
- **Quality gate:** Spec review — is it atomic? testable? unambiguous?

### Station 2 — DESIGN (Spec → Blueprint)
- **Input:** Atomic spec
- **Process:** Explore approaches; identify affected files; design API/interface changes
- **Coordination:** Speculative Swarm (spec 025) — fork 2–4 agents to explore different approaches; converge on best
- **Output:** Implementation blueprint (files to change, approach, edge cases, test plan)
- **Quality gate:** Blueprint review — does it address all acceptance criteria? Are edge cases covered?

### Station 3 — BUILD (Blueprint → Code)
- **Input:** Approved blueprint
- **Process:** Implement the change, write tests, create PR
- **Coordination:** Hierarchical (spec 030) — lead implementer delegates to specialists (frontend, backend, infra)
- **Output:** PR with passing CI, tests, and documentation
- **Quality gate:** CI green, coverage threshold met

### Station 4 — INSPECT (Code → Reviewed Code)
- **Input:** PR
- **Process:** Multi-dimensional review — correctness, security, performance, style, maintainability
- **Coordination:** Generative-Adversarial (spec 028) — reviewer agents find flaws, author agent fixes; iterate until convergence
- **Output:** Approved PR or specific rework items sent back to Station 3
- **Quality gate:** All review dimensions pass; no open threads

### Station 5 — HARDEN (Reviewed Code → Battle-Tested Code)
- **Input:** Approved PR
- **Process:** Stress testing, fuzzing, performance benchmarking, integration testing
- **Coordination:** Speculative Swarm — multiple agents attack from different angles (security, load, edge cases)
- **Output:** Hardening report; performance baselines; security attestation
- **Quality gate:** No P0/P1 findings; perf within SLA

### Station 6 — DEPLOY (Tested Code → Production)
- **Input:** Hardened PR
- **Process:** Merge, release, canary deploy, monitor
- **Coordination:** Pipeline (spec 030) — sequential deployment stages with rollback triggers
- **Output:** Live in production with monitoring
- **Quality gate:** Error rate within threshold; no rollback triggered

### Station 7 — MAINTAIN (Production → Continuous Health)
- **Input:** Deployed system
- **Process:** Monitor health, detect drift, proactive maintenance, dependency updates
- **Coordination:** Stigmergic (spec 029) — agents observe production artifacts (logs, metrics, alerts) and self-organize maintenance tasks
- **Output:** Maintenance tasks fed back to Station 1 (loop)
- **Quality gate:** SLA compliance; tech debt budget

---

## Factory Metrics — The Dashboard

A factory is measured, not felt. The core metrics:

| Metric | Definition | Target |
|---|---|---|
| **Throughput** | Shipped units (PRs merged) per day | Continuous improvement |
| **Cycle time** | Wall-clock time from intake (Station 1) to production (Station 6) | < 4 hours for atomic units |
| **Lead time** | Time from requirement creation to production | < 24 hours for standard work |
| **Defect rate** | % of shipped units requiring rework post-deploy | < 5% |
| **First-pass yield** | % of units passing all stations without rework loops | > 80% |
| **Cost per unit** | Total agent compute cost per shipped PR | Decreasing quarter over quarter |
| **Agent utilization** | % of time agents are productive vs idle/waiting | > 70% |
| **WIP count** | Work items currently in flight across all stations | Bounded by Kanban limits |
| **Station dwell time** | Time spent at each station | Balanced (no station > 2x average) |

---

## Gap Analysis — Where We Are vs Where We Need to Be

### What Synodic HAS Built (the Blueprints)

Synodic has 36 specs defining a sophisticated architecture:

- **Coordination Intelligence** — 6 abstract operations, 11 primitives (5 AI-native), composable playbooks. This is the theoretical foundation for the factory's "machines." *(specs 012-035)*
- **Execution Substrate Design** — Process supervisor, message bus, state persistence. The factory floor blueprint. *(specs 004-006)*
- **Auth & Identity Design** — Agent identity, scoped secrets, RBAC. The factory security system. *(specs 007-010)*
- **Cost Optimization Strategy** — Nemosis teacher-student distillation. The efficiency program. *(spec 016)*
- **Competitive Intelligence** — Deep analysis of Composio `ao` (the nearest competitor). *(spec 036)*
- **Six Domain Playbooks** — Coding, finance, marketing, research, legal, devops. *(spec 014)*

**Assessment:** Synodic has designed the engines, transmissions, and assembly tooling. High-quality, thorough work.

### What's MISSING (the Gaps)

The gaps fall into six categories, ordered by criticality:

---

#### GAP 1: No Running Factory (Critical) → **spec 038**
**Current state:** 36 specs, 0 lines of shipped code.
**Ford equivalent:** Having detailed blueprints for every machine, conveyor belt, and station — but an empty factory floor.

**What's needed:**
- [ ] Minimum viable Station 3 (BUILD) — one agent, one task, produce one PR
- [ ] Minimum viable Station 4 (INSPECT) — one agent reviews the PR
- [ ] Minimum viable conveyor belt — task flows from build → inspect without human intervention
- [ ] Minimum viable metrics — measure cycle time and first-pass yield for even this two-station line

**Why this is gap #1:** A factory that produces one car proves the concept. A factory with no cars is a museum of unbuilt machines.

---

#### GAP 2: No Assembly Line Abstraction (High) → **spec 039**
**Current state:** Coordination primitives (swarm, mesh, adversarial, etc.) are defined but there's no "Station" or "Pipeline" as first-class concepts.
**Ford equivalent:** Having designed various power tools and machines but not the conveyor belt system that connects them into a production line.

**What's needed:**
- [ ] **Station** abstraction — defines input type, output type, quality gate, staffing (which agent types), SLA
- [ ] **Line** abstraction — ordered sequence of stations with routing rules (pass, rework, reject)
- [ ] **Conveyor** abstraction — automatic movement of work items between stations
- [ ] **WIP limits** — Kanban-style constraints preventing any station from being overwhelmed
- [ ] **Back-pressure** — when downstream stations are full, upstream stations slow down

---

#### GAP 3: No Quality System (High) → **spec 040**
**Current state:** Generative-adversarial pattern exists as a coordination primitive, but there's no integrated quality pipeline.
**Ford equivalent:** Having designed a precision measuring tool but not a quality department, not inspection stations on the line, not a process for rejecting defective parts.

**What's needed:**
- [ ] **Quality gates** as first-class pipeline elements (not just coordination patterns)
- [ ] **Rework routing** — rejected items go back to the right station, not to the end of the queue
- [ ] **Defect tracking** — what failed, at which station, root cause classification
- [ ] **Escape analysis** — when defects reach production, trace back to which gate missed them
- [ ] **Andon cord** — any agent can halt the line when it detects a systemic issue

---

#### GAP 4: No Production Metrics (High) → **spec 041**
**Current state:** No observability layer for the factory itself. Individual agent health is spec'd (spec 004), but factory-level metrics are absent.
**Ford equivalent:** Having a foreman who can check if a worker is alive, but no one measuring how many cars roll off the line per hour or what the defect rate is.

**What's needed:**
- [ ] **Throughput tracking** — units per time period, by station and end-to-end
- [ ] **Cycle time tracking** — per station and end-to-end, with percentile distributions
- [ ] **Cost accounting** — token usage, API calls, compute time, per station and per unit
- [ ] **Bottleneck detection** — automatically identify the constraint station (Theory of Constraints)
- [ ] **Efficiency dashboard** — real-time factory floor view (not just agent health)

---

#### GAP 5: No Continuous Improvement Loop (Medium) → **spec 042**
**Current state:** Nemosis handles cost optimization (cheaper models for repetitive work), but there's no mechanism for the factory to improve its own process.
**Ford equivalent:** Having a plan to buy cheaper steel, but no system for workers to suggest line improvements, no A/B testing of station configurations, no process engineering department.

**What's needed:**
- [ ] **A/B testing of coordination strategies** — try different primitives at the same station, measure which produces better output
- [ ] **Retrospective automation** — after each unit, agents analyze what slowed them down
- [ ] **Pattern library** — successful coordination configurations are captured and reused
- [ ] **Skill accumulation** — agents learn station-specific skills over time (beyond Nemosis cost optimization)
- [ ] **Process evolution** — the line layout itself changes based on production data

---

#### GAP 6: No Supply Chain (Medium) → **spec 043**
**Current state:** Agents are assumed to have everything they need. No concept of "just-in-time" delivery of context, dependencies, or pre-computed artifacts.
**Ford equivalent:** Having designed the assembly line but not the supply chain that delivers parts to each station exactly when needed.

**What's needed:**
- [ ] **Context delivery** — each station receives exactly the context it needs, no more (context windows are expensive)
- [ ] **Artifact cache** — pre-computed intermediate results (test fixtures, type stubs, API mocks) stored for reuse
- [ ] **Dependency resolution** — external dependencies (APIs, libraries, services) are resolved before work reaches BUILD
- [ ] **Prefetch** — predictively prepare context and artifacts for downstream stations

---

## Priority Roadmap — Building the Factory

### Phase 0: First Car Off the Line (MVP)
**Goal:** Prove that coordinated agents can ship one PR, end to end.
**Stations:** BUILD → INSPECT (two-station line)
**Agents:** Claude Code only
**Metrics:** Cycle time, pass/fail
**Deliverable:** A working `synodic run <spec>` that produces a reviewed PR

### Phase 1: Three-Station Line
**Goal:** Add DESIGN station upstream. Speculative swarm explores approaches before BUILD.
**Stations:** DESIGN → BUILD → INSPECT
**Agents:** Claude Code + one additional (Codex CLI or Copilot CLI)
**Metrics:** + throughput, first-pass yield
**Deliverable:** `synodic pipeline <spec>` runs the three-station flow

### Phase 2: Full Seven-Station Line
**Goal:** Complete the production line from INTAKE to MAINTAIN.
**Stations:** All seven
**Agents:** Heterogeneous fleet
**Metrics:** Full dashboard
**Deliverable:** `synodic factory start` — continuous production mode

### Phase 3: Self-Improving Factory
**Goal:** The factory optimizes itself.
**Capabilities:** A/B testing, retrospectives, pattern library, skill accumulation
**Metric target:** Measurable improvement in throughput and defect rate over each month
**Deliverable:** Autonomous process evolution with human oversight

### Phase 4: Factory Network
**Goal:** Multiple factories, each specialized for a domain (coding, research, devops), sharing coordination patterns and agent skills.
**Deliverable:** `synodic network` — federated factory management

---

## Reframing the Current Specs Through the Factory Lens

Every existing spec maps to a factory component:

| Factory Component | Current Specs | Gap |
|---|---|---|
| **Factory floor** (execution) | 004, 005, 006 | No code; needs MVP |
| **Machines** (coordination primitives) | 012, 013, 025-030 | Well-designed; need to be installable at stations |
| **Conveyor belt** (task flow) | 005 (message bus) | Exists as messaging; needs pipeline/station abstraction |
| **Quality department** (inspection) | 028 (generative-adversarial) | One pattern; needs full quality system |
| **Security** (auth) | 007-010 | Well-designed; pre-production concern |
| **Efficiency program** (cost) | 016 (Nemosis) | Addresses model cost; doesn't address process cost |
| **Standard work** (playbooks) | 014, 015 | Domain playbooks exist; need station-level work instructions |
| **Parts supply** (context/artifacts) | 001 (workspace persistence) | Git-backed memory; no JIT context delivery |
| **Blueprints** (specs) | LeanSpec, 015 | Spec framework exists; INTAKE station wraps it |
| **Time studies** (metrics) | 041 (production metrics) | Spec'd |
| **Process engineering** (self-improvement) | 042 (continuous improvement) | Spec'd |
| **Station abstraction** | 039 (assembly line abstraction) | Spec'd |
| **Assembly line abstraction** | 039 (assembly line abstraction) | Spec'd |
| **Rework routing** | 040 (factory quality system) | Spec'd |

---

## The One-Line Test

If we can't explain it in one line to a factory worker, the abstraction is wrong:

> **"Requirements go in one end, shipped code comes out the other, and every station in between is staffed by AI agents that never sleep, never forget, and get cheaper every month."**

---

## Relationship to Existing Vision

The current vision — "Docker Compose for AI agents" — describes the **execution substrate** (Layer 1). It's necessary but not sufficient.

The factory vision subsumes and extends it:

```
Docker Compose for AI agents    →  The factory floor
+ Coordination primitives       →  The machines
+ Pipeline/Station abstraction  →  The assembly line
+ Quality gates                 →  The inspection system
+ Production metrics            →  The time studies
+ Self-improvement              →  The process engineering dept
+ Cost optimization             →  The economies of scale
────────────────────────────────────────────────────
= The Coding Factory
```

**"Docker Compose for AI agents" is how we build the floor. "The Coding Factory" is what we build on it.**
