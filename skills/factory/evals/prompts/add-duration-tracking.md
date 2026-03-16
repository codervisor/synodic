# Eval: Track Per-Station Duration in History

Run the factory skill on the duration tracking spec:

```
/factory run skills/factory/fixtures/add-duration-tracking/README.md
```

## Why this is hard

- **Serialization backward compatibility**: Adding a field to a `#[derive(Deserialize)]` struct breaks deserialization of existing JSON that lacks the field. The builder MUST add `#[serde(default)]` to `duration_ms`. Forgetting this is a silent bug that only manifests when loading old manifests — the test suite must explicitly cover it.
- **Instant vs DateTime trap**: The spec explicitly requires `std::time::Instant` for measurement, not `chrono::Utc::now()`. Using chrono for elapsed time is the naive approach but it's not monotonic and can produce negative durations if the system clock is adjusted. INSPECT should catch this.
- **Error path duration**: The spec requires capturing duration even when `process_station` returns `Err`. If the builder uses `process_station(item, repo_root).await?` (with the `?` operator), the function returns immediately on error and never records duration. The builder must restructure to: `let result = process_station(...).await; let elapsed = start.elapsed(); /* record */ result?;`
- **Multi-file coordination**: The type change is in `syn-types`, but every place in `syn-engine` that constructs a `StationTransition` must now include the new field. Missing one is a compile error, but the builder must find all construction sites.

## Traps for the builder

1. Forgetting `#[serde(default)]` — tests pass but old manifests break.
2. Using `chrono::Utc::now()` difference instead of `Instant::elapsed()`.
3. Using `?` on `process_station` before capturing elapsed time.
4. Forgetting to update existing test code that constructs `StationTransition` (the round-trip test in conveyor.rs).

## Verify

1. `StationTransition` has `pub duration_ms: u64` with `#[serde(default)]`.
2. Conveyor uses `std::time::Instant` for timing (grep for `Instant::now()`).
3. Duration is computed before error propagation (no `?` between `process_station` and elapsed).
4. Backward-compat deserialization test exists and passes.
5. `cargo test --workspace` passes.
6. `cargo clippy --workspace` has no warnings.
