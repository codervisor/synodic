# Fractal Skill v2 — Harness-Aware Upgrade

## Context

The current `skills/factory/SKILL.md` (Factory) was recently upgraded with Harness-aware governance: a STATIC GATE between BUILD and INSPECT, classified rework items, and a persistent GovernanceLog for cross-run learning.

The `skills/fractal/SKILL.md` (Fractal) is a sibling skill that uses a tree-shaped topology (DECOMPOSE → SOLVE → REUNIFY) instead of Factory’s linear pipeline. It currently has **zero governance** — no validation on decomposition quality, no review on leaf solutions, and no rework path when reunification fails. It assumes “decompose correctly once,” which is fragile in practice.

This prompt upgrades Fractal to be Harness-aware, using the same governance protocol as Factory (`evaluate() → Allow | Deny | Modify`) while respecting Fractal’s tree-shaped execution topology.

## Design Principle

Factory and Fractal are both **execution topologies** — pluggable strategies that run under the same Harness control plane. The Harness doesn’t care which topology you use. It cares that every critical decision point passes through a governance checkpoint. The governance protocol is orthogonal to the execution shape.

## Structural Gaps in Current Fractal

1. **DECOMPOSE has no validation.** A single subagent decides how to split. Nobody checks whether the split is orthogonal, complete, or appropriately sized. Common failure modes: overlapping scopes between children, missing critical dimensions, over-splitting that makes reunification harder than not splitting at all.
1. **SOLVE has no review.** Each leaf’s solution goes directly to REUNIFY with no independent quality gate. A low-quality leaf solution contaminates the entire tree.
1. **REUNIFY has no rework path.** If merging fails (CONFLICT status), the current design just reports PARTIAL and stops. There’s no feedback loop to re-solve conflicting leaves.
1. **No cross-run learning.** Same gap as Factory v1 — `.fractal/` is gitignored and ephemeral. Lessons from one decomposition never improve the next.

## Detailed Changes

### 1. Add DECOMPOSE Checkpoint (after Step 2)

After the DECOMPOSE subagent returns a SPLIT verdict, validate the decomposition before creating child directories. This does NOT require a subagent — it’s a structural check.

```
Step 2.5 — DECOMPOSE GATE

After parsing a SPLIT verdict, before writing child spec files:

1. Orthogonality check:
   - Compare each pair of children's scope descriptions.
   - Flag if any two children have >30% keyword overlap in their scope fields.
   - This is a heuristic: extract nouns/terms from scope, compute Jaccard similarity.

2. Coverage check:
   - Compare the union of all children's scopes against the parent spec.
   - Flag if the parent mentions key terms/requirements not covered by any child.

3. Budget sanity check:
   - If this split would bring total_nodes within 80% of max_total_nodes
     AND current depth < max_depth - 1, warn that budget is tight.

4. If any flags raised:
   - Do NOT auto-reject. Instead, append the flags to the DECOMPOSE subagent's
     output and re-prompt once with:
     
     "The following structural concerns were detected with your decomposition:
     {flags}
     Please revise your SPLIT verdict to address these concerns,
     or change to LEAF if splitting is not appropriate."
   
   - Parse the revised verdict. Accept it regardless (one retry only).
   - Record both the original and revised verdicts in the manifest.

5. If no flags: proceed normally.
```

Implementation note: the orthogonality check can be a simple Python script in `scripts/decompose_gate.py` that takes child scopes as input and returns flags. Keep it deterministic and fast — no LLM calls.

### 2. Add SOLVE Quality Gate (in Step 3)

Leverage the existing Factory skill for leaf solving. Update the `solve_mode` handling:

```
Step 3 — Solve (updated)

For each leaf node:

- If solve_mode is "parallel" or "sequential" (current behavior):
  Run the SOLVE subagent as-is, BUT after it returns:
  
  Step 3.5 — SOLVE GATE
  
  1. Run static checks against the leaf's output (same as Factory's STATIC GATE):
     - Type checking on changed files
     - Linting on changed files
     - If `.factory/rules/` exists, apply crystallized rules
  
  2. If static failures: re-solve once with failures as feedback.
     Track as `static_rework_count` per leaf in the manifest.
     Cap at 1 retry per leaf (trees have many leaves — keep cost bounded).
  
  3. If pass: proceed to reunify.

- If solve_mode is "factory" (composable mode):
  Delegate to `/factory run` for each leaf spec.
  Factory's own INSPECT provides the quality gate — no additional gate needed here.
  Record the Factory manifest reference in the Fractal manifest.
```

This creates two tiers of governance for SOLVE:

- **Lightweight** (default): static checks only, fast, no AI cost
- **Full** (`solve_mode: factory`): Factory’s complete BUILD → STATIC GATE → INSPECT pipeline per leaf

### 3. Add REUNIFY Rework Path (in Step 4)

Currently, REUNIFY reports CONFLICT but has no recovery. Add a bounded rework loop:

```
Step 4 — Reunify (updated)

After each REUNIFY subagent returns:

- If STATUS is MERGED: write result.md, continue bottom-up. No change.

- If STATUS is CONFLICT or PARTIAL:
  
  Step 4.5 — REUNIFY REWORK
  
  1. Parse the CONFLICTS list from the REUNIFY REPORT.
  2. Identify which child solutions are in conflict.
  3. Re-spawn SOLVE subagents for ONLY the conflicting children, with:
     - The original spec
     - The conflict description as additional context
     - The non-conflicting sibling results as fixed constraints
  4. After re-solve, retry REUNIFY once.
  5. If still CONFLICT after one retry: mark this node as PARTIAL in manifest
     and continue bottom-up. The parent reunification will see a PARTIAL child
     and can attempt resolution at its level.
  
  Track `reunify_rework_count` per node in the manifest.
  Maximum 1 rework per reunify node — trees compound, so keep it bounded.
```

