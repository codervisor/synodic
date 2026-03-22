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
# AI Harness: Intelligent Manufacturing Network for Autonomous Agent Coordination

## Overview

The factory skill (044) encodes its pipeline as SKILL.md — markdown instructions the LLM interprets unreliably. Industry consensus confirms: **the agent isn't the hard part — the harness is**.

This spec defines the **AI Harness** — not just a pipeline engine, but a full intelligent manufacturing substrate. Five AI-native coordination patterns operate on three architectural layers:

**Layer 1 — Coordination Substrates** (always-on infrastructure):
- **Context Mesh** (059): DAG-based knowledge graph — the omniscient nervous system. No routing, no central manager. Harness guards the DAG, detects gaps, spawns agents to fill them.
- **Stigmergic Coordination** (060): Artifact-driven event bus — the automatic conveyor belt. Agents react to environment changes (file writes, markers, labels), not direct messages. Harness provides debounce and marker TTL.

**Layer 2 — Pipeline Engine** (deterministic orchestration):
- 7 step types (`agent`, `gate`, `shell`, `watch`, `route`, `loop`, `parallel`)
- Declarative YAML pipelines, composable middleware, configurable gates
- Provider abstraction (claude-cli, agent-sdk, custom)

**Layer 3 — Skill Topologies** (production patterns):

| Phase | Pattern | Pipeline | Role |
|-------|---------|----------|------|
| Design | **Speculative Swarm** | swarm.yml | Divergent exploration on Mesh knowledge base |
| Design | **Fractal Decomposition** | fractal.yml | Orthogonal decomposition with scope isolation |
| Production | **Stigmergic Flow** | (event-driven) | Artifact-driven handoffs between stages |
| Production | **Generative-Adversarial** | adversarial.yml | Quality control at each node |
| Production | **Factory** | factory.yml | Linear BUILD → INSPECT → PR |

The five patterns form a closed loop: Context Mesh provides global state → Swarm explores strategies → Fractal decomposes complexity → Stigmergic flow drives production → Adversarial hardens quality → results feed back into the Mesh.

## Design

### Step types

Seven primitives compose to express all four skill topologies:

| Type | What it does | Used by |
|------|-------------|---------|
| `agent` | Invoke `claude -p` with constrained tools and structured output | All (BUILD, INSPECT, DECOMPOSE, SOLVE, GENERATE, ATTACK, MERGE) |
| `gate` | Run commands, check exit codes, collect failures | All (static checks, CI checks) |
| `shell` | Run a command for side effects | All (git push, gh pr create) |
| `watch` | Poll a condition until pass/timeout | Factory (CI monitor) |
| `route` | Branch on structured output (approve/rework/escalate) | Factory, Fractal |
| `loop` | Repeat a step sequence until termination condition | Adversarial (gen→attack→check) |
| `parallel` | Fan out over a dynamic set, collect results | Fractal (solve leaves), Swarm (branches) |

### Middleware (composable, wraps any step)

- `retry(n)` — retry on failure, up to N times
- `timeout(ms)` — kill step after duration
- `log(path)` — record outcome to governance JSONL
- `manifest()` — update manifest after step
- `on_fail(action)` — route: rework, escalate, skip, or invoke a named step

### Gate definitions (project-level)

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

Gates are shared across all pipelines. `match` filters by changed files.

### Provider abstraction

```yaml
# .harness/config.yml
provider:
  type: claude-cli          # claude-cli | agent-sdk | custom
  model: claude-sonnet-4-6
```

Agent steps invoke `claude -p` by default. Swap provider without changing pipelines.

---

### Pipeline 1: Factory

Linear pipeline with code review and CI monitoring.

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
    run: [static, ci]
    retry: 2
    on_fail: rework(build)

  - name: inspect
    type: agent
    prompt: skills/factory/prompts/inspect.md
    tools: [Read, Glob, Grep]           # read-only, enforced
    max_turns: 20
    output_schema: schemas/inspect-verdict.json

  - name: route
    type: route
    input: inspect.verdict
    approve: create-pr
    rework: build
    max_iterations: 3
    exhaust: escalate

  - name: create-pr
    type: shell
    command: gh pr create --title "factory: ${spec.title}" --body "${manifest.summary}"

  - name: ci-monitor
    type: watch
    command: gh pr checks ${pr.number} --watch --fail-fast
    timeout: 900000
    on_fail:
      step: ci-fix
      max_attempts: 2

  - name: ci-fix
    type: agent
    prompt: skills/factory/prompts/ci-fix.md
    tools: [Read, Edit, Write, Bash, Glob, Grep]
    session: build.session_id           # reuse BUILD context
    max_turns: 30
