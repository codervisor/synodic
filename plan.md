# Plan: `synodic` CLI — Rust binary + NPM wrapper

## Overview

Wrap the existing bash `harness` script and `evals/` scripts into a unified `synodic` CLI built in Rust, distributed via an NPM package.

```
synodic harness run -- claude "fix the bug"
synodic harness eval
synodic harness log
synodic harness rules
synodic eval run fb:mlflow-tracing --skill fractal
synodic eval score <instance-id>
synodic eval list
```

## Architecture

```
synodic/
├── cli/                        # Rust crate (the binary)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             # Entry point, clap app
│       ├── cmd/
│       │   ├── mod.rs
│       │   ├── harness.rs      # `synodic harness {run,eval,log,rules}`
│       │   └── eval.rs         # `synodic eval {run,score,list}`
│       └── util.rs             # Shared helpers (process exec, paths, output)
├── npm/                        # NPM wrapper package
│   ├── package.json            # "synodic" bin, platform-specific optionalDeps
│   ├── src/
│   │   └── index.ts            # bin stub: resolves + spawns the Rust binary
│   └── scripts/
│       └── postinstall.ts      # Downloads platform binary if optionalDep missed
└── (existing files unchanged)
```

## Step 1: Rust CLI crate (`cli/`)

**Cargo.toml** — Minimal deps:
- `clap` (derive) for arg parsing
- `serde` + `serde_json` for structured output
- `anyhow` for error handling

**Subcommands via clap:**

```rust
#[derive(Parser)]
#[command(name = "synodic", version, about = "Synodic governance CLI")]
enum Cli {
    Harness(HarnessCmd),
    Eval(EvalCmd),
}
```

### `synodic harness` subcommand

Maps 1:1 to the existing `./harness` bash script commands:

| CLI                              | Delegates to                          |
|----------------------------------|---------------------------------------|
| `synodic harness run [opts] -- <cmd>` | Spawns `./harness run [opts] -- <cmd>` |
| `synodic harness eval [opts]`    | Spawns `./harness eval [opts]`        |
| `synodic harness log [--json] [--tail N]` | Spawns `./harness log [opts]` |
| `synodic harness rules`         | Spawns `./harness rules`              |

**Phase 1 (this PR):** Thin wrapper — the Rust binary locates the repo-root `harness` script and delegates via `std::process::Command`. This gives us the CLI structure without rewriting 711 lines of bash.

**Phase 2 (future):** Incrementally port bash logic into native Rust (Layer 1 static checks, manifest management, governance log writes).

### `synodic eval` subcommand

Maps to the existing `evals/` scripts:

| CLI                              | Delegates to                          |
|----------------------------------|---------------------------------------|
| `synodic eval run <alias> [opts]` | Spawns `evals/run.sh <alias> [opts]` |
| `synodic eval score <id> [opts]`  | Spawns `evals/score.sh <id> [opts]`  |
| `synodic eval list`              | Reads `evals/evals.json`, prints table |

`synodic eval list` is the one native command — it parses `evals.json` and renders a formatted table of available benchmark tasks with their tags and aliases.

### Shared utilities (`util.rs`)

- `find_repo_root()` — Walk up from CWD looking for `.harness/` or `.git`
- `exec_script(path, args)` — Spawn child process, inherit stdio, forward exit code
- `JsonOutput` — Optional `--json` flag support for machine-readable output

## Step 2: NPM wrapper package (`npm/`)

**Distribution model:** Follow the pattern used by `@biomejs/biome`, `turbopack`, etc.

```json
{
  "name": "synodic",
  "version": "0.1.0",
  "bin": { "synodic": "bin/synodic" },
  "optionalDependencies": {
    "@synodic/cli-darwin-arm64": "0.1.0",
    "@synodic/cli-darwin-x64": "0.1.0",
    "@synodic/cli-linux-x64": "0.1.0",
    "@synodic/cli-linux-arm64": "0.1.0"
  }
}
```

**`bin/synodic`** (JS stub):
1. Detects platform/arch
2. Resolves the native binary from the matching `@synodic/cli-{platform}-{arch}` package
3. Spawns it with `child_process.execFileSync`, forwarding args and stdio

**For local dev:** `npm/bin/synodic` also checks for `../cli/target/debug/synodic` so `cargo build` is sufficient during development.

## Step 3: Wire up root `package.json`

Update the existing root `package.json` to add workspace config and a convenience bin alias:

```json
{
  "scripts": {
    "build": "cd cli && cargo build --release",
    "dev": "cd cli && cargo build"
  }
}
```

## Files to create/modify

### Create:
1. `cli/Cargo.toml`
2. `cli/src/main.rs`
3. `cli/src/cmd/mod.rs`
4. `cli/src/cmd/harness.rs`
5. `cli/src/cmd/eval.rs`
6. `cli/src/util.rs`
7. `npm/package.json`
8. `npm/bin/synodic` (JS stub)

### Modify:
9. Root `package.json` — add `scripts.build`, `scripts.dev`
10. `.gitignore` — add `cli/target/`, `npm/node_modules/`

## Non-goals (this PR)

- No CI/CD for cross-compilation or NPM publishing (future)
- No rewrite of bash logic into Rust (future — phase 2)
- No platform-specific `@synodic/cli-*` packages yet (just the wrapper + local dev path)
