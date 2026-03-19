# Eval: SWE-bench Pro — Primary Benchmark (Fractal + Factory)

> **Status**: Primary benchmark for all Synodic skill evaluations.
> See [evaluation-strategy.md](../../docs/evaluation/evaluation-strategy.md) for the full strategy.

## Source

- **Benchmark**: [SWE-bench Pro](https://labs.scale.com/leaderboard/swe_bench_pro_public) (Scale AI, 2025)
- **Dataset**: `scale-labs/SWE-bench_Pro_public` (1,865 tasks, 41 repos)
- **Private set**: 276 instances from proprietary codebases (requires Scale API)
- **SOTA**: ~23% resolved (public), ~15-18% resolved (private)
- **Avg complexity**: 4.1 files changed, 107 lines per fix
- **Languages**: Python, Go, TypeScript, JavaScript

## Why SWE-bench Pro is our primary benchmark

### 1. Contamination resistance

SWE-bench Verified (~80% SOTA) is likely inflated by training data contamination.
SWE-bench Pro addresses this:

- **Public set**: GPL-licensed repos — legal deterrent against training inclusion
- **Private set**: proprietary codebases — physically unseen by any model

The 57-point drop from Verified → Pro (80% → 23%) and the further drop on the private
set (23% → 15%) quantify the contamination effect.

### 2. Multi-file, long-horizon tasks

Tasks span 4+ files on average with 107-line solutions. Current agents fail because
they lose coherence across files — exactly the gap orchestration targets.

### 3. Diverse task types

Bug fixes, feature requests, security patches, performance optimizations, UI changes.
Not just "fix this failing test."

### 4. Rigorous test validation

Full test suite must pass (not just modified tests), with 3 reruns per instance to
eliminate flaky tests. This closes the ~11% false-positive gap found in SWE-bench
Verified.

## How Synodic skills map to SWE-bench Pro

### Fractal decomposition (multi-file tasks)

1. **DECOMPOSE** splits the bug fix into per-file or per-concern sub-problems
2. **SOLVE** agents each handle one concern in isolation (worktree)
3. **REUNIFY** merges fixes and ensures cross-file consistency
4. For simpler tasks, DECOMPOSE correctly detects them as LEAFs (no overhead)

### Factory (single-file / localized tasks)

1. **BUILD** implements the fix
2. **INSPECT** reviews for correctness, security, spec conformance
3. Rework loop if INSPECT finds issues

### Routing heuristic

| Task complexity | Skill | Rationale |
|----------------|-------|-----------|
| 1-2 files, localized | Factory | No decomposition needed |
| 3-5 files, related | Fractal (shallow) | Scope isolation helps |
| 5+ files, cross-cutting | Fractal (deep) | Parallel solving + reunification |

## Running

```bash
# Full e2e with fractal decomposition
./evals/run.sh swe:<instance-id> --split pro --skill fractal

# Same task with factory
./evals/run.sh swe:<instance-id> --split pro --skill factory

# Baseline (no skill)
./evals/run.sh swe:<instance-id> --split pro --skill baseline

# Dry run
./evals/run.sh swe:<instance-id> --split pro --dry-run

# Score only
./evals/score.sh <instance-id> --testbed-dir /tmp/swebench-testbed/<instance-id>
```

### Curated eval instances

These instances are registered in `evals.json` and cover a range of complexities:

| Instance ID | Repo | Complexity | Notes |
|-------------|------|-----------|-------|
| `django__django-16379` | Django | High (multi-file) | ORM + migrations |
| `scikit-learn__scikit-learn-25747` | scikit-learn | High (multi-file) | ML pipeline internals |
| `pallets__flask-5063` | Flask | Medium | Routing + error handling |

## Scoring

| Test type | Description | Required |
|-----------|-------------|----------|
| **F2P** (Fail-to-Pass) | Tests that should pass after the fix | Yes |
| **P2P** (Pass-to-Pass) | Existing tests — no regressions | Yes |
| **Full suite** | All repo tests pass | Yes (SWE-bench Pro) |
| **Rerun** | 3x rerun to catch flakiness | Yes (SWE-bench Pro) |

`resolved = F2P_all_pass AND P2P_all_pass AND full_suite_pass`

## Expected behavior per task complexity

| Task type | Expected decomposition | Expected benefit |
|-----------|----------------------|-----------------|
| 1-2 files, localized | LEAF (no split) | Neutral (no overhead) |
| 3-5 files, related | 2-3 children | Moderate (scope isolation) |
| 5+ files, cross-cutting | 4+ children, depth 2 | High (parallel solving) |

## Control experiment

Run the same tasks WITHOUT orchestration (plain Claude Code) to measure:

- Does decomposition improve resolve rate on multi-file tasks?
- Does it hurt on simple tasks (overhead)?
- What's the crossover point (how many files before decomposition helps)?
- Does Factory's INSPECT catch issues that plain Claude Code misses?

## Comparison with FeatureBench

FeatureBench (ICLR 2026) remains useful for testing feature addition tasks specifically,
but SWE-bench Pro supersedes it as the primary evaluation signal because:

1. Larger dataset (1,865 vs ~500 tasks)
2. Higher contamination resistance
3. Multi-language coverage
4. More diverse task types (not just feature additions)
5. Stricter test validation

See [evaluation-strategy.md](../../docs/evaluation/evaluation-strategy.md) for the full benchmark
hierarchy and evaluation priorities.
