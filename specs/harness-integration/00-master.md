# Synodic Harness Integration — Master Plan

## What This Is

This is a coordinated upgrade to the Synodic project that embeds a governance control plane (Harness) into the codebase. It touches 4 areas in strict dependency order. Each area has a detailed spec attached.

## Attached Specs

1. `harness-embed-prompt.md` — Create `HARNESS.md` at project root
1. `harness-portability-prompt.md` — Add self-governance (versioning + amendments) and `synodic init` CLI command
1. `factory-v2-prompt.md` — Upgrade `skills/factory/SKILL.md` with governance checkpoints
1. `fractal-v2-prompt.md` — Upgrade `skills/fractal/SKILL.md` with governance checkpoints

## Execution Order

Execute these in strict sequence. Each step depends on the previous.

### Phase 1 — Foundation

Read and execute `harness-embed-prompt.md`:

- Creates `HARNESS.md` at repo root (the governance protocol)
- Creates `.harness/` directory structure
- Updates `AGENTS.md` to reference `HARNESS.md`

### Phase 2 — Self-Governance + CLI

Read and execute `harness-portability-prompt.md`:

- Adds version frontmatter and amendment process (§10, §11) to `HARNESS.md`
- Creates helper scripts in `.harness/scripts/`
- Implements `synodic init` command in `packages/cli`
- Creates `specs/harness-spec/` stub
- Migrates governance artifacts to `.harness/` directory

### Phase 3 — Factory Upgrade

Read and execute `factory-v2-prompt.md`:

- Adds Step 2.5 STATIC GATE to Factory skill
- Adds classified rework items to INSPECT
- Adds GovernanceLog persistence
- Adds governance reference header pointing to `HARNESS.md`

### Phase 4 — Fractal Upgrade

Read and execute `fractal-v2-prompt.md`:

- Adds DECOMPOSE GATE, SOLVE GATE, REUNIFY REWORK to Fractal skill
- Adds classified feedback with category tags
- Adds GovernanceLog persistence
- Adds governance reference header pointing to `HARNESS.md`
- Reuses static checking logic from Factory (shared `.harness/rules/`)

## Ground Rules

- Read each attached spec fully before starting implementation. Do not skim.
- Commit after each phase with message format: `harness: phase N — <description>`.
- All changes are additive. Existing functionality must not break.
- If a spec says “future” or “TODO”, create the hook/stub but do not implement.
- When the spec offers an optional choice (e.g., rename `.factory/` to `.harness/`), prefer the recommended option.
- HARNESS.md should be ≤300 lines. Terse, imperative, agent-parseable.
- Run existing tests after each phase to verify nothing is broken.

## Verification

After all 4 phases, verify:

1. `HARNESS.md` exists at root with version frontmatter and all 11 sections
1. `AGENTS.md` references `HARNESS.md`
1. `.harness/rules/`, `.harness/scripts/`, `.harness/templates/` exist
1. `.harness/scripts/static_gate.sh`, `decompose_gate.py`, `aggregate_governance.py` are functional
1. `skills/factory/SKILL.md` has STATIC GATE (Step 2.5), classified INSPECT items, GovernanceLog (Step 7b)
1. `skills/fractal/SKILL.md` has DECOMPOSE GATE (Step 2.5), SOLVE GATE (Step 3.5), REUNIFY REWORK (Step 4.5), GovernanceLog (Step 5b)
1. Both skills reference `HARNESS.md` in their headers with checkpoint maps
1. `synodic init` command exists in `packages/cli` and scaffolds correctly
1. `specs/harness-spec/` directory exists with README stub
1. `.gitignore` tracks governance logs but ignores per-run manifests