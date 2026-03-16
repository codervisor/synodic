# Eval: Enforce Legal State Transitions with Exhaustive Guard

Run the factory skill on the transition guard spec:

```
/factory run skills/factory/fixtures/transition-guard/README.md
```

## Why this is hard

- **Cross-crate coordination**: Function defined in `syn-types`, called from `syn-engine/conveyor.rs`. Missing the import or getting the crate boundary wrong will fail compilation.
- **Conditional validity trap**: `Escalate` is legal from both stations but ONLY when `attempt >= 3`. A naive implementation that allows Escalate unconditionally will be caught by INSPECT. Conversely, one that blocks Escalate entirely will also fail.
- **Exhaustive coverage**: The spec demands at least 10 tests covering both legal AND illegal transitions. Most builders will test the happy paths but forget to test that illegal transitions actually return `Err`.
- **No changes to existing types**: The spec explicitly forbids modifying `StationId` or `StationOutcome`. The function must work with the existing enum variants.
- **Match arm completeness**: The validation function must handle all combinations of `StationId × StationOutcome`. Missing a case means either a compiler error (if using exhaustive matching) or a logic bug (if using wildcards).

## Traps for the builder

1. Using a catch-all `_ => Ok(())` that accidentally allows illegal transitions.
2. Forgetting the `attempt` parameter in the Escalate check.
3. Not handling `Pass { next: Build }` as illegal (Build coming after Build via Pass makes no sense).
4. Putting the validation call AFTER recording history instead of BEFORE.

## Verify

1. `validate_transition` exists in `crates/syn-types/src/lib.rs` with the correct signature.
2. All 6 legal transitions return `Ok(())`.
3. All illegal transitions return `Err(...)` with a descriptive message.
4. `run_pipeline` in conveyor.rs calls `validate_transition` before pushing to history.
5. At least 10 test functions in syn-types.
6. `cargo test --workspace` passes.
7. `cargo clippy --workspace` has no warnings.
8. No existing enum variants or function signatures were changed.
