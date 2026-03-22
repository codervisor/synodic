---
status: planned
created: 2026-03-22
priority: high
tags:
- harness
- stigmergy
- coordination
- events
- substrate
parent: '058'
created_at: 2026-03-22T21:03:56.723244355Z
updated_at: 2026-03-22T21:03:56.723244355Z
---

# Stigmergic Coordination: Artifact-Driven Event Bus for Agent Handoffs

## Overview

In centralized orchestration, work flows through a dispatcher (single point of failure, coordination overhead scales O(n²)). Stigmergic coordination eliminates the dispatcher — agents coordinate **indirectly through modifications to the shared environment**. Like ant pheromone trails or kanban cards: no scheduler needed, work flows because artifacts change state.

This is the "automatic conveyor belt" of the AI Harness. Code gets written → tester agent smells the change → tests get generated → scanner finds a vulnerability → security agent stamps a `needs-fix` pheromone marker → fixer agent reacts.

**AI-native property exploited:** Millisecond environment perception. An agent can detect file changes, parse marker metadata, and start working in under a second — faster than any message-passing system.

## Design

### Core concepts

| Concept | Definition | Physical analogy |
|---------|-----------|-----------------|
| **Artifact** | Any file or node that agents produce/consume | Workpiece on the line |
| **Marker** | Metadata label attached to an artifact | Kanban card / Andon light |
| **Watcher** | Rule that triggers when a marker or artifact changes | Sensor on the conveyor |
| **Reaction** | Pipeline or step spawned by a watcher | Worker picking up the piece |

### Marker system

Markers are the "pheromones" — lightweight labels with structured metadata:

```yaml
# .harness/markers/{artifact-hash}.yml
markers:
  - type: needs-test
    created_by: factory/build
    created_at: 2026-03-22T10:00:00Z
    ttl: 86400                    # expires in 24h if not consumed
    priority: high
    context:
      files: [src/auth.rs, src/session.rs]
      commit: abc1234
  - type: needs-fix
    created_by: adversarial/attack
    severity: critical
    context:
      issue: "SQL injection in user input handler"
      test_case: "input: '; DROP TABLE users; --"
```

Marker types are extensible per project. Standard types:

| Marker | Meaning | Typical producer | Typical consumer |
|--------|---------|-----------------|-----------------|
| `needs-test` | New code needs test coverage | BUILD, SOLVE | Test generator agent |
| `needs-fix` | Defect found | ATTACK, gate failure | Fixer agent |
| `needs-review` | Ready for human/AI review | Factory completion | INSPECT agent |
| `needs-docs` | Code lacks documentation | Coverage scanner | Doc generator agent |
| `blocked` | Waiting on external dependency | Any agent | Gap detector (mesh) |

### Watcher definitions

```yaml
# .harness/watchers.yml
watchers:
  - name: auto-test
    trigger:
      marker: needs-test
      min_priority: medium
    reaction:
      pipeline: factory
      step: build                 # can target a specific step
      prompt_override: skills/test-gen/prompts/generate-tests.md
    debounce: 30000               # 30s — batch rapid changes

  - name: security-fix
    trigger:
      marker: needs-fix
      filter: severity == "critical"
    reaction:
      pipeline: factory
      spec: auto                  # generate a micro-spec from marker context
    debounce: 60000

  - name: mesh-gap-filler
    trigger:
      source: mesh                # triggers on mesh gap detection (059)
      gap_type: dangling_dep
    reaction:
      pipeline: swarm             # explore the gap via swarm
      spec: auto
    debounce: 300000              # 5 min — gaps are not urgent
```

### Debounce (anti-storm mechanism)

Without debounce, a single commit touching 50 files could trigger 50 watchers simultaneously. The harness enforces:

1. **Time-based debounce**: Watcher won't re-fire within `debounce` ms of last trigger. Batches rapid changes.
2. **Scope-based dedup**: If the same marker type exists on the same artifact, the newer marker replaces the older one (no duplication).
3. **Cascade depth limit**: A reaction can produce artifacts that trigger more watchers. Max cascade depth of 3 prevents infinite chains. At depth 3, markers are created but reactions are suppressed — surfaced for human review.

### Marker TTL (lifecycle management)

Markers expire after their TTL to prevent stale pheromone buildup:

- `needs-test` markers expire after 24h (if no test agent consumed them, the code was likely tested manually)
- `needs-fix` critical markers expire after 7d (escalated to human if not resolved)
- `blocked` markers have no TTL (require explicit resolution)

Expired markers are moved to `.harness/markers/archive/` for governance analysis.

### How it composes with pipelines

Stigmergic coordination operates AROUND and BETWEEN pipeline runs:

- **Within a pipeline**: A `gate` failure can emit a `needs-fix` marker instead of (or in addition to) rework routing
- **Between pipelines**: Factory BUILD completion emits `needs-test` → triggers a test-gen pipeline
- **Cross-skill**: Adversarial ATTACK emits `needs-fix` markers → consumed by factory CI-fix
- **Human-in-the-loop**: Humans can create markers manually (`synodic marker add needs-review --file src/auth.rs`)

### Relationship to Context Mesh (059)

Stigmergy is the event layer; the mesh is the state layer.
- Mesh nodes are durable knowledge (constraints, decisions, artifacts)
- Markers are transient signals (needs-test, needs-fix, blocked)
- Watchers can trigger on BOTH mesh changes and marker changes
- A reaction pipeline writes its results back to the mesh (closing the loop)

## Plan

- [ ] Define marker schema (type, created_by, ttl, priority, context)
- [ ] Define watcher YAML schema (trigger, reaction, debounce)
- [ ] Implement marker CRUD (`synodic marker add/list/resolve/expire`)
- [ ] Implement watcher daemon (filesystem watch + marker polling)
- [ ] Implement debounce engine (time-based + scope dedup + cascade limit)
- [ ] Implement TTL expiration (background sweep or lazy check)
- [ ] Implement reaction spawning (invoke `synodic harness run` on trigger)
- [ ] Implement cascade depth tracking and suppression at depth 3
- [ ] Integration: factory gate failures emit markers
- [ ] Integration: adversarial attack findings emit markers

## Test

- [ ] Marker lifecycle: create → consume → archive
- [ ] TTL expiration: marker expires after configured duration
- [ ] Debounce: rapid file changes batched into single watcher trigger
- [ ] Cascade limit: depth-3 chain suppressed, surfaced for review
- [ ] Scope dedup: duplicate markers on same artifact collapsed
- [ ] Watcher trigger: marker creation fires matching watcher
- [ ] Reaction spawning: watcher triggers correct pipeline

## Notes

### Not a message queue

This is NOT RabbitMQ/Kafka for agents. Markers are environment modifications (files on disk), not messages in a queue. Any agent can scan the marker directory — no subscription needed, no broker dependency. This is what makes it stigmergic: coordination through the environment, not through communication channels.

### The watcher daemon

The watcher is a lightweight process (`synodic watch`) that monitors `.harness/markers/` and artifact directories. It can run as a background process during development or as a CI step. It does NOT need to be always-on — markers persist on disk and are processed whenever the watcher next runs.