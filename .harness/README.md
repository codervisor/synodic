# .harness/

Governance infrastructure for the Synodic Harness protocol. See [HARNESS.md](../HARNESS.md) for the full specification.

## Contents

| Directory/File | Purpose |
|----------------|---------|
| `rules/` | Crystallized static rules loaded by Layer 1 checks in all topologies |
| `templates/` | Decomposition templates (future) |
| `scripts/` | Helper scripts for governance checkpoints |
| `*.governance.jsonl` | Append-only governance logs per topology (cross-run learning) |

## Governance Log Schemas

### ci-fix.governance.jsonl

Records CI auto-fix attempts from the `.github/workflows/ci-fix.yml` feedback loop.

```json
{
  "work_id": "ci-fix-{pr-number}-{attempt}",
  "source": "ci-fix",
  "timestamp": "ISO 8601",
  "pr_number": 123,
  "branch": "factory/factory-1710600000",
  "attempt": 1,
  "max_attempts": 3,
  "ci_failure_summary": "cargo test failed: 2 test failures in src/lib.rs",
  "fix_applied": true,
  "fix_commit": "abc1234",
  "fix_description": "Fixed off-by-one in boundary check",
  "ci_passed_after_fix": true,
  "status": "fixed|exhausted|no-fix-found"
}
```
