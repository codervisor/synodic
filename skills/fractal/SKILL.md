---
name: fractal
description: "Fractal decomposition — recursively split a complex task into orthogonal sub-specs, solve each leaf independently via subagents, then reunify results bottom-up. Use when a task is too large for a single agent pass, when you need to decompose a problem into independently-solvable pieces, or when the user invokes /fractal decompose <task-or-spec-path>."
---

# Fractal Decomposition Skill

Recursively decompose a complex task into a tree of sub-specs on disk, solve each leaf via independent subagents, then reunify results bottom-up into a single coherent output.

## Why specs as intermediate medium

Claude Code subagents cannot spawn their own subagents (no nesting). True fractal decomposition requires recursive depth. The solution: **materialize the decomposition tree as spec files on disk**. Each level of the tree is a directory of sub-specs. The orchestrator (this skill) walks the tree level by level, spawning subagents for each node. The filesystem IS the recursion stack.

```
.fractal/{work-id}/
├── manifest.json              # Tree state + metrics
├── root.md                    # Original task spec
├── tree/
│   ├── 1-auth/
│   │   ├── spec.md            # Sub-spec: "design auth system"
│   │   ├── result.md          # Filled in after solve
│   │   ├── 1-oauth/
│   │   │   ├── spec.md        # Leaf spec: "OAuth flow"
│   │   │   └── result.md
│   │   └── 2-sessions/
│   │       ├── spec.md        # Leaf spec: "session management"
│   │       └── result.md
│   ├── 2-data/
│   │   ├── spec.md
│   │   ├── result.md
│   │   └── ...
│   └── 3-api/
│       ├── spec.md            # Leaf (no further split needed)
│       └── result.md
└── output.md                  # Final reunified result
```

## Usage

```
/fractal decompose <task-description-or-spec-path>
```

Examples:
```
/fractal decompose specs/045-some-feature/README.md
/fractal decompose "Design a microservices platform with auth, data layer, and API gateway"
```

## Configuration

Defaults can be overridden by placing a `.fractal.yaml` in the repo root:

```yaml
fractal:
  max_depth: 3                    # Maximum tree depth (default: 3)
  max_children: 5                 # Max sub-specs per node (default: 5)
  max_total_nodes: 20             # Hard cap on total tree nodes (default: 20)
  split_strategy: orthogonal      # orthogonal | aspect-based | temporal
  reunification: lossless-merge   # lossless-merge | best-child | summary-merge
  min_complexity: medium          # Minimum complexity before splitting is allowed
  solve_mode: parallel            # parallel | sequential
  output_mode: code               # code | design | analysis
```

## Orchestration Protocol

When invoked, execute the following steps **exactly**:

### Step 1 — Initialize

1. Parse the input: if a file path, read the spec. If a string, treat it as the task description.
2. Generate a work ID: `fractal-{unix-timestamp}` (e.g., `fractal-1710600000`).
3. Create `.fractal/{work-id}/` directory.
4. Write `root.md` with the original task.
5. Initialize `manifest.json`:
   ```json
   {
     "id": "{work-id}",
     "status": "decomposing",
     "config": {
       "max_depth": 3,
       "max_children": 5,
       "max_total_nodes": 20,
       "split_strategy": "orthogonal",
       "reunification": "lossless-merge"
     },
     "tree": {},
     "metrics": {}
   }
   ```
6. Read `.fractal.yaml` from the repo root if it exists and override defaults.

### Step 2 — Decompose (top-down, level by level)

This is the core phase. It runs iteratively (not recursively) to work around the no-nested-subagents constraint.

**For each level, starting from the root:**

Spawn a **general-purpose subagent** for each node that needs decomposition:

```
Agent(
  subagent_type: "general-purpose",
  prompt: <DECOMPOSE_PROMPT below>
)
```

**DECOMPOSE_PROMPT:**

