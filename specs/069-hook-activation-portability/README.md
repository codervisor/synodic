---
status: draft
created: 2026-03-28
priority: medium
tags:
- hooks
- dx
- portability
- l1
created_at: 2026-03-28T00:00:00Z
updated_at: 2026-03-28T00:00:00Z
---

# Hook Activation Portability

## Overview

Synodic's L1 governance relies on git hooks in `.githooks/`. These hooks are activated by `git config core.hooksPath .githooks`, currently triggered via the `prepare` script in `package.json`. This works — but only if the developer runs `pnpm install` (or `npm install`). This spec evaluates the portability gap and proposes solutions.

## Current Setup

### How it works today

1. `.githooks/pre-commit` — runs `cargo fmt --check` on staged `.rs` files
2. `.githooks/pre-push` — runs `cargo fmt --check`, `cargo clippy --workspace`, `cargo test --workspace` when `.rs` files changed
3. `package.json` has `"prepare": "git config core.hooksPath .githooks"` — auto-runs on `pnpm install`

### How to activate

```bash
# Option A: via pnpm (automatic)
pnpm install

# Option B: manual (one command)
git config core.hooksPath .githooks
```

### How to verify hooks are active

```bash
git config core.hooksPath
# Should print: .githooks
```

### How to test hooks

```bash
# Test pre-commit: stage a .rs file and commit
echo "// test" >> rust/harness-core/src/lib.rs
git add rust/harness-core/src/lib.rs
git commit -m "test"  # Should show "pre-commit: checking cargo fmt..."

# Test pre-push: push any branch with .rs changes
git push  # Should show "pre-push: Rust files changed — running L1 checks..."
```

## Problem

The `prepare` script approach has portability gaps:

| Scenario | Hooks activated? | Why |
|----------|-----------------|-----|
| Developer runs `pnpm install` | Yes | `prepare` script fires |
| Developer runs `npm install` | Yes | `prepare` script fires |
| Developer only works in `rust/` | **No** | Never runs npm/pnpm |
| CI runner | **No** | May not run `pnpm install` before builds |
| AI agent (Claude Code, Copilot) | **No** | Agents don't typically run `pnpm install` on session start |
| Fresh clone, Rust-only contributor | **No** | No reason to run npm commands |

Synodic is primarily a Rust project. Requiring an npm command to activate git hooks is a leaky abstraction — it ties a git feature to an unrelated package manager.

## Design Options

### Option A: Document and accept (minimal)

Keep the current setup. Document `git config core.hooksPath .githooks` as the manual alternative. Accept that some workflows won't have hooks and rely on CI as the enforced gate.

**Pros:** No new code, CI is the real gate anyway
**Cons:** Hooks are advisory-only and easy to miss

### Option B: Add a Makefile/Justfile target

```makefile
.PHONY: setup
setup:
	git config core.hooksPath .githooks

.PHONY: build
build: setup
	cd rust && cargo build
```

**Pros:** Works for Rust-only developers, language-agnostic
**Cons:** Another tool to remember, doesn't auto-run

### Option C: Cargo build script (xtask pattern)

Add a workspace `xtask` or `build.rs` that runs `git config core.hooksPath .githooks` on first build.

```rust
// rust/build.rs or rust/xtask/src/main.rs
fn main() {
    // Only run once — check if already configured
    let output = std::process::Command::new("git")
        .args(["config", "core.hooksPath"])
        .output();
    if let Ok(o) = output {
        if String::from_utf8_lossy(&o.stdout).trim() == ".githooks" {
            return;
        }
    }
    let _ = std::process::Command::new("git")
        .args(["config", "core.hooksPath", ".githooks"])
        .status();
}
```

**Pros:** Activates for anyone who runs `cargo build` — the most universal action in this repo
**Cons:** Build scripts running git config is surprising; may confuse contributors

### Option D: `.gitconfig` include (git-native)

Add a `.gitconfig` file at repo root and instruct contributors to include it:

```ini
# .gitconfig
[core]
    hooksPath = .githooks
```

Contributors run once: `git config --local include.path ../.gitconfig`

**Pros:** Pure git, no scripts
**Cons:** Still requires a manual step

### Option E: Claude Code SessionStart hook

For AI agent sessions (the primary use case for governance), add a SessionStart hook:

```json
{
  "hooks": {
    "SessionStart": [{
      "command": "git config core.hooksPath .githooks"
    }]
  }
}
```

**Pros:** Covers the AI agent case directly
**Cons:** Only covers Claude Code, not other agents or human devs

## Recommendation

**Combine Option A + C + E:**

1. **Option C** (cargo build script) as the primary activation — anyone building Rust gets hooks automatically
2. **Option E** (SessionStart hook) for AI agent sessions
3. **Option A** (documentation) as fallback with clear instructions
4. **CI as the enforced gate** regardless — hooks are convenience, CI is authority

This covers all scenarios:
- Rust developers: hooks activate on first `cargo build`
- Node developers: hooks activate on `pnpm install` (existing)
- AI agents: hooks activate on session start
- Everyone else: CI catches what hooks miss

## L1 Evaluation Results

Tested 2026-03-28 after activating hooks:

| Hook | Trigger | Result |
|------|---------|--------|
| pre-commit | `git commit` with staged `.rs` files | `cargo fmt --check` ran, PASS |
| pre-push | `git push` with `.rs` changes | fmt PASS, clippy PASS, test (215 tests) PASS |
| pre-commit | `git commit` with no `.rs` files | Skipped (correct) |

**L1 is fully functional when hooks are activated.** The gap is activation, not the hooks themselves.

## Success Criteria

- [ ] Hooks activate automatically for `cargo build` (Option C)
- [ ] SessionStart hook configured for AI agents (Option E)
- [ ] Setup instructions documented in CONTRIBUTING.md or README
- [ ] CI enforces the same checks as hooks (existing GitHub Actions)
- [ ] `synodic doctor` or similar command can verify hook activation

## Non-Goals

- Replacing Husky for projects that already use it
- Making hooks impossible to bypass (that's CI's job)
- Supporting non-git version control
