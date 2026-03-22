---
status: planned
created: 2026-03-22
priority: high
tags:
- harness
- context-mesh
- coordination
- dag
- substrate
parent: '058'
created_at: 2026-03-22T21:03:06.121406604Z
updated_at: 2026-03-22T21:03:06.121406604Z
---
# Context Mesh: DAG-Based Knowledge Substrate for Agent Coordination

## Overview

In traditional orchestration, information flows through an orchestrator hub (lossy, bottlenecked). Context Mesh eliminates the hub — it's a **routerless, managerless global data bus** where all design constraints, research findings, and intermediate artifacts exist as nodes in a DAG. Agents read and write directly to the mesh. The harness guards the DAG's structural integrity.

This is the "omniscient nervous system" of the AI Harness — the foundation layer that all pipelines operate on.

**AI-native property exploited:** Zero context transfer cost. When Agent B needs Agent A's findings, it reads a node — no summarization loss, no meeting overhead, no context window pollution from relay.

## Design

### DAG structure

The mesh is a directed acyclic graph stored on disk:

```
.harness/mesh/
├── mesh.json                 # DAG adjacency + node metadata index
├── nodes/
│   ├── {hash}.md             # Node content (constraint, finding, artifact)
│   └── ...
└── markers/                  # Stigmergic markers (cross-ref with 060)
```

Each node has: `id`, `type` (constraint | finding | artifact | decision), `created_by` (pipeline + step), `depends_on` (edges), `tags`, `content_hash`.

### Node types

| Type | What it captures | Written by | Example |
|------|-----------------|------------|---------|
| `constraint` | Design requirement or invariant | Spec, human, or crystallized rule | "API must be backward-compatible" |
| `finding` | Research result or discovered fact | Swarm branch, fractal decompose | "Library X doesn't support async" |
| `artifact` | Produced code, design, or analysis | Factory BUILD, fractal SOLVE | "auth module at commit abc123" |
| `decision` | Architectural choice with rationale | Route step, human | "Chose strategy-2 because..." |

### Gap detection (harness responsibility)

The harness periodically scans the DAG for structural gaps:

- **Dangling dependencies**: Node A depends on B, but B doesn't exist → spawn agent to fill B
- **Stale nodes**: Artifact node older than its constraint → flag for re-evaluation
- **Orphan clusters**: Subgraph disconnected from any active pipeline → surface for human review
- **Coverage gaps**: Spec requirements not traced to any artifact node → flag as unimplemented

Gap detection is deterministic (graph traversal), not LLM-based. When a gap is found, the harness can auto-spawn a pipeline to fill it — a factory run for a missing artifact, a swarm run for an unexplored constraint.

### Conflict resolution

When two agents write conflicting nodes (e.g., two decisions about the same architectural choice):

1. **Last-write-wins** for artifacts (git handles this via branches)
2. **Flag-and-surface** for decisions and constraints — human resolves
3. **Merge** for findings — both can coexist as evidence

### How pipelines interact with the mesh

- **Read**: Any agent step can query the mesh for relevant context (injected into prompt via `context: mesh.query("tag:auth AND type:constraint")`)
- **Write**: Agent output schemas include optional `mesh_nodes` field — the harness extracts and persists them
- **Subscribe**: Stigmergic watchers (060) can trigger on mesh node creation/update

### Relationship to CLAUDE.md / AGENTS.md

The mesh is the structured, queryable version of what AGENTS.md tries to be. Where AGENTS.md is a flat file that rots, the mesh is a living DAG that evolves with the project. OpenAI's harness engineering insight: "If information isn't discoverable to the agent, it's illegible." The mesh makes ALL project knowledge discoverable.

## Plan

- [ ] Define mesh.json schema (DAG adjacency list, node metadata)
- [ ] Define node types and required fields
- [ ] Implement mesh read/write commands (`synodic mesh add`, `synodic mesh query`)
- [ ] Implement gap detection algorithm (dangling deps, stale nodes, coverage gaps)
- [ ] Implement mesh context injection for agent steps (YAML `context: mesh.query(...)`)
- [ ] Implement auto-spawn on gap detection
- [ ] Integration: factory pipeline writes artifact nodes after BUILD
- [ ] Integration: swarm pipeline writes finding/decision nodes after merge

## Test

- [ ] DAG operations: add node, add edge, query by type/tag
- [ ] Gap detection: dangling dep detected, stale node flagged
- [ ] Conflict resolution: two conflicting decisions flagged for human review
- [ ] Pipeline integration: factory BUILD output creates artifact node in mesh
- [ ] Context injection: agent prompt includes relevant mesh nodes

## Notes

### Not a vector database

The mesh is a typed DAG with explicit edges, not an embedding-based retrieval system. Retrieval is structural (graph traversal + tag filtering), not semantic similarity. This is intentional — structural queries are deterministic and auditable. Semantic search can be added later as an optional query mode.

### Scope: project-level, not global

Each project has its own mesh in `.harness/mesh/`. Cross-project mesh sharing is out of scope for now.

### Logical Correctness Evaluation (2026-03-22)

**Issues found:**

1. **"Routerless, managerless" contradicted by design**: The overview claims a "routerless, managerless global data bus." The design then describes the harness guarding DAG integrity, periodically scanning for gaps, and auto-spawning agents. The harness IS the manager — the claim is misleading.

2. **Circular dependency in auto-spawn**: Gap detection auto-spawns pipelines, but pipeline spawning requires the pipeline engine (058). The mesh is Layer 1 infrastructure that pipelines run ON, yet auto-spawn depends on the pipeline engine (Layer 2). Layer 1 depends on Layer 2 which depends on Layer 1.

3. **"Zero context transfer cost" is incorrect**: Reading mesh nodes into an agent's context window consumes tokens proportional to node content. The cost is not zero — it's shifted from inter-agent relay to mesh reads. Correct claim: "zero summarization loss," not "zero cost."

4. **mesh.json concurrency hazard**: Single JSON file for the DAG adjacency list. Multiple parallel agents (enabled by 058's `parallel` step type) writing simultaneously will cause file-level conflicts. Conflict resolution section addresses semantic conflicts but ignores file-level contention on the index file.

5. **Missing test for auto-spawn**: Auto-spawn on gap detection is a key feature with no corresponding test case.
