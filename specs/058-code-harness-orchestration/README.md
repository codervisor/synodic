---
status: planned
created: 2026-03-22
priority: critical
tags:
- harness
- orchestration
- architecture
- runtime
- pipeline
- middleware
created_at: 2026-03-22T16:42:58.873730006Z
updated_at: 2026-03-22T20:41:42.952829462Z
---

# Harness Runtime: Configurable Pipeline Engine with Composable Gates and Middleware

## Overview

The factory skill (044) encodes its entire BUILD → INSPECT pipeline as a 340-line SKILL.md — markdown instructions that Claude interprets at runtime. This is fundamentally unreliable: retry logic is a prompt the LLM may miscount, tool constraints are honor-system, structured output is free-text parsing, and state management is delegated to the LLM.

Industry consensus (OpenAI harness engineering, OpenDev arXiv paper, Elastic/Dagger/Nx self-healing CI, Open SWE middleware pattern) confirms: **the agent isn't the hard part — the harness is**. Deterministic code must control the pipeline while the LLM handles only creative work.

This spec defines a **general-purpose harness runtime** — not a factory-specific orchestrator. Factory is the first pipeline; fractal, swarm, and adversarial define their own pipelines on the same runtime.

## Design

### Core primitives

The harness has four step types and one cross-cutting mechanism:

**Step types:**

| Type | What it does | Example |
|------|-------------|---------|
| `agent` | Invoke `claude -p` with constrained tools and structured output | BUILD, INSPECT, CI fix |
| `gate` | Run commands, check exit codes, collect failures | cargo test, clippy, eslint |
| `shell` | Run a command for side effects | gh pr create, git push |
| `watch` | Poll a condition until pass/timeout | gh pr checks --watch |

**Middleware** (composable, wraps any step):
- `retry(n)` — retry a step up to N times on failure
- `timeout(ms)` — kill step after duration
- `log(path)` — record step outcome to governance JSONL
- `manifest(path)` — update manifest after step completes
- `on_fail(action)` — route failures (rework, escalate, skip)

### Pipeline definitions

Pipelines are declared in config, not hardcoded. Each skill defines its pipeline:

```yaml
# .harness/pipelines/factory.yml
name: factory
steps:
  - name: build
    type: agent
    prompt: skills/factory/prompts/build.md
    tools: [Read, Edit, Write, Bash, Glob, Grep]
    max_turns: 50
    isolation: worktree
    output_schema: schemas/build-report.json

  - name: gates
    type: gate
    run: [static, ci]           # references .harness/gates.yml
    retry: 2                    # max rework cycles (shared across all gates)
    on_fail: rework(build)      # route failures back to build step

  - name: inspect
    type: agent
    prompt: skills/factory/prompts/inspect.md
    tools: [Read, Glob, Grep]   # read-only — enforced, not suggested
    max_turns: 20
    output_schema: schemas/inspect-verdict.json

  - name: route
    type: route
    input: inspect.verdict
    approve: create-pr
    rework: build               # max 3 iterations
    max_iterations: 3
    exhaust: escalate

  - name: create-pr
    type: shell
    command: gh pr create --title "factory: ${spec.title}" --body "${manifest.summary}"

  - name: ci-monitor
    type: watch
    command: gh pr checks ${pr.number} --watch --fail-fast
    timeout: 900000             # 15 min
    on_fail:
      step: ci-fix
      max_attempts: 2

  - name: ci-fix
    type: agent
    prompt: skills/factory/prompts/ci-fix.md
    tools: [Read, Edit, Write, Bash, Glob, Grep]
    session: build.session_id   # reuse BUILD session — full context preserved
    max_turns: 30
```

### Gate definitions (project-level, not skill-level)

Gates are configured per-project, not hardcoded per-language:

```yaml
# .harness/gates.yml
gates:
  static:
    - name: rust-check
      match: "*.rs"
      command: cd cli && cargo check
    - name: rust-lint
      match: "*.rs"
      command: cd cli && cargo clippy -- -D warnings
    - name: ts-typecheck
      match: "*.ts,*.tsx"
      command: npx tsc --noEmit
    - name: ts-lint
      match: "*.ts,*.tsx"
      command: npx eslint .
    - name: python-typecheck
      match: "*.py"
      command: pyright
    - name: custom-rules
      match: "*"
      command: .harness/scripts/run-rules.sh

  ci:
    - name: rust-test
      match: "*.rs"
      command: cd cli && cargo test
    - name: node-test
      match: "*.ts,*.tsx,*.js"
      command: npm test
```

The `match` field filters by changed files. Only relevant gates run. Projects add/remove/modify gates without touching skill code or the harness runtime.

### Harness runtime

