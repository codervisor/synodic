# Eval: FeatureBench — MLflow Tracing System (Fractal Decomposition)

## Source

- **Benchmark**: [FeatureBench](https://huggingface.co/datasets/LiberCoders/FeatureBench) (ICLR 2026)
- **Instance**: `mlflow__mlflow.93dab383.test_trace.17fde8b0.lv1`
- **Repo**: mlflow/mlflow
- **Difficulty**: 15 files changed, 42k char problem statement
- **SOTA resolve rate**: 11% (Claude Opus 4.5 via Claude Code)

## Why this task tests fractal decomposition

This task requires implementing a **comprehensive distributed tracing system** with 6 distinct subsystems that span 15 files. A monolithic approach fails because:
1. Cross-file dependency resolution is the #1 failure mode (FeatureBench finding)
2. The subsystems have clear orthogonal boundaries but shared interfaces
3. Each subsystem is independently testable but must integrate cleanly

Fractal decomposition should split this into orthogonal sub-specs, solve each independently, then reunify with interface alignment.

## Prompt

```
You have the fractal decomposition skill loaded.

/fractal decompose "Implement a comprehensive distributed tracing system for MLflow with:

1. **Span Management** (hierarchical execution spans with lifecycle control,
   attributes, inputs/outputs, events, status tracking)

2. **Trace Data Handling** (store and retrieve trace information with
   serialization, deserialization, cross-system compatibility)

3. **Assessment Integration** (evaluation source tracking — human, LLM,
   code-based — with validation and deprecation handling)

4. **Memory Management** (in-memory trace registry with thread-safe operations,
   caching, timeout-based cleanup)

5. **Fluent API** (decorator-based and context manager interfaces for
   instrumenting functions and code blocks)

6. **OpenTelemetry Integration** (compatibility with OTel standards while
   extending for ML-specific use cases)

Key constraints:
- Thread-safe concurrent access across multiple execution contexts
- Efficient serialization of complex ML data structures and nested span hierarchies
- Proper parent-child relationship management in distributed tracing
- Graceful error handling with fallback to no-op implementations
- Memory optimization with automatic cleanup
- Backward compatibility with schema evolution

Config: output_mode=design, max_depth=3, max_children=6"

Follow the full orchestration protocol from SKILL.md.
```

## Expected decomposition tree

```
root: MLflow Tracing System
├── 1-span-management/
│   ├── 1-span-lifecycle/       (create, start, end, status transitions)
│   └── 2-span-data/            (attributes, inputs/outputs, events)
├── 2-trace-data/
│   ├── 1-serialization/        (to/from JSON, protobuf, dict)
│   └── 2-storage/              (in-memory store, retrieval, indexing)
├── 3-assessment/               (LEAF — focused enough)
├── 4-memory-management/
│   ├── 1-registry/             (thread-safe registry, caching)
│   └── 2-cleanup/              (timeout-based, GC)
├── 5-fluent-api/               (LEAF — decorator + context manager)
└── 6-otel-integration/         (LEAF — adapter layer)
```

## Grading markers

```json
{
  "benchmark": "featurebench",
  "instance_id": "mlflow__mlflow.93dab383.test_trace.17fde8b0.lv1",
  "markers": {
    "orthogonal_split": {
      "check": "6 subsystems identified as top-level children with non-overlapping scopes",
      "required": true
    },
    "recursive_depth": {
      "check": "At least 2 children further decomposed (span-management and memory-management are natural candidates)",
      "required": true
    },
    "interface_contracts": {
      "check": "Each sub-spec defines input/output contracts — especially the Span type shared between span-management, trace-data, and fluent-api",
      "required": true
    },
    "thread_safety_isolation": {
      "check": "Thread-safety concern is scoped to memory-management, not spread across all children",
      "required": true
    },
    "reunification_integration": {
      "check": "Reunification step identifies shared types (Span, Trace, TraceStatus) and resolves interface alignment",
      "required": true
    },
    "cross_cutting_handling": {
      "check": "Error handling and backward compatibility handled as cross-cutting concerns during reunification, not duplicated per child",
      "required": false
    }
  },
  "pass_threshold": "all required markers present"
}
```

## Anti-signal

- Agent writes all 15 files top-to-bottom (no decomposition)
- Thread-safety duplicated in every sub-spec instead of scoped
- Shared types (Span, Trace) defined differently in each child (interface mismatch)
- No reunification — children's outputs just concatenated
- Decomposition by file rather than by concern (anti-pattern)