```

### Pipeline 2: Fractal

Recursive tree with algorithmic spine and parallel leaf solving.

```yaml
# .harness/pipelines/fractal.yml
name: fractal
config:
  max_depth: 3
  max_children: 5
  max_total_nodes: 20

steps:
  - name: complexity-check
    type: shell
    command: synodic fractal complexity --input ${spec.path}
    output_schema: schemas/complexity-score.json
    # If below threshold, skip decompose entirely

  - name: decompose
    type: loop
    until: all_nodes_are_leaves
    max_iterations: ${config.max_depth}
    steps:
      - name: split
        type: parallel
        over: pending_nodes                 # dynamic set from manifest
        step:
          type: agent
          prompt: skills/fractal/prompts/decompose.md
          tools: [Read, Glob, Grep]
          output_schema: schemas/decompose-verdict.json

      - name: decompose-gate
        type: shell
        command: synodic fractal gate --input ${manifest.tree}
        output_schema: schemas/gate-result.json
        on_fail: rework(split, 1)           # one retry per level

  - name: schedule
    type: shell
    command: synodic fractal schedule --input ${manifest.path}
    output_schema: schemas/solve-waves.json

  - name: solve
    type: parallel
    over: solve_waves                       # waves execute sequentially
    wave_mode: sequential                   # leaves within wave are parallel
    step:
      type: agent
      prompt: skills/fractal/prompts/solve.md
      tools: [Read, Edit, Write, Bash, Glob, Grep]
      isolation: worktree
      output_schema: schemas/solve-report.json

  - name: solve-gates
    type: gate
    run: [static]
    retry: 1                                # 1 retry per leaf (trees compound)
    on_fail: rework(solve)

  - name: reunify
    type: loop
    until: root_merged
    direction: bottom-up
    steps:
      - name: merge-attempt
        type: shell
        command: synodic fractal reunify --input ${node.children}
        output_schema: schemas/reunify-result.json

      - name: ai-resolve
        type: agent
        condition: merge-attempt.needs_ai    # only if algorithmic merge fails
        prompt: skills/fractal/prompts/reunify.md
        tools: [Read, Edit, Write, Bash]
        isolation: worktree
        output_schema: schemas/reunify-report.json

  - name: prune
    type: shell
    command: synodic fractal prune --input ${manifest.tree}

  - name: final-gates
    type: gate
    run: [static, ci]
    retry: 2
    on_fail: rework(reunify)

  - name: create-pr
    type: shell
    command: gh pr create --title "fractal: ${spec.title}" --body "${manifest.summary}"
```

Key: the `parallel` step fans out over a dynamic set. The `loop` step with `direction: bottom-up` walks the tree. The `condition` field on `ai-resolve` means the step only runs when the algorithmic merge can't resolve conflicts — no wasted LLM calls.

### Pipeline 3: Swarm

Fan-out exploration with cross-pollination, convergence pruning, and fragment fusion.

```yaml
# .harness/pipelines/swarm.yml
name: swarm
config:
  max_forks: 4
  convergence_threshold: 0.85
  merge_strategy: fragment-fusion

steps:
  - name: strategize
    type: agent
    prompt: skills/swarm/prompts/strategize.md
    tools: [Read, Glob, Grep]
    output_schema: schemas/strategy-set.json

  - name: explore
    type: loop
    until: converged_or_exhausted
    max_iterations: 3                       # checkpoint rounds
    steps:
      - name: solve-branches
        type: parallel
        over: active_branches               # dynamic: pruned branches removed
        step:
          type: agent
          prompt: skills/swarm/prompts/branch-solve.md
          tools: [Read, Edit, Write, Bash, Glob, Grep]
          isolation: worktree
          output_schema: schemas/branch-report.json

      - name: checkpoint
        type: shell
        command: synodic swarm checkpoint --manifest ${manifest.path}
        output_schema: schemas/checkpoint-result.json
        # Computes: Jaccard similarity, cross-pollination context

      - name: prune
        type: shell
        command: synodic swarm prune --manifest ${manifest.path} --threshold ${config.convergence_threshold}
        output_schema: schemas/prune-result.json
        # Removes convergent branches, enforces min 2 survivors

  - name: merge
    type: agent
    prompt: skills/swarm/prompts/merge.md
    tools: [Read, Edit, Write, Bash, Glob, Grep]
    isolation: worktree
    output_schema: schemas/merge-report.json

  - name: merge-gates
    type: gate
    run: [static, ci]
    retry: 1
    on_fail: rework(merge)

  - name: create-pr
    type: shell
    command: gh pr create --title "swarm: ${spec.title}" --body "${manifest.summary}"
