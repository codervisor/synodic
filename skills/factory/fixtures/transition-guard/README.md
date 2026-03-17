---
status: planned
created: 2026-03-16
priority: high
tags:
- fixture
- state-machine
- rust
- cross-crate
parent: 044-factory-skill-mvp
depends_on: []
---

# Enforce Legal State Transitions with Exhaustive Guard

> **Status**: planned · **Priority**: high · **Created**: 2026-03-16

## Overview

The pipeline currently accepts any `StationOutcome` from any station without
validation. This is a correctness gap — nothing prevents `process_build` from
returning `StationOutcome::Approved` (only Inspect should approve) or
`process_inspect` from returning `Pass { next: StationId::Build }` (Inspect
doesn't "pass" work backwards via Pass). Additionally, `Escalate` is only
semantically valid when the rework limit has been reached.

Add a `validate_transition` function that enforces a precise transition table,
and wire it into the conveyor pipeline so every transition is checked before
being recorded.

## Design

### Transition table

The following transitions are **legal**:

| From station | Outcome | Condition |
|---|---|---|
| Build | `Pass { next: Inspect }` | Always |
| Build | `Rework { back_to: Build, .. }` | Always |
| Build | `Escalate { .. }` | Only when `attempt >= 3` |
| Inspect | `Approved` | Always |
| Inspect | `Rework { back_to: Build, .. }` | Always |
| Inspect | `Escalate { .. }` | Only when `attempt >= 3` |

All other combinations are **illegal**. Specifically:

- `Build` CANNOT produce `Approved` (only Inspect can approve).
- `Inspect` CANNOT produce `Pass { next: .. }` (Inspect doesn't pass forward).
- `Rework { back_to: Inspect }` is ALWAYS illegal from any station (you can
  only rework back to Build).
- `Pass { next: Build }` is ALWAYS illegal (Build doesn't come after Build via
  Pass).
- `Escalate` from either station when `attempt < 3` is illegal (premature
  escalation).

### Function signature

Add this function to `crates/syn-types/src/lib.rs`:

```rust
pub fn validate_transition(
    from: &StationId,
    outcome: &StationOutcome,
    attempt: u32,
) -> Result<(), String>
```

Returns `Ok(())` for legal transitions, `Err(description)` for illegal ones.

### Integration

In `crates/syn-engine/src/conveyor.rs`, call `validate_transition` after
`process_station` returns and BEFORE recording the transition in history. If
validation fails, return an `anyhow::bail!` with the error message.

Do NOT change `process_station`, `StationId`, `StationOutcome`, or any
existing function signatures. Do NOT add new crate dependencies.

## Plan

- [ ] Add `validate_transition(from, outcome, attempt) -> Result<(), String>` to `crates/syn-types/src/lib.rs`
- [ ] Implement the transition table logic with all 6 legal transitions and rejection of all illegal ones
- [ ] In `crates/syn-engine/src/conveyor.rs`: call `validate_transition` in `run_pipeline` after `process_station` returns, before recording history
- [ ] Add `#[cfg(test)] mod tests` to `crates/syn-types/src/lib.rs` with at least 10 test cases covering:
  - All 6 legal transitions (should return `Ok`)
  - Build → Approved (illegal, should return Err)
  - Inspect → Pass{next: Inspect} (illegal, should return Err)
  - Rework{back_to: Inspect} from Build (illegal, should return Err)
  - Escalate from Build when attempt=1 (illegal — premature escalation)
  - Escalate from Inspect when attempt=3 (legal — at rework limit)
  - Escalate from Inspect when attempt=2 (illegal — premature)
- [ ] Run `cargo test --workspace` and ensure all tests pass
- [ ] Run `cargo clippy --workspace` with no warnings

## Test

- [ ] `cargo test --workspace` passes with 0 failures
- [ ] `cargo clippy --workspace` produces no warnings
- [ ] At least 10 test functions exist in syn-types
- [ ] The conveyor calls `validate_transition` before recording history
- [ ] No existing function signatures or enum variants were changed
