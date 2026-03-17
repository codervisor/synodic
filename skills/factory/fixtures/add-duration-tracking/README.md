---
status: planned
created: 2026-03-16
priority: high
tags:
- fixture
- serialization
- rust
- multi-file
parent: 044-factory-skill-mvp
depends_on: []
---

# Track Per-Station Duration in History

> **Status**: planned · **Priority**: high · **Created**: 2026-03-16

## Overview

The pipeline records `StationTransition` entries in `WorkItem.history` but
does not track how long each station took to process. This data is essential
for performance profiling — knowing that BUILD took 90s vs INSPECT took 10s
helps optimize the pipeline.

Add a `duration_ms` field to `StationTransition` that records elapsed wall-clock
milliseconds for each station invocation, and compute it correctly in the
conveyor pipeline loop.

## Design

### Type change

In `crates/syn-types/src/lib.rs`, add a field to `StationTransition`:

```rust
pub struct StationTransition {
    pub from: StationId,
    pub to: Option<StationId>,
    pub outcome: StationOutcome,
    pub timestamp: DateTime<Utc>,
    pub tokens_used: u64,
    pub duration_ms: u64,    // NEW: elapsed milliseconds for this station
}
```

### Timing logic

In `crates/syn-engine/src/conveyor.rs`, in `run_pipeline`:

1. Record `std::time::Instant::now()` **before** calling `process_station`.
2. After `process_station` returns (success or error), compute elapsed ms.
3. Include `duration_ms` in the `StationTransition` that gets pushed to history.

**Critical constraint**: Use `std::time::Instant` for elapsed measurement, NOT
`chrono::Utc::now()`. `Instant` is monotonic and immune to clock adjustments.
`DateTime<Utc>` is still used for the `timestamp` field (wall-clock record of
when the transition happened), but duration MUST use `Instant`.

**Error case**: If `process_station` returns an `Err`, the duration should still
be recorded. This means you must capture the `Instant` before the `?` operator.
Restructure the call so that duration is computed even on failure, then re-return
the error after recording. Specifically:

```rust
let start = Instant::now();
let result = process_station(item, repo_root).await;
let duration_ms = start.elapsed().as_millis() as u64;
// ... use duration_ms in the transition ...
// then handle result
```

Do NOT use the `?` operator on `process_station` before computing duration.

### Serialization

`StationTransition` derives `Serialize` and `Deserialize`. Adding a new field
means existing manifests (serialized without `duration_ms`) will fail to
deserialize unless the field has a default. Add `#[serde(default)]` to the
`duration_ms` field so that loading old manifests without this field succeeds
with `duration_ms: 0`.

Do NOT add new crate dependencies. `std::time::Instant` is in the standard
library.

## Plan

- [ ] Add `pub duration_ms: u64` field to `StationTransition` in `crates/syn-types/src/lib.rs`
- [ ] Add `#[serde(default)]` attribute to `duration_ms` for backward-compatible deserialization
- [ ] In `crates/syn-engine/src/conveyor.rs`: capture `Instant::now()` before `process_station`, compute elapsed after
- [ ] Restructure the `process_station` call in `run_pipeline` so duration is captured even when the result is `Err`
- [ ] Include `duration_ms` in every `StationTransition` pushed to `item.history`
- [ ] Add `#[cfg(test)] mod tests` to `crates/syn-types/src/lib.rs` with at least 4 tests:
  - Serialize a `StationTransition` with `duration_ms: 500`, deserialize it, assert it round-trips
  - Deserialize a JSON object WITHOUT `duration_ms` field → should succeed with `duration_ms: 0` (backward compat)
  - Deserialize a JSON object WITH `duration_ms: 1234` → should succeed with correct value
  - Verify `StationTransition` can be constructed with all fields including `duration_ms`
- [ ] Run `cargo test --workspace` and ensure all tests pass
- [ ] Run `cargo clippy --workspace` with no warnings

## Test

- [ ] `cargo test --workspace` passes with 0 failures
- [ ] `cargo clippy --workspace` produces no warnings
- [ ] `StationTransition` has a `duration_ms: u64` field with `#[serde(default)]`
- [ ] The conveyor uses `std::time::Instant` (not `chrono`) for elapsed measurement
- [ ] Duration is captured even when `process_station` returns Err
- [ ] No new crate dependencies added