> You are the DECOMPOSE station of a fractal decomposition pipeline.
>
> ## Task
> {content of the node's spec.md}
>
> ## Parent context
> {content of parent spec.md, or "This is the root task." if root}
>
> ## Constraints
> - Split strategy: {split_strategy}
> - Maximum children: {max_children}
> - Current depth: {current_depth} / {max_depth}
> - Remaining node budget: {remaining_nodes}
>
> ## Instructions
>
> Analyze this task and determine whether it should be split into sub-problems.
>
> **Do NOT split if:**
> - The task is simple enough for one agent to complete in a single pass
> - The task has no natural orthogonal decomposition
> - Current depth equals max depth
> - Splitting would produce trivial sub-problems
>
> **If splitting, for each sub-problem produce:**
> 1. A short slug name (e.g., `auth`, `data-layer`, `api-gateway`)
> 2. A scope description: what this sub-problem covers
> 3. Explicit boundaries: what it does NOT cover
> 4. Input dependencies: what it needs from sibling sub-problems (if any)
> 5. Output contract: what it produces
>
> ## Output format
>
> If this task should NOT be split:
> ```
> === DECOMPOSE VERDICT ===
> VERDICT: LEAF
> REASON: {why splitting is unnecessary}
> === END DECOMPOSE VERDICT ===
> ```
>
> If this task SHOULD be split:
> ```
> === DECOMPOSE VERDICT ===
> VERDICT: SPLIT
> CHILDREN:
> - slug: {slug}
>   scope: {what this child handles}
>   boundaries: {what it does NOT handle}
>   inputs: {dependencies from siblings, or "none"}
>   outputs: {what it produces}
> - slug: {slug}
>   ...
> === END DECOMPOSE VERDICT ===
> ```

After each decompose subagent returns:
- Parse the DECOMPOSE VERDICT.
- If LEAF: mark this node as a leaf in the manifest. No further action.
- If SPLIT: create child directories under `tree/{node-path}/` and write each child's `spec.md` from the scope/boundaries/outputs.
- Update `manifest.json` with the tree structure.
- Decrement the remaining node budget.

**Repeat** for the next level of the tree until all nodes are either leaves or at max depth.

**Budget enforcement:** If `max_total_nodes` would be exceeded, mark the current node as a leaf regardless of the decompose verdict.

### Step 3 — Solve (leaves, bottom-up)

Process all leaf nodes. If `solve_mode` is `parallel`, spawn all leaf subagents concurrently. If `sequential`, process them in dependency order.

For each leaf node, spawn a **general-purpose subagent**:

```
Agent(
  subagent_type: "general-purpose",
  isolation: "worktree",   # Only if output_mode is "code"
  prompt: <SOLVE_PROMPT below>
)
```

**SOLVE_PROMPT:**

> You are the SOLVE station of a fractal decomposition pipeline.
> You are solving ONE specific sub-problem in isolation.
>
> ## Your sub-problem
> {content of this leaf's spec.md}
>
> ## Scope boundaries
> You are ONLY responsible for: {scope}
> You are NOT responsible for: {boundaries}
>
> ## Sibling context (read-only)
> These are the other sub-problems being solved in parallel. You may read
> their specs for interface alignment, but do NOT implement their functionality.
> {list of sibling spec.md contents}
>
> ## Output contract
> Your solution must produce: {outputs from decompose step}
>
> ## Instructions
>
> 1. Read the sub-problem spec carefully.
> 2. Implement a self-contained solution within your declared scope.
> 3. If output_mode is "code": write files, run tests, commit with prefix `fractal({slug}):`.
> 4. If output_mode is "design" or "analysis": produce a structured document.
> 5. Respect your boundaries — do not reach into sibling scopes.
>
> ## Output format
>
> ```
> === SOLVE REPORT ===
> SLUG: {slug}
> STATUS: COMPLETE | PARTIAL | FAILED
> FILES: {comma-separated list of files changed, or "N/A" for non-code}
> SUMMARY: {one-paragraph summary of what was produced}
> INTERFACES: {any interfaces/contracts this solution exposes for siblings}
> === END SOLVE REPORT ===
> ```

After each solve subagent returns:
- Parse the SOLVE REPORT.
- Write `result.md` in the leaf's directory with the full subagent output.
- Update the manifest with solve status.

### Step 4 — Reunify (bottom-up)

Once all leaves at a level are solved, reunify them into their parent. Walk the tree bottom-up.

For each non-leaf node whose children are all solved, spawn a **general-purpose subagent**:

```
Agent(
  subagent_type: "general-purpose",
  isolation: "worktree",   # Only if output_mode is "code"
  prompt: <REUNIFY_PROMPT below>
)
```

**REUNIFY_PROMPT:**

> You are the REUNIFY station of a fractal decomposition pipeline.
> Your job is to merge child solutions into a coherent whole for this level.
>
> ## This node's scope
> {content of this node's spec.md}
>
> ## Reunification strategy: {reunification}
>
> ## Child solutions
> {for each child: slug, spec.md content, result.md content}
>
> ## Instructions
>
> **lossless-merge** (default): Integrate ALL child outputs. Resolve interface
> mismatches. The merged result must be strictly more complete than any child alone.
>
> **best-child**: Select the single child whose solution best addresses the parent
> scope. Justify the selection.
>
> **summary-merge**: Produce a synthesis from child summaries without including
> full implementations (use when context would be too large).
>
> Steps:
> 1. Review each child's solution against its declared scope and output contract.
> 2. Identify interface mismatches or boundary conflicts between siblings.
> 3. Resolve conflicts — prefer the solution that better matches the parent scope.
> 4. Produce the merged result.
> 5. If output_mode is "code": integrate code, resolve import conflicts, run tests.
>
> ## Output format
>
> ```
> === REUNIFY REPORT ===
> STATUS: MERGED | CONFLICT | PARTIAL
> CONFLICTS: {list of conflicts found, or "none"}
> RESOLUTION: {how conflicts were resolved, or "N/A"}
> SUMMARY: {one-paragraph summary of the merged result}
> === END REUNIFY REPORT ===
> ```

After each reunify subagent returns:
- Write `result.md` in the node's directory.
- Update the manifest.
- Continue bottom-up until the root is reunified.

### Step 5 — Prune & Finalize

1. Review the tree for redundancy: any child whose output was fully absorbed by a sibling can be marked pruned.
2. Write `output.md` in the work directory with the final reunified result.
3. Update `manifest.json` with final metrics:
   ```json
   {
     "status": "complete",
     "metrics": {
       "cycle_time_seconds": 0,
       "tree_depth": 0,
       "total_nodes": 0,
       "leaf_nodes": 0,
       "solve_parallelism": 0,
       "pruned_nodes": 0
     }
   }
   ```
4. Report results to the user.

## Parsing Rules

- DECOMPOSE VERDICT is between `=== DECOMPOSE VERDICT ===` and `=== END DECOMPOSE VERDICT ===`.
- SOLVE REPORT is between `=== SOLVE REPORT ===` and `=== END SOLVE REPORT ===`.
- REUNIFY REPORT is between `=== REUNIFY REPORT ===` and `=== END REUNIFY REPORT ===`.
- If a subagent response doesn't contain the expected block, log the raw response in the manifest and treat the node as FAILED.

## Important Notes

- **No nested subagents.** The orchestrator (you) walks the tree level by level, spawning flat subagents at each step. The spec files on disk ARE the recursion.
- `.fractal/` directory is gitignored — working artifacts are local only.
- SOLVE subagents with `output_mode: code` run in `isolation: worktree` to avoid conflicts.
- SOLVE subagents receive sibling specs as read-only context for interface alignment.
- The decompose phase is inherently sequential (each level depends on the previous). The solve phase can be parallel.
- Each fractal run is independent — concurrent runs use different work IDs.
- Budget enforcement is strict: if `max_total_nodes` is hit, remaining nodes become forced leaves.

## Comparison with Factory Skill

| Aspect | Factory | Fractal |
|--------|---------|---------|
| Shape | Linear pipeline (BUILD → INSPECT) | Recursive tree (DECOMPOSE → SOLVE → REUNIFY) |
| Parallelism | Sequential stations | Parallel leaf solving |
| Rework | INSPECT → BUILD loop (max 3) | No rework — decompose correctly once |
| Output | PR from single implementation | Unified result from merged sub-solutions |
| When to use | Single spec, needs review | Complex task, needs decomposition |

## Composability

The fractal skill composes with the factory skill:

- **Fractal → Factory**: Each leaf sub-spec is implemented via `/factory run` instead of a bare SOLVE subagent. This adds adversarial review to each leaf. To enable, set `solve_mode: factory` in `.fractal.yaml`.
- **Factory → Fractal**: A factory BUILD station can invoke `/fractal decompose` if it determines the spec is too complex for a single pass.