```

Key: the `loop` over explore rounds handles checkpoint/prune cycles. `parallel` over `active_branches` dynamically shrinks as branches are pruned. The algorithmic steps (`synodic swarm checkpoint`, `synodic swarm prune`) are `shell` steps — deterministic, zero LLM cost.

### Pipeline 4: Adversarial

Escalating generator-critic loop with termination detection.

```yaml
# .harness/pipelines/adversarial.yml
name: adversarial
config:
  max_rounds: 5
  escalation: progressive
  consecutive_clean_to_terminate: 2
  critic_modes: [syntax-and-types, edge-cases, concurrency-safety, adversarial-inputs, semantic-analysis]

steps:
  - name: adversarial-loop
    type: loop
    until: terminated
    max_iterations: ${config.max_rounds}
    termination:
      consecutive_clean: ${config.consecutive_clean_to_terminate}
      plateau_rounds: 3                     # stop if issue count not decreasing
    steps:
      - name: generate
        type: agent
        prompt: skills/adversarial/prompts/generate.md
        tools: [Read, Edit, Write, Bash, Glob, Grep]
        isolation: worktree
        output_schema: schemas/generate-report.json

      - name: static-gate
        type: gate
        run: [static]
        retry: 2
        on_fail: rework(generate)

      - name: attack
        type: agent
        prompt: skills/adversarial/prompts/attack.md
        tools: [Read, Glob, Grep]           # read-only critic, enforced
        max_turns: 20
        output_schema: schemas/attack-report.json
        context:
          critic_mode: ${loop.iteration | index_into(config.critic_modes)}
          escalation: ${config.escalation}

  - name: final-gates
    type: gate
    run: [static, ci]
    retry: 2
    on_fail: escalate

  - name: create-pr
    type: shell
    command: gh pr create --title "adversarial: ${spec.title}" --body "${manifest.summary}"
    condition: terminated_clean             # only PR if clean termination
