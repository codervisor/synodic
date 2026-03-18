---
status: planned
created: 2026-03-18
priority: high
tags:
- adversarial
- skill
- hardening
- generator-critic
- quality
parent: 037-coding-factory-vision
depends_on:
- 044-factory-skill-mvp
---

# Adversarial Skill — Generator-Critic Hardening Loop

> **Status**: planned · **Priority**: high · **Created**: 2026-03-18

## Overview

Factory gives us BUILD → INSPECT: a single review pass that catches issues.
Fractal gives us DECOMPOSE → SOLVE → REUNIFY: parallel decomposition of complex tasks.

Neither provides **iterative hardening** — the generator and critic locked in an
escalating quality loop where the critic actively tries to break the output and
the generator anticipates and preempts attacks. This is Station 5 (HARDEN) from
the seven-station vision (spec 037).

The adversarial skill implements the generative-adversarial coordination primitive
(spec 028) as a concrete Claude Code skill. Two subagents — Generator and Critic —
iterate through an escalation ladder until the Critic can't find new issues for K
consecutive rounds, or the round cap is hit.

**Key difference from Factory INSPECT:** Factory's INSPECT is a single-pass
reviewer that returns APPROVE/REWORK. The adversarial Critic is a multi-round
attacker that escalates sophistication each round and must produce concrete
failing test cases, not just review comments.

**Agent property exploited:** No fatigue + lossless critique. Agents maintain
consistent attack quality across arbitrarily many rounds without ego,
defensiveness, or cognitive degradation.

### Use cases

1. **Post-Factory hardening**: Factory produces an approved PR → adversarial skill
   hardens it against edge cases, concurrency bugs, security vulnerabilities
2. **Standalone hardening**: Point at existing code and harden a specific module
3. **Test suite generation**: Critic produces adversarial test cases as artifacts,
   not just feedback — the test suite is a first-class output
4. **Security audit**: Fixed critic mode (`adversarial-inputs`) for targeted
   security hardening of a specific component

## Design

### Skill Architecture

```
/adversarial harden <target>

Main conversation (orchestrator)
  │
  ├─→ GENERATE subagent (general-purpose, isolation: worktree)
  │     • Reads target (spec, file, or diff)
  │     • Produces/improves artifact
  │     • Addresses all issues from prior ATTACK round
  │     • Commits to adversarial/{work-id} branch
  │     • Returns: GENERATE REPORT
  │
  ├─→ ATTACK subagent (general-purpose, fresh context each round)
  │     • Reads artifact only (no generator reasoning = adversarial)
  │     • Escalates attack sophistication per round
  │     • Must produce concrete failing tests, not vague complaints
  │     • Returns: ATTACK REPORT with issues + test cases
  │
  └─→ Orchestration loop
        • If issues found: re-invoke GENERATE with issues + test cases
        • If clean round: increment consecutive_clean counter
        • If consecutive_clean >= K: terminate (hardened)
        • If rounds >= max_rounds: terminate (cap hit)
```

### Escalation Ladder

Each round, the Critic operates at a specific attack level. In `progressive`
mode (default), difficulty increases each round:

| Level | Mode | Description |
|-------|------|-------------|
| 1 | `correctness` | Logic errors, off-by-one, wrong return values, type mismatches |
| 2 | `edge-cases` | Boundary conditions, empty inputs, overflow, nil/null, unicode |
| 3 | `error-handling` | Missing error paths, uncaught exceptions, resource leaks, partial failures |
| 4 | `concurrency` | Race conditions, deadlocks, TOCTOU, resource contention, shared state |
| 5 | `security` | Injection (SQL/XSS/command), path traversal, SSRF, auth bypass, secrets exposure |
| 6 | `adversarial-inputs` | Malformed data, protocol abuse, resource exhaustion, timing attacks |

After completing the ladder, subsequent rounds cycle back to `correctness` but
with full accumulated context — catching regressions from prior fixes.

### Escalation Modes

| Mode | Behavior | When to use |
|------|----------|-------------|
| `progressive` | Follows ladder top-to-bottom, then cycles | Default — systematic coverage |
| `fixed` | Same attack level every round | Targeted hardening (e.g., security-only) |
| `adaptive` | Critic chooses attack level based on prior findings | When attack surface is unknown |

### Termination Conditions

The loop ends when **any** condition is met:

1. **Consecutive clean rounds** (`consecutive_clean_rounds`, default: 2) — Critic
   found no new issues for K rounds in a row
