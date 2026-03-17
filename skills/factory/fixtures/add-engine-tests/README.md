---
status: planned
created: 2026-03-16
priority: medium
tags:
- fixture
- testing
- rust
parent: 044-factory-skill-mvp
depends_on: []
---

# Add Unit Tests to syn-engine

> **Status**: planned · **Priority**: medium · **Created**: 2026-03-16

## Overview

The `syn-engine` crate has zero test coverage. The `parse_claude_output`
function in `agent.rs` and the `save_manifest` / `load_manifest` round-trip
in `conveyor.rs` are pure-logic functions that can be unit-tested without
spawning real processes.

This spec adds a focused test suite that validates the parsing and
serialization logic.

## Design

Add `#[cfg(test)] mod tests` blocks to the relevant source files. Tests
must use only the standard library and existing workspace dependencies
(serde_json, chrono, etc.) — no new crates.

### parse_claude_output (agent.rs)

The function parses JSON emitted by `claude -p --output-format json`. Test
cases:

1. **Valid complete JSON** — `{"result": "done", "usage": {"input_tokens": 100, "output_tokens": 50}}` → result_text="done", tokens_used=150
2. **Missing result field** — `{"usage": {"input_tokens": 10, "output_tokens": 5}}` → result_text="" (default), tokens_used=15
3. **Missing usage field** — `{"result": "hello"}` → result_text="hello", tokens_used=0
4. **Empty JSON object** — `{}` → result_text="", tokens_used=0
5. **Invalid JSON** — `"not json at all"` → returns Err

### save_manifest / load_manifest round-trip (conveyor.rs)

1. Create a `WorkItem` with known values, save it to a temp directory, load
   it back, and assert all fields match.
2. Verify that loading from a nonexistent path returns an error.

## Plan

- [ ] Add `#[cfg(test)] mod tests` to `crates/syn-engine/src/agent.rs` with 5 test cases for `parse_claude_output`
- [ ] Make `parse_claude_output` visible to tests (it's currently private — add `pub(crate)` or test in-module)
- [ ] Add `#[cfg(test)] mod tests` to `crates/syn-engine/src/conveyor.rs` with 2 test cases for manifest round-trip
- [ ] Run `cargo test --workspace` and ensure all tests pass
- [ ] Run `cargo clippy --workspace` with no warnings

## Test

- [ ] `cargo test --workspace` passes with 0 failures
- [ ] `cargo clippy --workspace` produces no warnings
- [ ] At least 7 new test functions exist across the two files
