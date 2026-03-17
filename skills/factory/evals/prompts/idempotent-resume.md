# Eval: Idempotent Pipeline Resume from Last Checkpoint

Run the factory skill on the idempotent resume spec:

```
/factory run skills/factory/fixtures/idempotent-resume/README.md
```

## Why this is hard

- **5-way case analysis**: The resume logic must handle 5 distinct outcomes from `history.last()`, each with different behavior. Missing one case is a logic bug. Getting the pattern matching wrong on `StationOutcome` variants (which have associated data) is a common Rust mistake.
- **Rework feedback preservation**: When resuming from a Rework, the builder must clone the feedback string from the `StationOutcome::Rework { feedback, .. }` into `item.rework_feedback`. If they forget this, BUILD will re-run without knowing what to fix — a subtle semantic bug that compiles fine but produces wrong behavior.
- **Attempt counter trap**: The spec explicitly says NOT to increment the attempt counter on resume. The existing pipeline loop increments attempt when it processes a Rework outcome. If the resume logic also increments attempt, it gets double-counted. This is a classic "off-by-one across two code paths" bug.
- **Ownership and cloning**: `StationOutcome` variants contain `String` fields. When matching on `history.last()` (which returns `Option<&StationTransition>`), the builder must clone the strings out since they need to assign owned values to `item.station`, `item.rework_feedback`, etc. Forgetting `.clone()` will fail compilation, but the pattern matching syntax (`if let Some(last) = item.history.last()` then `match &last.outcome { ... }`) requires careful borrowing.
- **Test construction complexity**: Each test must construct a `WorkItem` with a specific `history` containing `StationTransition` entries with the right `outcome` variants. This requires importing and constructing `StationId`, `StationOutcome`, `StationTransition`, `DateTime<Utc>`, etc. — a lot of boilerplate that's easy to get wrong.

## Traps for the builder

1. Forgetting the `Escalate` case and letting it fall into the loop (would restart an escalated pipeline).
2. Incrementing `item.attempt` during Rework resume (double-counting).
3. Not setting `item.rework_feedback` during Rework resume (BUILD loses context).
4. Using `item.history.last_mut()` instead of `item.history.last()` — mutating history during resume is wrong.
5. Putting resume logic inside the loop instead of before it.
6. Testing only the happy path (Pass/Approved) and missing Rework/Escalate tests.

## Verify

1. Resume logic is at the TOP of `run_pipeline`, before the `loop { }` block.
2. All 5 cases are handled: empty history, Pass, Rework, Approved, Escalate.
3. Rework resume sets `item.rework_feedback = Some(feedback.clone())`.
4. Attempt counter is NOT modified during resume.
5. Approved resume returns `Ok(())` immediately.
6. Escalate resume returns `Err(...)`.
7. At least 6 test functions covering all cases.
8. `cargo test --workspace` passes.
9. `cargo clippy --workspace` has no warnings.
10. `run_pipeline` function signature is unchanged.
