---
status: planned
created: 2026-03-16
priority: high
tags:
- fixture
- bugfix
- rust
parent: 044-factory-skill-mvp
depends_on: []
---

# Bugfix — truncate() panics on multi-byte UTF-8

> **Status**: planned · **Priority**: high · **Created**: 2026-03-16

## Overview

The `truncate` function in `crates/syn-engine/src/station.rs` panics when
slicing a string at a byte offset that falls inside a multi-byte UTF-8
character. For example, truncating `"héllo world"` at byte offset 2 will
panic because `é` occupies bytes 1-2 (two bytes in UTF-8) and slicing at
`&s[..2]` lands inside the character.

This is a real production risk because BUILD and INSPECT agent outputs
regularly contain non-ASCII characters (em dashes, smart quotes, emoji,
non-English text in reviews).

## Design

Fix `truncate()` in `crates/syn-engine/src/station.rs` to use a
UTF-8-safe truncation strategy. The function must:

1. Never panic on any valid `&str` input.
2. Truncate to at most `max` **bytes** (not characters) to preserve the
   existing size-bounding semantics.
3. If `max` falls inside a multi-byte character, round down to the nearest
   character boundary.
4. Still append `"... (truncated)"` when the string was actually truncated.

Do NOT change the function signature. Do NOT add external crate dependencies.

## Plan

- [ ] Fix `truncate()` in `crates/syn-engine/src/station.rs` to find the nearest char boundary at or before `max`
- [ ] Add unit tests in `crates/syn-engine/src/station.rs` covering:
  - ASCII string shorter than max (no truncation)
  - ASCII string longer than max (truncated)
  - Multi-byte string where max falls inside a character (no panic, rounds down)
  - Empty string
  - String that is exactly max length
- [ ] Run `cargo test --workspace` and ensure all tests pass
- [ ] Run `cargo clippy --workspace` with no warnings

## Test

- [ ] `cargo test --workspace` passes with 0 failures
- [ ] `cargo clippy --workspace` produces no warnings
- [ ] The new tests explicitly cover a multi-byte truncation case without panic
