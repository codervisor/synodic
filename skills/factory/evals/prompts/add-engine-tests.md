# Eval: Add unit tests to syn-engine

Run the factory skill on the engine tests spec:

```
/factory run skills/factory/fixtures/add-engine-tests/README.md
```

## Why this is hard

- Requires understanding existing code to write meaningful tests
- `parse_claude_output` is a private function — BUILD must figure out how to test it (in-module tests or change visibility)
- The manifest round-trip test needs a temp directory and valid `WorkItem` construction with all required fields (chrono, PathBuf, enums)
- All tests must compile against the real types (`WorkItem`, `StationId`, etc.)
- `cargo test` is the judge — the eval fails if tests don't compile or don't pass

## Verify

1. `crates/syn-engine/src/agent.rs` has a `#[cfg(test)] mod tests` block with at least 5 tests.
2. `crates/syn-engine/src/conveyor.rs` has a `#[cfg(test)] mod tests` block with at least 2 tests.
3. `cargo test --workspace` passes with 0 failures.
4. `cargo clippy --workspace` has no warnings.
5. Tests actually assert meaningful things (not just `assert!(true)`).