2. **Max rounds** (`max_rounds`, default: 8) — safety cap to prevent infinite loops
3. **Quality plateau** — no improvement for 3 consecutive rounds (issues found
   but Generator can't fix them → escalate to human)
4. **Budget exhaustion** — token budget exceeded

### GENERATE Subagent

- **Type:** general-purpose with `isolation: worktree`
- **Input:** Target content + attack report from prior ATTACK round (if any)
- **Process:**
  1. Read target (spec, code file, or diff reference)
  2. If first round: produce initial implementation/improvement
  3. If rework round: address ALL issues from prior ATTACK report
  4. Run the Critic's test cases from prior round — they must pass
  5. Anticipate next attack level and proactively harden
  6. Commit to `adversarial/{work-id}` branch
- **Output:** GENERATE REPORT block:
  ```
  === GENERATE REPORT ===
  ROUND: {N}
  FILES: {comma-separated list}
  TESTS_ADDED: {count of new test cases added}
  PRIOR_ISSUES_ADDRESSED: {count} / {total from prior ATTACK}
  COMMIT: {short SHA}
  BRANCH: {branch name}
  === END GENERATE REPORT ===
  ```

### ATTACK Subagent

- **Type:** general-purpose (NO worktree — read-only, fresh context each round)
- **Input:** Artifact (diff or file contents) + attack level + original target spec
- **Adversarial isolation:** NO generator reasoning, NO prior attack reports
  (fresh eyes each round to prevent anchoring to prior findings)
- **Process:**
  1. Read the artifact at current state
  2. Attack at the specified level (or choose level in `adaptive` mode)
  3. For each issue found, produce a **concrete failing test case**
  4. Classify each issue: `[correctness]`, `[edge-case]`, `[error-handling]`,
     `[concurrency]`, `[security]`, `[adversarial-input]`
  5. Report honestly when no issues are found
- **Output:** ATTACK REPORT block:
  ```
  === ATTACK REPORT ===
  ROUND: {N}
  LEVEL: {attack level}
  VERDICT: ISSUES_FOUND | CLEAN
  ISSUES:
  - [category] {description}
    TEST: {concrete test case or reproduction steps}
  - [category] {description}
    TEST: {concrete test case or reproduction steps}
  === END ATTACK REPORT ===
  ```

### Generator-Critic Contract

**Generator MUST:**
- Produce a complete, testable artifact each round
- Address ALL issues from the prior ATTACK report (not cherry-pick)
- Run the Critic's test cases — they must all pass before submitting
- Not suppress or ignore Critic feedback

**Critic MUST:**
- Provide specific, reproducible issues with concrete test cases
- Escalate difficulty as configured (not repeat prior levels unless cycling)
- Report CLEAN honestly when no issues are found at current level
- Not issue vague complaints or subjective style feedback

### Orchestration Protocol

When invoked, execute the following steps **exactly**:

#### Step 1 — Initialize

1. Parse `<target>` — can be a spec path, file path, or `--diff <ref>` for a git diff
2. Generate work ID: `adversarial-{unix-timestamp}`
3. Create `.adversarial/{work-id}/` directory
4. Load configuration from `.adversarial.yaml` (or defaults)
5. Initialize manifest at `.adversarial/{work-id}/manifest.json`:
   ```json
   {
     "id": "{work-id}",
     "target": "{target}",
     "status": "hardening",
     "branch": "adversarial/{work-id}",
     "config": {
       "max_rounds": 8,
       "escalation": "progressive",
       "consecutive_clean_rounds": 2,
       "attack_levels": ["correctness", "edge-cases", "error-handling",
                         "concurrency", "security", "adversarial-inputs"]
     },
     "rounds": [],
     "metrics": {}
   }
   ```
6. Record start time.

#### Step 2 — GENERATE (round N)

Spawn a **general-purpose subagent** with `isolation: worktree`:

**GENERATE prompt includes:**
- Target content (spec or code)
- If round > 1: full ATTACK REPORT from prior round
- If round > 1: "You MUST address all issues listed above. Run the test cases
  provided — they must pass."
- Current round number and remaining rounds
- Next attack level (so Generator can proactively harden)

**Parse GENERATE REPORT** from subagent response. Record in manifest.

#### Step 2.5 — STATIC GATE (Layer 1)

Same as Factory — run language-specific linters/checkers on changed files.
If failures: rework back to Step 2 (max 2 static retries, not counted toward
round limit). Track as `static_rework_count`.

#### Step 3 — ATTACK (round N)

Spawn a **general-purpose subagent** (NO worktree, fresh context):

**ATTACK prompt includes:**
- Git diff reference: `git diff main...{build-branch}`
- Original target (spec or code description)
- Attack level for this round (from escalation ladder)
- "You are an adversarial critic. Your job is to BREAK this code."
- "For each issue, provide a CONCRETE FAILING TEST CASE."
- "If you find no issues at this level, report CLEAN."

**CRITICAL:** No Generator reasoning, no prior ATTACK reports. Fresh context
each round for adversarial independence.

**Parse ATTACK REPORT** from subagent response. Record in manifest.

#### Step 4 — Route

1. If `VERDICT: CLEAN`:
   - Increment `consecutive_clean` counter
   - If `consecutive_clean >= consecutive_clean_rounds`: → Step 5 (Finalize)
   - Else: advance to next attack level → Step 2 (next round)
2. If `VERDICT: ISSUES_FOUND`:
   - Reset `consecutive_clean` counter to 0
   - Check for quality plateau: if same issue count as last 2 rounds → Step 6 (Escalate)
   - If `round < max_rounds`: → Step 2 with ATTACK feedback (next round)
   - If `round >= max_rounds`: → Step 6 (Escalate, cap hit)

#### Step 5 — Finalize (Hardened)

1. Update manifest status: `"hardened"`
2. Collect all test cases produced by Critic across all rounds → write to
   `.adversarial/{work-id}/adversarial_tests.md`
3. Push branch: `git push -u origin adversarial/{work-id}`
4. Create PR:
   ```
   gh pr create \
     --title "adversarial: {target title}" \
     --body "## Adversarial Hardening Report\n\n{round history}\n\nTests added: {count}"
   ```
5. Record final metrics in manifest
6. Report to user: PR URL + hardening summary

#### Step 6 — Escalate

1. Update manifest status: `"escalated"` (if cap hit) or `"plateau"` (if stuck)
2. Report to user:
   - Rounds completed and attack levels covered
   - Outstanding issues from last ATTACK report
   - Branch available for manual review
3. Do NOT create PR
4. Suggest: run again with `--fixed {level}` targeting specific weakness

#### Step 7 — Persist to Governance Log

Append record to `.harness/adversarial.governance.jsonl`:

```json
{
  "work_id": "adversarial-...",
  "source": "adversarial",
  "target": "{target}",
  "timestamp": "<ISO 8601>",
  "status": "hardened|escalated|plateau",
  "total_rounds": 5,
  "static_rework_count": 0,
  "attack_levels_covered": ["correctness", "edge-cases", "error-handling", "concurrency", "security"],
  "issues_found_total": 7,
  "issues_by_category": {"correctness": 2, "edge-case": 3, "security": 2},
  "tests_generated": 12,
  "consecutive_clean_at_termination": 2,
  "cycle_time_seconds": 480
}
```

### Configuration

Optional `.adversarial.yaml` in repo root:

```yaml
adversarial:
  max_rounds: 8
  escalation: progressive        # progressive | fixed | adaptive
  consecutive_clean_rounds: 2    # Clean rounds needed to terminate
  attack_levels:                 # Escalation ladder (order matters)
    - correctness
    - edge-cases
    - error-handling
    - concurrency
    - security
    - adversarial-inputs
  fixed_level: null              # Override: lock to single level
  output_tests: true             # Collect Critic test cases as artifact
```

### Composability

| Composition | Valid | Rationale |
|-------------|-------|-----------|
| Factory → Adversarial | Yes | Factory produces approved code → Adversarial hardens it further |
| Fractal → Adversarial | Yes | Each fractal leaf gets adversarial hardening before reunification |
| Adversarial → Factory | Yes | Hardened code feeds back into Factory for final PR review |
| **Adversarial → Adversarial** | No | Meta-critique without grounded production — infinite regress |

The primary composition is **Factory → Adversarial**: Factory's INSPECT ensures
basic correctness, then Adversarial's escalating attacks harden against
progressively sophisticated failures. This maps to Station 3-4 (BUILD+INSPECT)
feeding Station 5 (HARDEN) in the vision.

### Comparison with Factory and Fractal

| Aspect | Factory | Fractal | Adversarial |
|--------|---------|---------|-------------|
| Shape | Linear (BUILD → INSPECT) | Tree (DECOMPOSE → SOLVE → REUNIFY) | Loop (GENERATE ↔ ATTACK) |
| Iterations | max 3 rework cycles | 1 per node | max 8 rounds (configurable) |
| Review style | Single-pass approve/reject | Per-leaf static gate | Multi-round escalating attacks |
| Critic output | Review comments | Static check failures | Concrete failing test cases |
| Primary goal | Ship a spec as PR | Decompose & solve complex tasks | Harden code against edge cases |
| Quality model | "Good enough to ship" | "Orthogonal and complete" | "Can't break it" |
| Test generation | No | No | Yes — adversarial test suite is first-class output |

### Governance Checkpoint Map

| Step | Layer | Description |
|------|-------|-------------|
| Step 2.5 STATIC GATE | Layer 1 | Deterministic linters/checkers, no AI cost |
| Step 3 ATTACK | Layer 2 | Adversarial AI review with escalating sophistication |
| Step 6 Escalate | Layer 3 | Human escalation on round cap or quality plateau |

### Skill Structure

```
skills/adversarial/
├── SKILL.md                      # Skill definition (orchestration protocol)
├── references/
│   └── manifest.schema.json      # Work manifest JSON schema
├── fixtures/
│   └── sample-target/            # Trivial target for validation
│       ├── README.md             # Sample spec
│       └── calculator.py         # Deliberately vulnerable implementation
├── evals/
│   ├── evals.json                # Behavioral evals
│   └── prompts/
│       ├── e2e-hardening.md      # Full loop on sample target
│       ├── escalation-ladder.md  # Attack levels progress correctly
│       ├── termination-clean.md  # Terminates on consecutive clean rounds
│       ├── termination-cap.md    # Terminates on max rounds
│       ├── quality-plateau.md    # Detects plateau and escalates
│       ├── test-generation.md    # Adversarial test cases collected as artifact
│       └── adversarial-isolation.md  # Critic has fresh context each round
```

## Plan

- [ ] Create `skills/adversarial/SKILL.md` with full orchestration protocol
- [ ] Define `manifest.schema.json` for adversarial work manifests
- [ ] Implement GENERATE subagent prompt (target reading, fix-all-issues, commit)
- [ ] Implement ATTACK subagent prompt (escalation ladder, concrete test cases, fresh context)
- [ ] Implement orchestration loop (escalation, consecutive-clean termination, plateau detection)
- [ ] Implement manifest writing (`.adversarial/{work-id}/manifest.json` after each round)
- [ ] Implement test collection (aggregate Critic test cases across rounds)
- [ ] Implement PR creation on hardened status
- [ ] Create sample fixture with deliberately vulnerable target
- [ ] Create behavioral evals (7 scenarios)
- [ ] Register skill in CLAUDE.md and update README
- [ ] Add `.adversarial/` to `.gitignore`

## Test

- [ ] `/adversarial harden` on a sample target completes multi-round loop and produces PR
- [ ] Attack levels escalate in `progressive` mode (round 1=correctness, round 2=edge-cases, etc.)
- [ ] Loop terminates after 2 consecutive CLEAN rounds (default K=2)
- [ ] Loop terminates at max_rounds cap with escalation to human
- [ ] Quality plateau detected (3 rounds, no improvement) → escalation
- [ ] Critic produces concrete failing test cases, not vague complaints
- [ ] Test cases from all rounds collected into adversarial_tests.md artifact
- [ ] Critic has fresh context each round (no generator reasoning, no prior attack reports)
- [ ] Static gate runs between GENERATE and ATTACK (same as Factory)
- [ ] `--fixed security` locks critic to security level for all rounds
- [ ] Governance log entry appended to `.harness/adversarial.governance.jsonl`
- [ ] Factory → Adversarial composition: approved Factory PR fed into adversarial hardening

## Notes

This skill implements the generative-adversarial coordination primitive from spec
028 (archived). The theoretical framework is preserved — escalation ladder,
generator-critic contract, termination conditions — but adapted to the skill-first
architecture established by Factory (044) and Fractal.

The key insight from spec 028 remains: **this is not code review.** Factory's
INSPECT is review (approve/reject). Adversarial's ATTACK is active exploitation
(break it or declare it hardened). The Critic must produce failing test cases,
not opinions. This grounds the adversarial loop in executable evidence.
