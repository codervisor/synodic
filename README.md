# Synodic — AI Agent Governance via Hooks

> *synodic* (adj.) — from Greek *synodos*, "meeting, conjunction." The period when orbiting bodies align into the same configuration.

**Synodic** enforces governance rules on AI coding agent sessions through standard hook mechanisms — git hooks for deterministic checks, Claude Code hooks for real-time tool call interception.

## Why Synodic?

AI coding agents can write to files, run commands, and push code. Synodic ensures they can't:
- Force-push or `git reset --hard`
- Leak secrets in tool arguments
- Write to system directories (`/etc`, `/usr`)
- `rm -rf /` or `rm -rf ~`

No databases, no log files, no custom event stores — just hooks.

## Quick Start

```bash
# Build
cd rust && cargo build --release

# Initialize governance in your project
synodic init
```

`synodic init` configures:
- **L1**: `git config core.hooksPath .githooks` (deterministic: fmt, clippy, test)
- **L2**: `.claude/settings.json` with `PreToolUse` → `synodic intercept` (pattern-based blocking)

## Two-Layer Governance

| Layer | Mechanism | What it checks | When |
|-------|-----------|----------------|------|
| **L1** | `pre-commit` git hook | `cargo fmt --check` | Every commit |
| **L1** | `pre-push` git hook | fmt + clippy + test | Every push |
| **L1** | GitHub Actions CI | Same checks, enforced | Every PR |
| **L2** | `PreToolUse` Claude hook | 5 default interception rules | Every tool call |

### Default interception rules (L2)

| Rule | Blocks | Tools |
|------|--------|-------|
| `destructive-git` | `git reset --hard`, `git push --force`, `git clean -fd` | Bash |
| `secrets-in-args` | API keys, passwords, tokens in arguments | All |
| `writes-outside-project` | Writes to `/etc/**` | Write, Edit |
| `writes-to-system` | Writes to `/usr/**` | Write, Edit |
| `dangerous-rm` | `rm -rf /`, `rm -rf ~` | Bash |

## Project Structure

```
synodic/
├── rust/
│   ├── harness-core/          # L2 interception engine
│   └── harness-cli/           # CLI: init + intercept
├── .githooks/                 # L1: git hooks (fmt, clippy, test)
│   ├── pre-commit
│   └── pre-push
├── .claude/                   # L2: Claude Code hooks
│   ├── settings.json          # PreToolUse → intercept.sh
│   └── hooks/
│       └── intercept.sh       # stdin JSON → synodic intercept → exit 0/2
├── skills/                    # Claude Code skills
├── docs-site/                 # Docusaurus documentation
└── specs/                     # LeanSpec specs
```

## CLI

```bash
synodic init                    # Setup L1 git hooks + L2 Claude Code hooks
synodic intercept --tool <name> --input '<json>'  # Evaluate tool call against rules
```

## Documentation

| Document | Description |
|----------|-------------|
| [CLAUDE.md](./CLAUDE.md) | Claude Code project instructions |
| [docs-site/](./docs-site/) | Full documentation site |

### Related repositories

- **[codervisor/eval](https://github.com/codervisor/eval)** — Standalone eval framework (SWE-bench, FeatureBench, DevBench)
- **[codervisor/orchestra](https://github.com/codervisor/orchestra)** — Pipeline engine, coordination patterns

## License

MIT