The runtime is a Rust binary in the `synodic` CLI:

```
synodic harness run --pipeline factory --spec <spec-path>
```

It reads the pipeline YAML, resolves gate definitions, and executes steps sequentially. Agent steps call `claude -p` as subprocess. Gate steps run commands and collect exit codes. The runtime owns all state (manifest, governance log).

### Provider abstraction

Agent steps invoke `claude -p` by default, but the runtime abstracts the invocation:

```yaml
# .harness/config.yml
provider:
  type: claude-cli          # or: agent-sdk, api, custom
  model: claude-sonnet-4-6
  # type: agent-sdk would use @anthropic-ai/claude-agent-sdk
  # type: custom would invoke a user-defined command
```

This keeps the door open for Agent SDK, other LLMs, or custom providers without changing pipeline definitions.

### How skills use the harness

Each skill defines:
1. **Pipeline YAML** — step sequence, retry policies, routing
2. **Prompt templates** — what the agent does at each step (markdown files)
3. **Output schemas** — JSON schemas for structured agent output

SKILL.md becomes a thin shim:
```
/factory run <spec> → synodic harness run --pipeline factory --spec <spec>
```

### Extensibility for other skills

Fractal pipeline (different steps, same runtime):

```yaml
# .harness/pipelines/fractal.yml
steps:
  - name: decompose
    type: agent
    prompt: skills/fractal/prompts/decompose.md
    tools: [Read, Glob, Grep]
    output_schema: schemas/decomposition.json
  - name: solve-leaves
    type: agent
    prompt: skills/fractal/prompts/solve.md
    tools: [Read, Edit, Write, Bash]
    parallel: true              # run leaf solves concurrently
    isolation: worktree
  - name: reunify
    type: agent
    prompt: skills/fractal/prompts/reunify.md
    tools: [Read, Edit, Write, Bash]
  - name: gates
    type: gate
    run: [static, ci]
    retry: 2
    on_fail: rework(reunify)
```

Same gate definitions, same runtime, different pipeline.

## Plan

- [ ] Define pipeline YAML schema (step types, middleware, routing)
- [ ] Define gate YAML schema (match patterns, commands)
- [ ] Implement harness runtime in `synodic` CLI: YAML parser + step executor
- [ ] Implement `agent` step: `claude -p` subprocess with structured output + tool constraints
- [ ] Implement `gate` step: file-match filtering, command execution, failure collection
- [ ] Implement `shell` step: command execution with variable interpolation
- [ ] Implement `watch` step: polling loop with timeout
- [ ] Implement `route` step: verdict-based branching with iteration cap
- [ ] Implement middleware: retry, timeout, governance logging, manifest updates
- [ ] Implement provider abstraction (claude-cli default, agent-sdk upgrade path)
- [ ] Create factory pipeline YAML + prompt templates + output schemas
- [ ] Create default gate definitions for Synodic project
- [ ] Migrate factory SKILL.md to thin shim invoking `synodic harness run`
- [ ] Integration tests: mock agent responses, verify pipeline execution

## Test

- [ ] Pipeline YAML parsing: valid configs load, invalid configs produce clear errors
- [ ] Gate execution: only matching gates run based on changed files
- [ ] Agent step: tool constraints enforced via --allowedTools
- [ ] Retry caps: gate reworks stop at configured limit
- [ ] Route step: approve/rework/exhaust branches work correctly
- [ ] Watch step: timeout triggers on_fail handler
- [ ] Provider swap: switching claude-cli → agent-sdk doesn't change pipeline behavior
- [ ] End-to-end: factory pipeline on trivial spec produces PR

## Notes

### Why YAML pipelines, not code-defined pipelines?

Code pipelines (Rust functions) are powerful but require recompilation. YAML pipelines:
- Can be modified without rebuilding the binary
- Are readable by agents (an agent can reason about a pipeline definition)
- Can be validated statically (schema check before execution)
- Match industry patterns (GitHub Actions, Dagger, CI systems)

### Why not Ruflo or Open SWE?

- Ruflo: 250K lines, 55 alpha iterations, massive dependency for our needs
- Open SWE: Python-centric, LangChain dependency chain
- Our harness is ~1-2K lines of Rust reading YAML and calling subprocesses
- If we outgrow it, migration to a framework is mechanical

### Relationship to existing specs

- **Supersedes 057** (auto-close feedback loop): CI monitoring is a pipeline step
- **Evolves 044** (factory skill MVP): Same pipeline, deterministic orchestration
- **Enables 049** (factory test harness): Code pipelines are testable
- **Enables 052** (fractal + factory composition): Both are pipelines on the same runtime
- **Enables 056** (harness bug fixes): Gates replace brittle static_gate.sh