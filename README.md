# Synodic — AI-Native Agent Orchestration

> *synodic* (adj.) — from Greek *synodos*, "meeting, conjunction." The period when orbiting bodies align into the same configuration.

**Synodic** is an AI-native orchestration platform that coordinates heterogeneous AI coding tools — Claude Code, OpenAI Codex CLI, GitHub Copilot CLI, Gemini CLI, OpenCode, and others — as a unified, continuously running agent fleet.

## Why Synodic?

Today's AI coding tools are powerful individually but isolated. Each runs as a standalone CLI with its own lifecycle, communication model, and capabilities. There's no way to:

- **Orchestrate** multiple AI tools working on the same task
- **Route** subtasks to the best-fit tool based on capabilities and cost
- **Coordinate** agents using patterns like speculative swarm, pipeline, or generative-adversarial
- **Persist** fleet state across crashes and restarts
- **Optimize** cost by routing repetitive work to cheaper models

Synodic fills this gap — it's the **Docker Compose for AI agents**.

## Architecture

```
┌─────────────────────────────────────────────────┐
│                    SYNODIC                       │
│         AI-Native Agent Orchestration            │
│                                                  │
│  synodic up  ·  synodic ps  ·  synodic send     │
│                                                  │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │ Claude   │ │ Codex    │ │ Copilot  │  ...    │
│  │ Code     │ │ CLI      │ │ CLI      │         │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘        │
│       │             │             │              │
│  ┌────┴─────────────┴─────────────┴──────┐      │
│  │         Agent Backend Trait            │      │
│  │  spawn / communicate / health / stop   │      │
│  └────┬──────────────────────────────────┘      │
│       │                                          │
│  ┌────┴────────────────────────────────┐        │
│  │          Fleet Supervisor           │        │
│  │  Process mgmt · Health probes       │        │
│  │  Message bus · Task lifecycle        │        │
│  │  SQLite persistence · Recovery       │        │
│  └─────────────────────────────────────┘        │
└─────────────────────────────────────────────────┘
```

## Relationship to ClawDen

[ClawDen](https://github.com/codervisor/clawden) manages **claw runtimes** (OpenClaw, ZeroClaw, PicoClaw, etc.) — installation, configuration, Docker images, and channels.

Synodic orchestrates **AI coding tools** as fleet agents. ClawDen is one agent backend that Synodic can manage, alongside Claude Code, Codex CLI, and others.

## Status

Early stage — spec-driven development. See [`specs/`](./specs/) for the design.

## License

MIT
