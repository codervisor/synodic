---
status: planned
created: 2026-03-18
priority: high
tags:
- architecture
- eval
- refactor
created_at: 2026-03-18T22:30:33.987624411Z
updated_at: 2026-03-18T22:30:33.987624411Z
---

# Decouple Eval as Standalone Testing Framework

## Overview

The eval framework (setup → agent → score pipeline) is a general-purpose AI coding evaluation tool that should work independently of synodic's governance harness. Today they share a single binary, a single Cargo.toml, and a shared util.rs — making it impossible to use eval without pulling in harness code and conventions.

**Why now:** Eval is mature enough (29 tests, 3 benchmarks, batch mode) to stand alone. Decoupling enables:
- Independent versioning and release cycles
- Use by external teams without adopting synodic governance
- Cleaner dependency graphs (eval doesn't need harness's rule engine)
- Synodic can dogfood eval as a consumer, not as a co-resident

## Design

### Current coupling points (none are deep)

| Coupling | Mechanism | Severity |
|----------|-----------|----------|
| Single binary | `Cli { Harness, Eval }` in main.rs | Low — just dispatch |
| Shared util.rs | `find_repo_root()`, `exec_script()` | Low — 2 functions, ~40 lines |
| Governance logs | eval writes `.harness/eval.governance.jsonl` | Medium — format assumption |
| SYNODIC_ROOT env var | harness sets → eval reads via util.rs | Medium — naming/convention |
| Shared Cargo.toml | One crate, all deps mixed | Low — no conflicting deps |

### Target architecture: Cargo workspace with two crates

```
cli/
├── Cargo.toml              # [workspace] members = ["synodic", "synodic-eval"]
├── synodic/                # Harness binary crate
│   ├── Cargo.toml          # depends on synodic-eval (optional, for `synodic eval` passthrough)
│   └── src/
│       ├── main.rs         # Cli { Harness, Eval } — eval delegates to synodic-eval
│       ├── cmd/harness.rs
│       ├── harness/        # unchanged
│       └── util.rs         # harness-specific utils only
├── synodic-eval/           # Eval library + binary crate
│   ├── Cargo.toml          # standalone deps (serde, regex, quick-xml, chrono)
│   └── src/
│       ├── lib.rs          # Public API: run(), score(), setup(), list(), batch()
│       ├── main.rs         # Standalone binary: `synodic-eval run`, `synodic-eval score`
│       ├── run.rs
│       ├── batch.rs
│       ├── list.rs
│       ├── report.rs
│       ├── score/          # parser, runner, verdict, report — unchanged
│       ├── setup/          # swebench, featurebench, devbench — unchanged
│       └── util.rs         # eval-specific: find_project_root() (no .harness assumption)
```

### Key design decisions

**1. Governance log integration becomes a plugin/callback, not hardcoded**

Currently `eval/run.rs` directly writes to `.harness/eval.governance.jsonl`. Instead:

```rust
// synodic-eval exposes a trait
pub trait EvalReporter {
    fn on_verdict(&self, verdict: &EvalVerdict, metrics: &RunMetrics) -> Result<()>;
}

// Default: JSON to stdout (standalone mode)
pub struct StdoutReporter;

// Synodic provides: JSONL to .harness/ (governance mode)
pub struct GovernanceReporter { harness_dir: PathBuf }
```

This way eval doesn't know about `.harness/` at all. Synodic wires in its own reporter when invoking eval.

**2. SYNODIC_ROOT → EVAL_PROJECT_ROOT (generic naming)**

Eval should use a generic env var like `EVAL_PROJECT_ROOT` to find the project root. Synodic's harness can set both `SYNODIC_ROOT` and `EVAL_PROJECT_ROOT` for backwards compatibility.

**3. `synodic eval` becomes a thin passthrough**

The synodic binary keeps the `eval` subcommand for convenience but delegates to `synodic-eval` (either as a library dependency or subprocess). This preserves the `synodic eval run` UX while eval is independently usable as `synodic-eval run`.

**4. evals/ directory stays with synodic (task registry is project-specific)**

The `evals/evals.json` task registry and `evals/tasks/` definitions are project-specific. synodic-eval reads them from a configurable path (default: `./evals/evals.json`), not from a hardcoded location.

## Plan

- [ ] Create Cargo workspace with `synodic` and `synodic-eval` member crates
- [ ] Move eval modules (run, batch, list, report, score/, setup/) to synodic-eval crate
- [ ] Extract eval-specific util functions into synodic-eval/src/util.rs
- [ ] Define EvalReporter trait; replace hardcoded governance log writes with trait calls
- [ ] Implement StdoutReporter (default) and GovernanceReporter (synodic-specific)
- [ ] Rename SYNODIC_ROOT to EVAL_PROJECT_ROOT in eval crate; add compat in harness
- [ ] Wire synodic binary to depend on synodic-eval as library for `synodic eval` passthrough
- [ ] Update evals.json path to be configurable (--evals-file flag, env var, default)
- [ ] Verify all 29 existing tests pass in the new synodic-eval crate
- [ ] Update CLAUDE.md and docs to reflect new architecture

## Test

- [ ] `cd cli/synodic-eval && cargo test` — all 29 tests pass standalone
- [ ] `cd cli/synodic-eval && cargo build` — produces standalone `synodic-eval` binary
- [ ] `synodic-eval run` works without .harness/ directory present
- [ ] `synodic eval run` still works (passthrough from main binary)
- [ ] GovernanceReporter writes correct JSONL when invoked through synodic harness
- [ ] EVAL_PROJECT_ROOT env var respected; SYNODIC_ROOT still works as fallback

## Notes

**Alternatives considered:**
- **Feature flags instead of workspace:** Could use `#[cfg(feature = "harness")]` to conditionally compile governance integration. Rejected — doesn't give eval its own binary or independent release.
- **Separate repo:** Too aggressive for now. Workspace keeps them co-developed while allowing independent builds. Can extract to separate repo later if needed.
- **Git subtree/submodule:** Adds git complexity without clear benefit at this stage.

**Migration path:** This is a pure refactor — no behavioral changes. All existing `synodic eval` commands continue working. New `synodic-eval` binary is an addition, not a replacement.
