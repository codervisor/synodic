# Eval: FeatureBench — Seaborn Regression Plotting (Factory Skill)

## Source

- **Benchmark**: [FeatureBench](https://huggingface.co/datasets/LiberCoders/FeatureBench) (ICLR 2026)
- **Instance**: `mwaskom__seaborn.7001ebe7.test_regression.ce8c62e2.lv1`
- **Repo**: mwaskom/seaborn
- **Difficulty**: 5 files changed, 30k char problem statement
- **SOTA resolve rate**: 11% (Claude Opus 4.5 via Claude Code)

## Why this task tests the factory skill

This is a medium-complexity task (5 files, focused domain) that fits the factory's BUILD → INSPECT pipeline:
- **BUILD**: Implement regression plotting with statistical computation
- **INSPECT**: Verify statistical correctness, API consistency, edge case handling

The task has clear acceptance criteria (test suite) and a bounded scope that a single BUILD agent should handle, with INSPECT catching statistical correctness issues.

## Prompt

```
You have the factory skill loaded.

Create a spec file, then run the factory on it:

## Spec: Seaborn Regression Plotting System

### Overview
Implement statistical regression plotting for seaborn, supporting scatter plots
with fitted regression lines, confidence intervals, multiple model types, and
faceted grid visualizations.

### Plan
- [ ] Implement _RegressionPlotter class with data preprocessing (variable
      extraction, missing data removal, binning for discrete variables)
- [ ] Add regression model fitting: linear, polynomial (order N), logistic,
      robust (using statsmodels RLM), lowess, and log-transformed
- [ ] Implement bootstrap-based confidence interval estimation with
      configurable number of iterations and CI percentage
- [ ] Create regplot() function for single-axes regression scatter plots
- [ ] Create residplot() function for residual diagnostics
- [ ] Create lmplot() function for multi-faceted regression grids using FacetGrid
- [ ] Handle edge cases: singleton inputs, perfect separation in logistic,
      missing data, non-numeric inputs
- [ ] Ensure matplotlib integration: proper axes handling, color cycling,
      marker customization

### Test
- [ ] Linear regression produces correct fit line and CI band
- [ ] Polynomial regression with order=2 fits a parabola
- [ ] Logistic regression handles binary outcomes with sigmoid curve
- [ ] Bootstrap CI width decreases with more iterations
- [ ] Residual plot shows residuals vs fitted values
- [ ] lmplot correctly facets by hue/col/row variables
- [ ] Edge cases: NaN removal, empty data, singleton input

/factory run this-spec
```

## Expected factory behavior

1. **BUILD**: Agent implements regression.py and touches utils.py, axisgrid.py, categorical.py, matrix.py
2. **INSPECT**: Reviews statistical correctness (bootstrap impl, CI calculation), API consistency with seaborn patterns, edge case coverage
3. Likely 1-2 attempts — INSPECT may catch missing edge cases or incorrect bootstrap implementation

## Grading markers

```json
{
  "benchmark": "featurebench",
  "instance_id": "mwaskom__seaborn.7001ebe7.test_regression.ce8c62e2.lv1",
  "markers": {
    "build_produces_code": {
      "check": "BUILD agent creates/modifies files and commits with factory: prefix",
      "required": true
    },
    "inspect_statistical_review": {
      "check": "INSPECT reviews statistical correctness (bootstrap, CI, regression fitting)",
      "required": true
    },
    "inspect_edge_cases": {
      "check": "INSPECT checks for edge case handling (NaN, empty, singleton)",
      "required": true
    },
    "rework_feedback_specific": {
      "check": "If REWORK, items are specific and actionable (not 'improve code quality')",
      "required": true
    },
    "manifest_complete": {
      "check": "manifest.json has attempts array with build/inspect records",
      "required": true
    }
  },
  "pass_threshold": "all required markers present"
}
```

## Anti-signal

- BUILD produces skeleton code with TODO comments
- INSPECT approves without checking statistical correctness
- Rework items are vague ("add more tests" without specifics)
- No manifest tracking
