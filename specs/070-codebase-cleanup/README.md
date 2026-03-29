---
status: draft
created: 2026-03-29
priority: high
tags:
- cleanup
- dead-code
- architecture
- tech-debt
created_at: 2026-03-29T00:00:00Z
updated_at: 2026-03-29T00:00:00Z
depends_on:
- "068"
- "069"
---

# Codebase Cleanup — Remove Dead Code and Misaligned Features

> **Status**: draft · **Priority**: high · **Created**: 2026-03-29

## Overview

Spec 067 repositioned Synodic as a governance platform and extracted coordination patterns to `codervisor/orchestra`. Spec 068 removed L1 infrastructure. But the extraction was incomplete — `orchestra-core` (5,583 LOC) is still in the workspace, pipeline definitions and orchestration skills are still in the repo, and the CLI still exposes `fractal` and `swarm` commands. Additionally, the `meta/` module (2,081 LOC) is an orthogonal AI meta-testing framework that doesn't serve the governance mission, and several `.harness/` scripts are dead.

This spec identifies everything that should be removed, consolidated, or fixed.

## Inventory

### A. Orchestra remnants (should have been extracted in spec 067)

| Item | Size | Status |
|------|------|--------|
| `rust/orchestra-core/` | 5,583 LOC | Full crate still in workspace |
| `rust/Cargo.toml` workspace member `orchestra-core` | — | Still listed |
| `rust/harness-cli/Cargo.toml` dep on `orchestra-core` | — | Still declared |
| `rust/harness-cli/src/cmd/fractal.rs` | 99 LOC | CLI command wrapping orchestra |
| `rust/harness-cli/src/cmd/swarm.rs` | 64 LOC | CLI command wrapping orchestra |
| `Cli::Fractal` / `Cli::Swarm` in `main.rs` | — | Enum variants + dispatch |
| `pipelines/*.yml` (factory, adversarial, fractal, swarm) | 4 files | Pipeline definitions for orchestra |
| `schemas/` (build-report, inspect-verdict, decompose-verdict, etc.) | 9 files | Pipeline output schemas |
| `skills/factory/`, `skills/fractal/`, `skills/swarm/`, `skills/adversarial/` | 4 dirs | Orchestra pipeline skills |

**Total**: ~5,750 LOC + 17 files/dirs that belong in `codervisor/orchestra`.

### B. Meta-testing module (orthogonal to governance)

| Item | Size | Notes |
|------|------|-------|
| `rust/harness-cli/src/meta/mod.rs` | 792 LOC | Orchestration, types, rework loop |
| `rust/harness-cli/src/meta/consult.rs` | 641 LOC | AI consultation, prompt building |
| `rust/harness-cli/src/meta/execute.rs` | 399 LOC | Test execution, infra provisioning |
| `rust/harness-cli/src/meta/validate.rs` | 249 LOC | Result reliability assessment |
| `HarnessSubcommand::Meta` in harness_legacy.rs | ~80 LOC | CLI wiring |
| `mod meta;` in main.rs | — | Module declaration |

**Total**: 2,081 LOC. This is an AI-powered test generation framework — valuable but not governance. It should either be extracted or explicitly scoped as a future feature behind a feature flag.

### C. Dead .harness/ scripts and files

| Item | Notes |
|------|-------|
| `.harness/scripts/evaluate_harness.py` | Called by `harness eval`, thin Python wrapper |
| `.harness/scripts/aggregate_governance.py` | Not referenced anywhere in Rust code |
| `.harness/scripts/simulate_governance.py` | Not referenced anywhere in Rust code |
| `.harness/scripts/harness_eval_runner.sh` | Not referenced anywhere in Rust code |
| `.harness/eval.governance.jsonl` | 0 bytes, eval was extracted |
| `.harness/harness.governance.jsonl` | 0 bytes, never written to by current code |
| `.harness/gates.yml` | Orchestra's preflight gate config, empty `preflight: []` |
| `.harness/templates/` | Empty directory |

### D. Harness legacy naming

| Item | Notes |
|------|-------|
| `cmd/harness_legacy.rs` | Named "legacy" but contains the core governance engine |
| `HarnessSubcommand::Eval` | Delegates to Python script that may not exist |
| `HarnessSubcommand::Rules` | Lists `.harness/rules/` which was removed in spec 068 |

### E. PostgreSQL unsafe code

`rust/harness-core/src/storage/postgres.rs` uses `unsafe` to cast `&self` to `&mut self` (5 occurrences) instead of proper interior mutability. This is unsound and can cause undefined behavior under concurrent access.

### F. Copilot parser misalignment

`rust/harness-core/src/parsers/copilot.rs` detects L1 concerns (tool errors, command failures, content filter blocks) that per spec 068 belong to git hooks and CI, not Synodic's L2 layer.

## Plan

