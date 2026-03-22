# CI Auto-Fix Adoption Guide

Adopt the CI auto-fix feedback loop in your project so that agent-created PRs
that fail CI get automatically fixed.

## Quick Start

1. Copy `.github/workflows/ci-fix.yml` to your repository.
2. Add `ANTHROPIC_API_KEY` to your repo secrets (Settings → Secrets → Actions).
3. Done. CI failures on `factory/*` and `claude/*` branches will auto-fix.

## Configuration

All configuration is via GitHub repo variables (Settings → Variables → Actions):

| Variable | Default | Description |
|----------|---------|-------------|
| `CI_FIX_MAX_ATTEMPTS` | `3` | Max fix attempts before giving up |
| `CI_FIX_BRANCH_PATTERN` | `factory/.*\|claude/.*` | Regex for branches to monitor |
| `CI_FIX_MODEL` | `claude-sonnet-4-6` | Claude model for fixes |

## How It Works

```
PR created → CI runs → CI fails
                          ↓
              ci-fix.yml triggers
                          ↓
              Extract error from CI logs
                          ↓
              Claude Code diagnoses + fixes
                          ↓
              Push fix to same branch
                          ↓
              CI re-runs automatically
                          ↓
         (repeat up to MAX_ATTEMPTS times)
```

## Labels

The workflow uses labels to track state:

- `ci-fix-attempt-1`, `-2`, `-3` — tracks which attempt we're on
- `ci-fix-exhausted` — all attempts failed, needs human intervention

## Prerequisites

- GitHub Actions enabled
- `ANTHROPIC_API_KEY` secret with valid API key
- Claude Code CLI available in the runner, OR use `anthropics/claude-code-action`

## Customizing for Your CI

The workflow extracts errors from GitHub's check run API and workflow logs.
For non-standard CI setups, you may want to adjust the "Extract CI failure logs"
step to parse your specific CI output format.

## Relationship to Local Static Gate

If you use the Synodic factory skill, it already runs a local static gate
(cargo check, clippy, tsc, etc.) before creating the PR. The CI fix loop
handles failures that only appear in remote CI:

- Platform differences (Ubuntu CI vs your dev environment)
- Missing system dependencies
- CI-specific test fixtures
- Integration tests not run locally
