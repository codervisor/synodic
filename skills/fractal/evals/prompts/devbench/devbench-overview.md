# Eval: DevBench — Full Project Build from PRD (Fractal Decomposition)

## Source

- **Benchmark**: [DevBench](https://github.com/open-compass/DevBench) (2024)
- **Tasks**: 22 curated projects across Python, C/C++, Java, JavaScript
- **SOTA**: <40% (all models performed poorly as of 2024)
- **Complexity**: Build entire multi-file projects from Product Requirements Documents

## Why DevBench is the best fit for fractal decomposition

DevBench is the **most natural benchmark** for fractal decomposition because:

1. **PRDs decompose into modules.** A product spec naturally splits into
   auth, data layer, API, UI, etc. — exactly what DECOMPOSE produces.

2. **Modules are independently buildable.** Each module can be implemented
   in an isolated worktree by a SOLVE agent.

3. **Integration is the hard part.** REUNIFY's job — merging modules with
   consistent interfaces — is precisely what makes DevBench hard for agents.

4. **Current agents fail catastrophically.** <40% SOTA means there's massive
   room for improvement. Agents lose coherence when building 10+ files.

## How fractal decomposition maps to DevBench

```
PRD → DECOMPOSE → Module specs
  ├── auth-module/     → SOLVE (worktree) → auth code
  ├── data-layer/      → SOLVE (worktree) → data code
  ├── api-gateway/     → SOLVE (worktree) → API code
  └── cli-interface/   → SOLVE (worktree) → CLI code
                       → REUNIFY → integrated project
```

## Running

```bash
# DevBench project (e.g., TextCNN — a text classification CNN)
./skills/fractal/evals/e2e/run.sh dev:TextCNN

# Dry run
./skills/fractal/evals/e2e/run.sh dev:TextCNN --dry-run
```

## Scoring

DevBench scoring differs from SWE-bench/FeatureBench:

| Dimension | Metric | Weight |
|-----------|--------|--------|
| **Build** | Does the project compile/install? | Required |
| **Acceptance tests** | Do the acceptance tests pass? | Required |
| **Code quality** | LLM-judge evaluation (optional) | Bonus |

`resolved = build_success AND all_acceptance_tests_pass`

## Available projects

DevBench includes 22 projects across 4 languages. Good candidates for testing:

### Python (best for initial testing)
- **TextCNN** — Text classification CNN (moderate complexity)
- **Chat-Application** — Socket-based chat app (multi-module)
- **Weather-Service** — API + data processing (orthogonal concerns)

### JavaScript
- **Calculator** — Simple calculator app (control — should be LEAF)
- **Todo-App** — Full-stack todo (moderate decomposition)

### Java
- **Library-System** — Library management (classic OOP decomposition)

### C/C++
- **Mini-Database** — Database engine (complex, deep decomposition)

## Expected decomposition patterns

| Project | Expected depth | Expected children | Key challenge |
|---------|---------------|-------------------|---------------|
| Calculator | 1 (LEAF) | 0 | Too simple to split |
| TextCNN | 2 | 3-4 | Data pipeline + model + training + eval |
| Chat-Application | 2 | 4-5 | Server + client + protocol + auth + UI |
| Mini-Database | 3 | 5-6 | Parser + storage + query + index + API + tests |
