---
status: planned
created: 2026-03-22
priority: critical
tags:
- harness
- orchestration
- factory
- architecture
- rust
created_at: 2026-03-22T16:42:58.873730006Z
updated_at: 2026-03-22T16:42:58.873730006Z
---

# Code-Based Harness: Move Factory Orchestration from SKILL.md to Rust CLI

## Overview

The factory skill (044) encodes its entire BUILD → INSPECT pipeline as a 340-line SKILL.md — markdown instructions that Claude interprets at runtime. This is fundamentally unreliable: retry logic is a prompt the LLM may miscount, tool constraints are honor-system, structured output is free-text parsing, and state management is delegated to the LLM.

Industry consensus (OpenAI harness engineering, OpenDev arXiv paper, Elastic/Dagger/Nx self-healing CI) confirms: **the agent isn't the hard part — the harness is**. Five independent teams converged on the same pattern: coding agents become reliable only when deterministic code controls the pipeline and the LLM handles only creative work.

**Current (prompt-as-orchestration):**
SKILL.md says "run cargo check" → LLM decides whether to comply

**Target (code-as-orchestration):**
Rust CLI runs `cargo check` deterministically → LLM implements/reviews code

## Design

### Architecture: Rust CLI + `claude -p` headless

The `synodic` Rust CLI becomes the harness. It calls `claude -p` (headless mode) for creative work (BUILD, INSPECT) and handles everything else in deterministic Rust code. Zero new framework dependencies.

```
synodic factory run <spec-path>
        │
  ┌─────▼──────────────────────────────┐
  │  Rust orchestrator (deterministic)  │
  │                                     │
  │  1. Initialize    — Rust code       │
  │  2. BUILD         — claude -p       │
  │  3. Static Gate   — cargo/clippy    │
  │  4. CI Gate       — cargo test      │
  │  5. INSPECT       — claude -p       │
  │  6. Route         — if/else         │
  │  7. Create PR     — gh pr create    │
  │  8. CI Monitor    — gh pr checks    │
  │  9. CI Fix        — claude -p       │
  │  10. Finalize     — write JSONL     │
  └─────────────────────────────────────┘
```

### Key mechanisms

**Structured output**: `claude -p --output-format json --json-schema '{...}'` returns typed JSON, not free-text `=== BUILD REPORT ===` blocks.

**Tool enforcement**: `--allowedTools "Read,Edit,Write,Bash"` for BUILD; `--allowedTools "Read,Glob,Grep"` for INSPECT. INSPECT literally cannot edit files.

**Session continuity**: `--session-id` for multi-turn. CI fix reuses the BUILD session so the agent retains context about what it built and why.

**Cost control**: `--max-turns 50` for BUILD, `--max-turns 20` for INSPECT.

**Retry budget (code-enforced)**:
- Gate reworks: max 2 (shared between static + CI gate)
- INSPECT loop: max 3 attempts
- CI monitoring fixes: max 2 attempts

### What stays as markdown

BUILD_PROMPT and INSPECT_PROMPT remain as templates (loaded from files). They define WHAT the agent does. The Rust code controls HOW the pipeline flows.

### SKILL.md role after migration

SKILL.md becomes a thin shim that describes the skill for discovery and invokes `synodic factory run`. It no longer encodes pipeline logic.

### Graduation path

If `claude -p` subprocess model hits limits (need hooks, complex session management), graduate to Claude Agent SDK (TypeScript). The pipeline structure stays identical — only the invocation mechanism changes.

## Plan

- [ ] Add `factory` subcommand to `synodic` CLI with `run` action
- [ ] Implement manifest initialization (create `.factory/{id}/`, write manifest.json)
- [ ] Implement BUILD step: call `claude -p` with structured JSON schema for build report
- [ ] Implement Static Gate: detect languages, run cargo check/clippy/tsc/eslint
- [ ] Implement CI Gate: run full test suite (cargo test / npm test) in worktree
- [ ] Implement INSPECT step: call `claude -p` with read-only tools, parse verdict JSON
- [ ] Implement routing logic (approve → PR, rework → loop, exhaust → escalate)
- [ ] Implement PR creation via `gh pr create`
- [ ] Implement CI monitoring: `gh pr checks --watch` with timeout, log extraction
- [ ] Implement CI fix: call `claude -p` with failure context + session continuity
- [ ] Implement governance log write (`.harness/factory.governance.jsonl`)
- [ ] Migrate SKILL.md to thin shim that invokes `synodic factory run`
- [ ] Add integration tests for pipeline happy path and failure modes

## Test

- [ ] Unit tests for manifest creation, gate execution, verdict parsing
- [ ] Integration test: mock `claude -p` responses, verify full pipeline flow
- [ ] Verify tool constraints: INSPECT with write tools must be impossible
- [ ] Verify retry caps: gate reworks stop at 2, INSPECT at 3, CI fixes at 2
- [ ] Verify structured output parsing: malformed JSON handled gracefully
- [ ] End-to-end: run on a trivial spec, verify PR created with CI green

## Notes

### Why Rust CLI, not Agent SDK or Ruflo?

- **Zero new deps**: `synodic` CLI already exists, `claude` CLI already installed
- **Fits architecture**: Synodic is Rust-first; adding TS orchestrator creates split-brain
- **Subprocess is proven**: Elastic's production system uses `claude` as subprocess
- **Graduation path**: If we outgrow `claude -p`, Agent SDK migration is mechanical (same pipeline, different invocation)

### Industry references

- OpenAI harness engineering: AGENTS.md as table of contents, code controls pipeline
- OpenDev (arXiv 2603.05344): six-phase ReAct loop + seven subsystems, all in code
- Elastic: shell script orchestrates `claude` subprocess, 24 PRs fixed, 20 dev-days saved
- Dagger: Go code defines constrained workspace, agent iterates in tight loop
- Nx: `npx nx fix-ci` — code command, not prompt

### Relationship to existing specs

- **Supersedes 057** (auto-close feedback loop): CI monitoring is now a pipeline step, not a separate GitHub Action
- **Implements 044** (factory skill MVP): Same pipeline, deterministic orchestration
- **Enables 049** (factory test harness): Code-based pipeline is testable; SKILL.md is not
- **Enables 052** (fractal + factory composition): Code orchestrator can compose with fractal