---
status: planned
created: 2026-03-16
priority: high
tags:
- fixture
- pipeline
- rust
- edge-cases
parent: 044-factory-skill-mvp
depends_on: []
---

# Idempotent Pipeline Resume from Last Checkpoint

> **Status**: planned · **Priority**: high · **Created**: 2026-03-16

## Overview

If the process crashes or is interrupted mid-pipeline, calling `run_pipeline`
again with the same `WorkItem` (loaded from its manifest) should resume from
where it left off — not replay completed stations. Currently, `run_pipeline`
always starts processing from `item.station` in an infinite loop, but it
doesn't skip work that the history already records as completed.

Add resume logic to `run_pipeline` that inspects the existing `history` to
determine where to pick up.

## Design

### Resume rules

At the **start** of `run_pipeline` (before entering the loop), examine
`item.history` to determine the resume point:

1. **Empty history**: No work done yet. Start normally (no change to current
   behavior).

2. **Last entry outcome is `Pass { next }`**: The station completed and was
   about to advance. Set `item.station = next` and continue from there. Do NOT
   re-run the station that produced the Pass.

3. **Last entry outcome is `Rework { back_to, feedback }`**: A rework was
   requested. Set `item.station = back_to` and `item.rework_feedback =
   Some(feedback.clone())`. The attempt counter should already reflect the
   rework (it was incremented when the rework was recorded). Do NOT increment
   attempt again.

4. **Last entry outcome is `Approved`**: Pipeline already finished successfully.
   Return `Ok(())` immediately without entering the loop. Log a message:
   `"Pipeline already approved, nothing to resume."`

5. **Last entry outcome is `Escalate { .. }`**: Pipeline already failed. Return
   an error: `"Pipeline was previously escalated, cannot resume."` Do NOT
   re-enter the loop.

### Critical constraints

- Do NOT modify `process_station`, the station functions, or any type
  definitions.
- Do NOT change the function signature of `run_pipeline`.
- The resume logic goes at the TOP of `run_pipeline`, before the existing loop.
- Do NOT re-increment `item.attempt` during resume. The attempt counter was
  already updated when the transition was originally recorded.
- The resume logic must handle `item.history` being a `Vec` — use `.last()` to
  get the most recent entry.
- After resume repositioning, the existing loop takes over normally.

### Edge case: Rework feedback preservation

When resuming from a Rework, the `rework_feedback` field in `WorkItem` must be
set from the Rework outcome's feedback string. This is critical because the
BUILD station reads `item.rework_feedback` to know what to fix. If the
feedback is lost on resume, BUILD will repeat the same mistakes.

The `StationOutcome::Rework { feedback, .. }` field contains the feedback
string. Clone it into `item.rework_feedback = Some(feedback.clone())`.

## Plan

- [ ] At the top of `run_pipeline` in `crates/syn-engine/src/conveyor.rs`, add resume logic that checks `item.history.last()`
- [ ] Handle all 5 cases: empty history, Pass, Rework, Approved, Escalate
- [ ] For Pass: set `item.station` to the `next` station from the outcome
- [ ] For Rework: set `item.station` to `back_to` AND set `item.rework_feedback` from the feedback string
- [ ] For Approved: return `Ok(())` immediately with a log message
- [ ] For Escalate: return an error
- [ ] Do NOT increment `item.attempt` during resume
- [ ] Add `#[cfg(test)] mod tests` in `conveyor.rs` (or extend existing tests) with at least 6 tests:
  - Resume with empty history → station unchanged (Build stays Build)
  - Resume after Pass{next: Inspect} → station becomes Inspect
  - Resume after Rework{back_to: Build, feedback: "fix X"} → station becomes Build, rework_feedback is Some("fix X")
  - Resume after Approved → returns Ok immediately
  - Resume after Escalate → returns Err
  - Resume after Rework does NOT increment attempt (set attempt=2, resume, verify still 2)
- [ ] Run `cargo test --workspace` and ensure all tests pass
- [ ] Run `cargo clippy --workspace` with no warnings

## Test

- [ ] `cargo test --workspace` passes with 0 failures
- [ ] `cargo clippy --workspace` produces no warnings
- [ ] Resume logic is at the top of `run_pipeline`, before the loop
- [ ] All 5 history cases are handled
- [ ] Rework feedback is preserved on resume
- [ ] Attempt counter is NOT re-incremented on resume
- [ ] `run_pipeline` function signature is unchanged
- [ ] No type definitions were modified
