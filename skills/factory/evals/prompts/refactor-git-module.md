# Eval: Refactor — Extract git helpers into dedicated module

Run the factory skill on the refactoring spec:

```
/factory run skills/factory/fixtures/refactor-git-module/README.md
```

## Why this is hard

- Refactoring across multiple Rust files requires understanding the dependency graph
- `station.rs` imports `git` from `crate::agent` — this import path must change
- `agent.rs` re-exports `git` publicly — external callers (like `main.rs`) may break
- The Rust compiler will catch any missed import, but BUILD must navigate the errors
- INSPECT must verify this is a pure move (no logic changes, no signature changes)

## Verify

1. `crates/syn-engine/src/git.rs` exists and contains the `git()` function.
2. `crates/syn-engine/src/agent.rs` no longer defines `git()`.
3. `crates/syn-engine/src/station.rs` imports from `crate::git`, not `crate::agent`.
4. `crates/syn-engine/src/lib.rs` has `pub mod git;`.
5. `cargo build --workspace` succeeds.
6. `cargo test --workspace` passes.
7. `cargo clippy --workspace` has no warnings.
8. No logic was changed — only import paths moved.