### 4. Classified Decompose and Reunify Issues

Same principle as Factory’s classified rework items. Add category tags to make cross-run aggregation possible.

**For DECOMPOSE GATE flags**, use these categories:

- `[orthogonality]` — scope overlap between children
- `[coverage]` — parent requirements not covered by any child
- `[granularity]` — splitting too fine or too coarse
- `[budget]` — node budget pressure

**For REUNIFY CONFLICTS**, add this instruction to the REUNIFY_PROMPT:

> Each conflict MUST be prefixed with a category tag:
> 
> - [interface] — mismatched types, signatures, or contracts between children
> - [boundary] — a child implemented something outside its declared scope
> - [redundancy] — multiple children solved the same thing differently
> - [gap] — something needed for integration was not produced by any child

### 5. GovernanceLog Integration

Same pattern as Factory v2. After Step 5 (Finalize), add:

```
Step 5b — Persist to GovernanceLog

Append a summary record to `.fractal/governance.jsonl` (NOT gitignored):

{
  "work_id": "fractal-...",
  "source": "fractal",
  "timestamp": "...",
  "status": "complete|partial|failed",
  "config": { "max_depth": 3, "split_strategy": "orthogonal", ... },
  "tree_metrics": {
    "depth": N,
    "total_nodes": N,
    "leaf_nodes": N,
    "pruned_nodes": N
  },
  "decompose_flags": [
    {"node": "1-auth", "category": "orthogonality", "description": "..."}
  ],
  "solve_failures": [
    {"leaf": "1-auth/2-sessions", "static_failures": ["type_error"], "rework_count": 1}
  ],
  "reunify_conflicts": [
    {"node": "1-auth", "category": "interface", "description": "..."}
  ],
  "cycle_time_seconds": N
}
```

### 6. Future: Cross-Topology Crystallization

Add a section at the bottom of the SKILL.md:

```markdown
## Future: Cross-Topology Crystallization

Both Factory and Fractal write to governance logs (`.factory/governance.jsonl`
and `.fractal/governance.jsonl`). A future crystallization process can aggregate
across BOTH logs to identify patterns that transcend execution topology:

- Recurring decompose flags → improve DECOMPOSE_PROMPT heuristics or add
  static decomposition templates for known problem shapes
- Recurring solve static failures → feed into `.factory/rules/` (shared with Factory)
- Recurring reunify conflicts → generate interface contract templates
  in `.fractal/templates/` that future decompositions can reference

The static rules in `.factory/rules/` are shared between Factory and Fractal
because SOLVE (Fractal) and BUILD (Factory) face the same class of structural errors.
Crystallizing once benefits both topologies.
```

## Implementation Notes

- All changes are additive. The existing Steps 1–5 remain intact for the happy path.
- The DECOMPOSE GATE (Step 2.5) should be implemented as a script (`scripts/decompose_gate.py`) for deterministic, fast execution. It takes child scopes as JSON input and returns flags as JSON output.
- SOLVE GATE (Step 3.5) reuses the same static checking logic as Factory’s STATIC GATE. If Factory has helper scripts for this, import them rather than duplicating.
- REUNIFY REWORK (Step 4.5) is bounded to 1 retry per node. This is intentional — trees compound exponentially, and unbounded rework at every node would be catastrophic for cost and latency.
- The `.fractal/governance.jsonl` file should be tracked in version control. Update `.gitignore` accordingly.
- Update the `## Comparison with Factory Skill` table to include governance characteristics.
- Update `## Important Notes` to document the new checkpoints.

## Updated Comparison Table

Add a governance row to the existing comparison table:

```markdown
| Aspect       | Factory                              | Fractal                                    |
|------------- |--------------------------------------|--------------------------------------------|
| Shape        | Linear pipeline (BUILD → INSPECT)    | Recursive tree (DECOMPOSE → SOLVE → REUNIFY) |
| Parallelism  | Sequential stations                  | Parallel leaf solving                       |
| Rework       | INSPECT → BUILD loop (max 3)         | Bounded: 1 retry at DECOMPOSE, SOLVE, REUNIFY |
| Governance   | STATIC GATE + adversarial INSPECT    | DECOMPOSE GATE + SOLVE GATE + REUNIFY REWORK |
| Learning     | .factory/governance.jsonl            | .fractal/governance.jsonl (shared rules)    |
| Output       | PR from single implementation        | Unified result from merged sub-solutions    |
| When to use  | Single spec, needs review            | Complex task, needs decomposition           |
```

## Acceptance Criteria

- [ ] DECOMPOSE GATE (Step 2.5) with orthogonality/coverage/budget checks
- [ ] `scripts/decompose_gate.py` for deterministic flag detection
- [ ] SOLVE GATE (Step 3.5) with static checks, reusing Factory’s checking logic
- [ ] REUNIFY REWORK (Step 4.5) with bounded 1-retry loop for CONFLICT status
- [ ] Category tags on decompose flags and reunify conflicts
- [ ] `.fractal/governance.jsonl` append logic in Step 5b
- [ ] Future Cross-Topology Crystallization section documented
- [ ] Updated comparison table with governance row
- [ ] Existing Steps 1–5 still work unchanged for the happy path (no flags, no conflicts)