### Phase 1: Remove orchestra remnants

- [ ] Delete `rust/orchestra-core/` directory
- [ ] Remove `orchestra-core` from `rust/Cargo.toml` workspace members
- [ ] Remove `orchestra-core` dependency from `rust/harness-cli/Cargo.toml`
- [ ] Delete `rust/harness-cli/src/cmd/fractal.rs`
- [ ] Delete `rust/harness-cli/src/cmd/swarm.rs`
- [ ] Remove `Fractal` and `Swarm` variants from `Cli` enum in `main.rs`
- [ ] Remove `fractal`/`swarm` from `cmd/mod.rs`
- [ ] Delete `pipelines/` directory (4 YAML files)
- [ ] Delete `schemas/` directory (9 JSON files) — these are orchestra pipeline schemas
- [ ] Delete `skills/factory/`, `skills/fractal/`, `skills/swarm/`, `skills/adversarial/`
- [ ] Update CLAUDE.md: remove orchestra references, pipeline topologies section, fractal/swarm CLI commands

### Phase 2: Remove or quarantine meta-testing

- [ ] Delete `rust/harness-cli/src/meta/` directory (2,081 LOC)
- [ ] Remove `mod meta;` from `main.rs`
- [ ] Remove `HarnessSubcommand::Meta` from `harness_legacy.rs`
- [ ] Remove meta-related dependencies from `Cargo.toml` if any are exclusive to meta

### Phase 3: Clean dead .harness/ files

- [ ] Delete `.harness/scripts/aggregate_governance.py`
- [ ] Delete `.harness/scripts/simulate_governance.py`
- [ ] Delete `.harness/scripts/harness_eval_runner.sh`
- [ ] Delete `.harness/eval.governance.jsonl` (eval was extracted)
- [ ] Delete `.harness/gates.yml` (orchestra's gate config, empty)
- [ ] Delete `.harness/templates/` (empty directory)
- [ ] Evaluate `.harness/scripts/evaluate_harness.py` — remove if `harness eval` is removed

### Phase 4: Fix harness legacy command

- [ ] Rename `harness_legacy.rs` → `governance.rs`, `HarnessCmd` → `GovernanceCmd`
- [ ] Remove `HarnessSubcommand::Eval` (Python wrapper for removed eval framework)
- [ ] Remove `HarnessSubcommand::Rules` (lists `.harness/rules/` which no longer exists)
- [ ] Keep `Run` and `Log` — these are core governance features
- [ ] Rename CLI: `synodic harness run` → `synodic governance run` (alias `harness` for backwards compat)

### Phase 5: Fix unsafe PostgreSQL code

- [ ] Replace `unsafe { &mut *(self as *const Self as *mut Self) }` with `RefCell<Client>`
- [ ] Wrap `Client` in `RefCell` in `PostgresStore` struct
- [ ] Use `self.client.borrow_mut()` in all trait methods
- [ ] Verify all 5 unsafe occurrences are eliminated
- [ ] Run tests to confirm no regressions

### Phase 6: Fix copilot parser alignment

- [ ] Remove generic error/outcome detection (L1 concern → hooks/CI)
- [ ] Remove content_filter → ComplianceViolation mapping (GitHub's enforcement, not Synodic's)
- [ ] Focus parser on L2-relevant signals: hallucination patterns, misalignment indicators
- [ ] Update tests to reflect new scope

## Test

- [ ] `cargo build --workspace` succeeds after all removals
- [ ] `cargo test --workspace` passes (no broken imports/references)
- [ ] `synodic --help` shows no fractal/swarm/meta commands
- [ ] `synodic governance run` works (renamed from `harness run`)
- [ ] `synodic governance log` works (renamed from `harness log`)
- [ ] No `unsafe` blocks remain in postgres.rs
- [ ] Copilot parser only emits L2-relevant events (no ToolCallError from generic errors)
- [ ] `.harness/` directory contains only: README.md, harness.governance.jsonl, scripts/evaluate_harness.py (if kept)
- [ ] No references to orchestra, fractal, swarm, pipeline, factory, adversarial in Rust source

## Notes

- **LOC removed**: ~7,800 (orchestra-core 5,583 + meta 2,081 + CLI commands 163)
- **Files removed**: ~30+ (orchestra crate, pipelines, schemas, skills, scripts)
- The `harness eval` → `evaluate_harness.py` pattern is an anti-pattern (Rust CLI shelling out to Python). If eval functionality is needed, it should be reimplemented in Rust or removed entirely.
- `harness.governance.jsonl` is 0 bytes — the governance log is never written to by current code. The `harness run` command writes to `.runs/` manifests instead. Consider whether the JSONL log is still needed or if `.runs/` is the canonical persistence layer.
- The `Search` command largely overlaps with `List`. Consider merging `Search` into `List --search <query>` in a future cleanup, but this is low priority.
