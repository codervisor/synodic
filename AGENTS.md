# AI Agent Instructions

## Project: Synodic

AI coding factory — structured BUILD → INSPECT pipelines for spec-driven development.

## Skills

This project uses [lean-spec](https://github.com/codervisor/lean-spec) for spec management:

```bash
npx skills add codervisor/lean-spec@leanspec-sdd -g -y
```

### Local Skills

| Skill | Description | Usage |
|-------|-------------|-------|
| `factory` | Coding factory — transforms a spec into a reviewed PR via BUILD → INSPECT pipeline | `/factory run <spec-path>` |
| `fractal` | Fractal decomposition — recursively splits complex tasks into sub-specs, solves leaves independently, reunifies bottom-up | `/fractal decompose <task-or-spec-path>` |

## Conventions

- **Specs first**: Create a spec before starting non-trivial work
- **LeanSpec format**: All specs use YAML frontmatter (status, created, tags, priority)
- **Governance**: All agent operations follow [HARNESS.md](./HARNESS.md)

## Governance

All agent operations in this repository are subject to the governance protocol
defined in [HARNESS.md](./HARNESS.md). Before executing any skill, read HARNESS.md
to understand checkpoint requirements, feedback classification, and escalation rules.
