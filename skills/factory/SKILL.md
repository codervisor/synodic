---
name: factory
description: "Coding factory — transforms a spec into a reviewed PR via BUILD → INSPECT pipeline with adversarial review. Use when you need to implement a spec end-to-end with independent code review, or when the user invokes /factory run <spec-path>."
---

# Factory Skill

Implement a spec as a reviewed PR using a two-station assembly line: BUILD (implement + test + commit) → INSPECT (adversarial review with fresh context). Rework up to 3 times if INSPECT finds issues.

## Usage

```
/factory run <spec-path>
```

Example:
```
/factory run specs/044-factory-skill-mvp/README.md
```

## Orchestration Protocol

When invoked, execute the following steps **exactly**:

### Step 1 — Initialize

1. Read the spec at `<spec-path>`.
2. Generate a work ID: `factory-{unix-timestamp}` (e.g., `factory-1710600000`).
3. Create the manifest directory: `.factory/{work-id}/`.
4. Initialize the manifest file at `.factory/{work-id}/manifest.json` with:
   ```json
   {
     "id": "{work-id}",
     "spec": "{spec-path}",
     "status": "building",
     "branch": "factory/{work-id}",
     "attempts": [],
     "metrics": {}
   }
   ```
5. Record the start time for cycle-time measurement.

### Step 2 — BUILD (attempt N)

Spawn a **general-purpose subagent** with `isolation: worktree`:

```
Agent(
  subagent_type: "general-purpose",
  isolation: "worktree",
  prompt: <BUILD_PROMPT below>
)
```

**BUILD_PROMPT:**

> You are the BUILD station of a coding factory. Your job is to implement a spec.
>
> ## Spec
> {full spec content}
>
> ## Rework feedback (if any)
> {rework items from previous INSPECT, or "None — this is the first attempt."}
>
> ## Instructions
>
> 1. Read the spec's Plan section carefully. Each checkbox item is a task.
> 2. If there is rework feedback, focus on addressing those specific items first.
> 3. Implement the code changes described in the plan.
> 4. Run any tests mentioned in the spec's Test section (if applicable and if test infrastructure exists).
> 5. Commit all changes to the current branch with a clear commit message prefixed with `factory:`.
> 6. After committing, run `git diff main...HEAD --stat` to summarize what changed.
>
> ## Output format
>
> End your response with a structured summary block exactly like this:
>
> ```
> === BUILD REPORT ===
> FILES: <comma-separated list of files changed>
> TESTS: PASS | FAIL | SKIPPED
> COMMIT: <short SHA>
> BRANCH: <branch name from the worktree>
> === END BUILD REPORT ===
> ```

After the BUILD subagent returns:
- Extract the BUILD REPORT from its response.
- Record the attempt in the manifest.
- If the subagent reported TESTS: FAIL and no commit was made, record the failure and proceed to rework (go to Step 2 with the failure as rework feedback) if attempts remain.

### Step 3 — INSPECT

Spawn a **general-purpose subagent** (no worktree — read-only review):

```
Agent(
  subagent_type: "general-purpose",
  prompt: <INSPECT_PROMPT below>
)
```

**INSPECT_PROMPT:**

> You are the INSPECT station of a coding factory. You are an adversarial reviewer.
> You have NOT seen the build process — you are reviewing with fresh eyes.
>
> ## Original Spec
> {full spec content}
>
> ## Changes to review
> Run `git diff main...{build-branch}` to see all changes made by the builder.
> Also read the changed files directly to understand the full context.
>
> ## Review dimensions
>
> Evaluate the changes against these criteria:
>
> 1. **Completeness**: Does the implementation address all items in the spec's Plan section?
> 2. **Correctness**: Is the logic correct? Are there bugs, off-by-one errors, or logic flaws?
> 3. **Security**: Are there injection risks, hardcoded secrets, or unsafe operations?
> 4. **Spec conformance**: Does the implementation match what the spec describes, not just "something that works"?
> 5. **Quality**: Is the code clean, well-structured, and maintainable?
>
> ## Output format
>
> End your response with a verdict block exactly like this:
>
> If approved:
> ```
> === INSPECT VERDICT ===
> VERDICT: APPROVE
> SUMMARY: <one-line summary of why it's approved>
> === END INSPECT VERDICT ===
> ```
>
> If rework needed:
> ```
> === INSPECT VERDICT ===
> VERDICT: REWORK
> ITEMS:
> - <specific actionable rework item 1>
> - <specific actionable rework item 2>
> ...
> SUMMARY: <one-line summary of key issues>
> === END INSPECT VERDICT ===
> ```
>
> Be rigorous but fair. Only request rework for genuine issues, not style preferences.
> Each rework item must be specific and actionable — the builder must know exactly what to fix.

After the INSPECT subagent returns:
- Parse the INSPECT VERDICT block.
- Record the inspect result in the manifest.

### Step 4 — Route

Based on the INSPECT verdict:

- **APPROVE**: Go to Step 5.
- **REWORK** and attempts < 3: Go back to Step 2 with the rework items as feedback.
- **REWORK** and attempts >= 3: Go to Step 6 (escalate).

### Step 5 — Create PR

1. Push the build branch: `git push -u origin factory/{work-id}`
2. Create a PR using `gh pr create`:
   ```
   gh pr create \
     --title "factory: {spec title}" \
     --body "## Factory Build Report

   **Spec:** {spec-path}
   **Work ID:** {work-id}
   **Attempts:** {total attempts}
   **First-pass yield:** {yes if approved on attempt 1, no otherwise}

   ## Attempt History
   {formatted attempt history from manifest}

   ---
   *Produced by the Synodic factory skill.*"
   ```
3. Update the manifest with final status and metrics.
4. Report success to the user with the PR URL.

### Step 6 — Escalate

If the rework limit (3 attempts) is reached without approval:

1. Update the manifest: `"status": "escalated"`.
2. Report to the user:
   - The factory could not produce an approved implementation in 3 attempts.
   - Show the rework items from the last INSPECT.
   - The build branch `factory/{work-id}` contains the latest attempt for manual review.
3. Do NOT create a PR.

### Step 7 — Finalize Manifest

After Step 5 or Step 6, update `.factory/{work-id}/manifest.json` with:

```json
{
  "metrics": {
    "cycle_time_seconds": <wall-clock seconds from start to finish>,
    "total_attempts": <number>,
    "first_pass_yield": <true if approved on attempt 1>
  }
}
```

## Parsing Rules

- BUILD REPORT is between `=== BUILD REPORT ===` and `=== END BUILD REPORT ===`.
- INSPECT VERDICT is between `=== INSPECT VERDICT ===` and `=== END INSPECT VERDICT ===`.
- If a subagent response doesn't contain the expected block, treat it as a failure and log the raw response in the manifest.

## Important Notes

- BUILD always runs in `isolation: worktree` so it has its own git branch and working tree.
- INSPECT runs WITHOUT worktree isolation — it reviews by reading the diff, not by modifying files.
- INSPECT must NEVER see the BUILD subagent's reasoning or conversation. It only sees the diff and the spec. This ensures adversarial independence.
- The `.factory/` directory is gitignored — manifests are local artifacts, not committed.
- Each factory run is independent — concurrent runs use different work IDs and branches.
