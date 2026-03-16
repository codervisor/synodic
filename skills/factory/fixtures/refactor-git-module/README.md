---
status: planned
created: 2026-03-16
priority: medium
tags:
- fixture
- refactoring
- rust
parent: 044-factory-skill-mvp
depends_on: []
---

# Refactor — Extract git helpers into dedicated module

> **Status**: planned · **Priority**: medium · **Created**: 2026-03-16

## Overview

The `git()` helper function currently lives in `crates/syn-engine/src/agent.rs`
alongside the `ClaudeAgent` struct. This is a layering violation — git
operations are infrastructure, not agent logic. As the engine grows (worktree
management, branch cleanup, push/PR creation), keeping git helpers in the
agent module will cause it to bloat.

Extract all git-related functions into a new `crates/syn-engine/src/git.rs`
module and re-export from `lib.rs`.

## Design

1. Create `crates/syn-engine/src/git.rs` containing the `git()` function
   (moved from `agent.rs`).
2. Update `agent.rs` to import `git` from the new module instead of defining
   it locally.
3. Update `station.rs` to import `git` from `crate::git` instead of
   `crate::agent`.
4. Add `pub mod git;` to `lib.rs`.
5. Ensure all existing call sites compile without changes to their logic.

Do NOT change function signatures. Do NOT rename anything. This is a
pure move refactor.

## Plan

- [ ] Create `crates/syn-engine/src/git.rs` with the `git()` function moved from `agent.rs`
- [ ] Remove `git()` and its imports (`use std::path::Path`, `tokio::process::Command` if only used by git) from `agent.rs`
- [ ] Update `agent.rs` to not export `git` (remove `pub use` if any call sites used `agent::git`)
- [ ] Update `station.rs` imports: change `use crate::agent::git` to `use crate::git::git`
- [ ] Add `pub mod git;` to `crates/syn-engine/src/lib.rs`
- [ ] Run `cargo build --workspace` to verify compilation
- [ ] Run `cargo test --workspace` to verify no regressions
- [ ] Run `cargo clippy --workspace` with no warnings

## Test

- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` passes with 0 failures
- [ ] `cargo clippy --workspace` produces no warnings
- [ ] `git()` is no longer defined in `agent.rs`
- [ ] `git()` is defined in `git.rs` and publicly exported from `lib.rs`