```

Key: the `loop` has built-in termination conditions (consecutive clean, plateau detection, round cap). The `context` field on `attack` injects the current critic mode based on loop iteration index — the escalation ladder is data, not code. The critic gets `tools: [Read, Glob, Grep]` — enforced read-only, not honor-system.

---

### How the runtime resolves cross-cutting concerns

All four pipelines share infrastructure the runtime provides automatically:

| Concern | Runtime responsibility | Pipeline author responsibility |
|---------|----------------------|-------------------------------|
| Manifest lifecycle | Create, update after each step, finalize | Define output schemas |
| Governance logging | Append to `.harness/{skill}.governance.jsonl` | — (automatic) |
| Gate execution | Read `gates.yml`, match files, run commands | Declare which gate groups to run |
| PR creation | Variable interpolation (`${spec.title}`, etc.) | Provide shell command template |
| Worktree management | Create/cleanup worktrees for isolated steps | Declare `isolation: worktree` |
| Session management | Track session IDs across agent steps | Declare `session: {step}.session_id` |
| Cost control | Enforce `max_turns` per agent step | Set limits |

### Algorithmic commands needed

The fractal and swarm pipelines require deterministic CLI commands:

| Command | Purpose | Exists? |
|---------|---------|---------|
| `synodic fractal complexity` | Complexity scoring | Designed in SKILL.md, not implemented |
| `synodic fractal gate` | TF-IDF orthogonality, cycle detection | Designed, not implemented |
| `synodic fractal schedule` | DAG-based wave scheduling | Designed, not implemented |
| `synodic fractal reunify` | Git merge-tree reunification | Designed, not implemented |
| `synodic fractal prune` | Set cover redundancy detection | Designed, not implemented |
| `synodic swarm checkpoint` | Jaccard similarity, cross-pollination | New |
| `synodic swarm prune` | Convergence detection, branch pruning | New |

These are all JSON-in/JSON-out CLI commands — pure algorithms, no LLM.

## Plan

- [ ] Define pipeline YAML schema (all 7 step types + middleware + termination)
- [ ] Define gate YAML schema (match patterns, commands)
- [ ] Implement harness runtime: YAML parser + step executor
- [ ] Implement `agent` step: `claude -p` subprocess with structured output + tool constraints
- [ ] Implement `gate` step: file-match filtering, command execution, failure collection
- [ ] Implement `shell` step: command execution with variable interpolation
- [ ] Implement `watch` step: polling loop with timeout
- [ ] Implement `route` step: verdict-based branching with iteration cap
- [ ] Implement `loop` step: iteration with termination conditions (clean count, plateau, max)
- [ ] Implement `parallel` step: fan-out over dynamic set, collect results
- [ ] Implement middleware: retry, timeout, governance logging, manifest
- [ ] Implement provider abstraction (claude-cli default, agent-sdk upgrade path)
- [ ] Create all four pipeline YAMLs + prompt templates + output schemas
- [ ] Create default gate definitions for Synodic project
- [ ] Implement `synodic swarm checkpoint` and `synodic swarm prune` commands
- [ ] Migrate all four SKILL.md files to thin shims
- [ ] Integration tests per pipeline

## Test

- [ ] Pipeline YAML parsing for all 7 step types
- [ ] Gate execution: file-match filtering works correctly
- [ ] `loop` step: termination conditions (consecutive clean, plateau, max iterations)
- [ ] `parallel` step: fan-out over dynamic set, results collected correctly
- [ ] `route` step: approve/rework/exhaust branches
- [ ] `watch` step: timeout triggers on_fail
- [ ] Agent tool constraints enforced (read-only critic can't edit)
- [ ] Session continuity: ci-fix reuses build session
- [ ] Conditional steps: `condition` field skips step when false
- [ ] Provider swap: claude-cli → agent-sdk transparent
- [ ] End-to-end: each pipeline on a trivial spec

## Notes

### Why 7 step types, not fewer?

Could reduce to 3 (agent, shell, control-flow) but the semantic types matter for:
- **Static validation**: a `gate` that doesn't reference `gates.yml` is an error
- **Automatic middleware**: gates auto-log to governance; shells don't
- **Documentation**: pipeline YAML reads like a spec, not like code

### Why YAML, not code?

- Modifiable without recompilation
- Readable by agents (an agent can reason about pipeline structure)
- Statically validatable (schema check before execution)
- Industry pattern (GitHub Actions, Dagger, CI systems)

### Relationship to existing specs

- **Supersedes 057** (CI feedback loop): CI monitoring is a step in factory pipeline
- **Evolves 044** (factory MVP): Same pipeline, deterministic orchestration
- **Enables 049** (factory test harness): Code pipelines are testable
- **Enables 052** (fractal + factory composition): Both are pipelines sharing gates
- **Enables 056** (harness bug fixes): Configurable gates replace brittle static_gate.sh

### Relationship to SKILL.md files

SKILL.md files become thin shims after migration:
```
/factory run <spec> → synodic harness run --pipeline factory --spec <spec>
/fractal decompose <spec> → synodic harness run --pipeline fractal --spec <spec>
/swarm run <spec> → synodic harness run --pipeline swarm --spec <spec>
/adversarial run <spec> → synodic harness run --pipeline adversarial --spec <spec>
```

The prompt templates (BUILD_PROMPT, INSPECT_PROMPT, etc.) move to `skills/{name}/prompts/*.md`. The orchestration logic is in the pipeline YAML. The algorithmic spine (`synodic fractal gate`, etc.) stays as Rust CLI commands.

### Three-layer architecture

The harness is NOT just a pipeline engine. Three layers:

1. **Substrates** (Context Mesh + Stigmergic): Always-on infrastructure that all pipelines share. The Mesh is the knowledge DAG; stigmergy is the event bus.
2. **Pipeline Engine**: Deterministic YAML-driven orchestration (this spec's core, sections above).
3. **Skill Topologies**: The five patterns (factory, fractal, swarm, adversarial + stigmergic flow) that compose on the engine.

Child specs:
- **059**: Context Mesh — DAG storage, gap detection, spawn triggers, conflict resolution
- **060**: Stigmergic Coordination — watchers, pheromone markers, debounce, TTL, cascade control
