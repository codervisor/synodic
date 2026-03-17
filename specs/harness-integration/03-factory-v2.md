# Factory Skill v2 — Harness-Aware Upgrade

## Context

The current `skills/factory/SKILL.md` implements a BUILD → INSPECT pipeline with adversarial review and up to 3 rework cycles. It works, but has three structural gaps:

1. **No static pre-flight checks.** Every defect — including trivially catchable ones like type errors, lint violations, and dependency boundary breaches — consumes a full INSPECT round (expensive AI tokens + latency).
1. **No structured classification of rework reasons.** INSPECT’s rework items are free-text, making it impossible to aggregate patterns across runs.
1. **No cross-run learning.** Each factory run is independent. Lessons from run N never improve run N+1. The `.factory/` directory is gitignored and ephemeral.

## Goal

Evolve the Factory skill into a **Harness-aware** pipeline with three additions:

- **STATIC GATE** (new Step 2.5) — fast, deterministic checks between BUILD and INSPECT
- **Classified rework items** — each INSPECT rework item gets a category tag
- **GovernanceLog + Crystallization** — persistent cross-run learning that feeds back into STATIC GATE rules

## Detailed Changes

### 1. Add Step 2.5 — STATIC GATE

Insert between Step 2 (BUILD) and Step 3 (INSPECT). This step runs **without spawning a subagent** — it’s pure tooling:

```
Step 2.5 — STATIC GATE

After BUILD completes and reports a commit:

1. Run project-level checks against the build branch:
   - Type checking (cargo check / tsc --noEmit / pyright, depending on what changed)
   - Linting (clippy / eslint / ruff)
   - If a `.factory/rules/` directory exists, run each rule script against the diff

2. Collect all failures into a structured report.

3. If any failures:
   - Record them in the manifest under the current attempt as:
     {"gate": "static", "failures": [...]}
   - Route back to Step 2 (BUILD) with the failures as rework feedback.
   - This does NOT count toward the 3-attempt INSPECT limit.
     Track separately as `static_rework_count` in the manifest.
     Cap static rework at 2 to prevent infinite loops.
   
4. If all pass: proceed to Step 3 (INSPECT).
```

Rationale: structural errors caught in seconds for zero AI cost. INSPECT is reserved for semantic issues only.

### 2. Classify INSPECT Rework Items

Update the INSPECT_PROMPT’s output format. Change the REWORK items format from:

```
ITEMS:
- <specific actionable rework item>
```

To:

```
ITEMS:
- [completeness] <item>
- [correctness] <item>
- [security] <item>
- [conformance] <item>
- [quality] <item>
```

The category tag MUST be one of the five review dimensions already defined in the INSPECT prompt (completeness, correctness, security, conformance, quality). Add this instruction to the INSPECT_PROMPT:

> Each rework item MUST be prefixed with a category tag in square brackets matching one of the five review dimensions above. This enables automated pattern tracking across factory runs.

### 3. Enhance Manifest with GovernanceLog

#### 3a. Expand manifest.json schema

Add to the manifest schema:

```json
{
  "attempts": [
    {
      "number": 1,
      "build": { "commit": "abc123", "tests": "PASS", "files": [...] },
      "static_gate": { "passed": true, "failures": [] },
      "inspect": {
        "verdict": "REWORK",
        "items": [
          {"category": "completeness", "description": "Missing error handler for..."},
          {"category": "correctness", "description": "Off-by-one in pagination..."}
        ]
      }
    }
  ],
  "metrics": {
    "cycle_time_seconds": 120,
    "total_attempts": 2,
    "static_rework_count": 1,
    "first_pass_yield": false,
    "rework_categories": {"completeness": 1, "correctness": 1}
  }
}
```

#### 3b. Persist to GovernanceLog

After Step 7 (Finalize Manifest), add:

```
Step 7b — Persist to GovernanceLog

1. Append a summary record to `.factory/governance.jsonl` (this file is NOT gitignored — it accumulates across runs).
2. Each line is a JSON object:
   {
     "work_id": "factory-...",
     "spec": "specs/...",
     "timestamp": "...",
     "status": "approved|escalated",
     "total_attempts": N,
     "first_pass_yield": bool,
     "static_failures": ["type_error", "lint_violation", ...],
     "rework_items": [{"category": "...", "description": "..."}]
   }
```

#### 3c. Add Crystallization Hint (future hook)

Add a `## Future: Crystallization` section at the bottom of the SKILL.md as a documented extension point:

```markdown
## Future: Crystallization

When `.factory/governance.jsonl` accumulates enough data (target: 10+ runs), 
a crystallization process can:

1. Aggregate rework_items by category across all runs.
2. Identify high-frequency patterns (e.g., "completeness/missing-error-handler" appears in >30% of runs).
3. Generate new static gate rules in `.factory/rules/` that catch these patterns before INSPECT.
4. This creates a feedback loop: the more factory runs, the fewer issues reach INSPECT.

This is not yet implemented — it's a hook for a future `factory-crystallize` skill or CI job.
```

## Implementation Notes

- Keep the existing pipeline structure intact. This is additive, not a rewrite.
- The STATIC GATE step should detect which languages were modified in the BUILD output and only run relevant checkers.
- The `.factory/governance.jsonl` file should be added to version control (remove from `.gitignore` if necessary) so the learning persists across clones.
- The `.factory/rules/` directory is initially empty. It’s a hook for future crystallized rules.
- Update the `## Important Notes` section to document the new STATIC GATE behavior and governance log.

## Acceptance Criteria

- [ ] SKILL.md includes Step 2.5 STATIC GATE with clear instructions
- [ ] INSPECT_PROMPT requires category tags on rework items
- [ ] Manifest schema includes static_gate results and categorized rework items
- [ ] Metrics include `static_rework_count` and `rework_categories`
- [ ] `.factory/governance.jsonl` append logic documented in Step 7b
- [ ] Future Crystallization section present as extension point
- [ ] Existing Steps 1-7 still work unchanged for the happy path