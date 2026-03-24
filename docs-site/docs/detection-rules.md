---
sidebar_position: 7
---

# Detection Rules

Synodic uses a two-layer governance model for detecting issues in AI agent sessions.

## Layer 1 — Static rules

L1 rules are fast, deterministic pattern matchers that run at zero AI cost. They use regex patterns to detect known issues.

### Built-in rules

| Rule | Detects | Type | Severity |
|------|---------|------|----------|
| `secret-in-output` | API keys, passwords, tokens in output | `compliance_violation` | Critical |
| `rm-rf-dangerous` | `rm -rf /`, `rm -rf ~`, `rm -rf $HOME` | `compliance_violation` | Critical |
| `force-push` | `git push --force` to main/master | `compliance_violation` | High |
| `nonexistent-file-ref` | "No such file or directory", ENOENT, FileNotFoundError | `hallucination` | Medium |
| `tool-error` | Generic error/failed/exception patterns | `tool_call_error` | Medium |

### L1 evaluator

The `L1Evaluator` in `harness-core` provides a high-level API for running L1 rules:

```rust
use harness_core::l1::L1Evaluator;

let mut eval = L1Evaluator::new("my-source");

// Evaluate arbitrary content
let result = eval.evaluate("some agent output");
assert!(result.passed);

// Evaluate a diff (only checks added lines)
let result = eval.evaluate_diff(&diff_content);
if !result.passed {
    for finding in &result.matches {
        println!("[{}] {}: {}", finding.severity, finding.rule_name, finding.matched_text);
    }
}
```

### Custom rules

Rules are defined as structs with a regex pattern:

```rust
use harness_core::rules::Rule;
use harness_core::events::{EventType, Severity};

Rule {
    name: "no-todos".to_string(),
    description: "Detects TODO comments left in code".to_string(),
    pattern: r"TODO|FIXME|HACK".to_string(),
    event_type: EventType::Misalignment,
    severity: Severity::Low,
    enabled: true,
}
```

### Rule crystallization

When a pattern is detected 3+ times, Synodic promotes it to a candidate L1 rule:

1. **Detection** — pattern appears repeatedly across sessions
2. **Candidacy** — meets promotion threshold (default: 3 occurrences)
3. **Backtest** — validate against historical logs
4. **Promotion** — becomes an enforceable L1 rule

Access promotion candidates via:
```rust
let candidates = eval.engine().promotion_candidates();
```

## Layer 2 — AI judge

L2 uses an independent LLM to perform semantic analysis of agent output. It evaluates against five dimensions:

1. **Completeness** — does the change address a coherent goal?
2. **Correctness** — logic errors, bugs, null handling?
3. **Security** — injection, hardcoded secrets, unsafe operations?
4. **Conformance** — does the approach fit the codebase?
5. **Quality** — clean, maintainable, no dead code?

The AI judge sees only the diff — never the agent's reasoning — ensuring an independent review.

### Verdicts

- **APPROVE** — changes pass review
- **REWORK** — specific issues must be addressed (triggers rework loop)

### Rework loop

When L1 or L2 rejects changes, Synodic generates structured feedback and re-runs the agent (up to `max_rework` times). If the limit is reached, the issue escalates to human review.

```bash
synodic harness run --max-rework 3 -- claude "implement feature X"
```

## Managing rules

```bash
# List all active rules
synodic rules list

# Test a rule against a log file
synodic rules test secret-in-output --against session.jsonl

# Add a custom rule pattern
synodic rules add "(?i)drop\s+table"
```
