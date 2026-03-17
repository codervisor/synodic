# Eval: SWE-bench Pro — Multi-file Bug Fixes (Fractal Decomposition)

## Source

- **Benchmark**: [SWE-bench Pro](https://labs.scale.com/leaderboard/swe_bench_pro_public) (Scale AI, 2025)
- **Dataset**: `scale-labs/SWE-bench_Pro_public` (1,865 tasks)
- **SOTA**: 23.1% resolved (Claude Opus 4.1), 23.3% (GPT-5)
- **Avg complexity**: 4.1 files changed, 107 lines per fix

## Why SWE-bench Pro tests fractal decomposition

SWE-bench Verified (80.9% SOTA) is mostly single-file, localized fixes — too easy for
decomposition to matter. SWE-bench Pro deliberately selects **long-horizon, multi-file** tasks
where current agents struggle. The 57-point drop from Verified → Pro (80.9% → 23.1%)
is exactly the gap that decomposition targets.

Key characteristics:
- Tasks span 4+ files on average
- Solutions require 107 lines of code on average
- Enterprise-level complexity (Django, Flask, scikit-learn, etc.)
- Current agents fail because they lose coherence across files

## How fractal decomposition should help

1. **DECOMPOSE** splits the bug fix into per-file or per-concern sub-problems
2. **SOLVE** agents each handle one concern in isolation (worktree)
3. **REUNIFY** merges fixes and ensures cross-file consistency
4. For simpler tasks, DECOMPOSE correctly detects them as LEAFs (no overhead)

## Running

```bash
# Full e2e with a specific SWE-bench Pro instance
./skills/fractal/evals/e2e/run.sh swe:django__django-16379 --split pro

# Or with any SWE-bench Pro instance ID
./skills/fractal/evals/e2e/run.sh swe:scikit-learn__scikit-learn-25747 --split pro

# Dry run
./skills/fractal/evals/e2e/run.sh swe:django__django-16379 --split pro --dry-run
```

## Scoring

Same as FeatureBench: F2P tests must pass (bug fixed), P2P tests must still pass (no regressions).

## Expected behavior per task complexity

| Task type | Expected decomposition | Expected benefit |
|-----------|----------------------|-----------------|
| 1-2 files, localized | LEAF (no split) | Neutral (no overhead) |
| 3-5 files, related | 2-3 children | Moderate (scope isolation) |
| 5+ files, cross-cutting | 4+ children, depth 2 | High (parallel solving) |

## Control experiment

Run the same tasks WITHOUT fractal decomposition (plain Claude Code) to measure:
- Does decomposition improve resolved rate on multi-file tasks?
- Does it hurt on simple tasks (overhead)?
- What's the crossover point (how many files before decomposition helps)?
