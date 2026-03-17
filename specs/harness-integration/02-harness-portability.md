# HARNESS.md Self-Governance + `synodic init` Scaffolding

## Context

We’ve created `HARNESS.md` as the governance protocol for Synodic. Two follow-up requirements:

1. **HARNESS.md must be a living document** with its own versioning, amendment process, and change governance — it should evolve as crystallization reveals new patterns.
1. **Other projects should be able to bootstrap Harness governance quickly** without copying files manually. The Synodic CLI (`packages/cli`) should provide a scaffolding command.

This prompt implements both.

-----

## Part 1: HARNESS.md Self-Governance

### 1a. Add Version Header

Add a YAML frontmatter block to the top of `HARNESS.md`:

```yaml
---
version: 0.1.0
last_amended: 2026-03-17
changelog:
  - "0.1.0: Initial governance protocol"
---
```

Version follows semver:

- **major**: breaking change (removed/redefined category, changed checkpoint protocol signature, removed a required section)
- **minor**: additive change (new category, new section, new compliance checklist item)
- **patch**: clarification, typo, wording improvement (no semantic change)

### 1b. Add §10 — Amending This Document

Add a new section at the end of HARNESS.md:

```markdown
## §10 — Amending This Document

This document is versioned (see frontmatter). It evolves through two mechanisms:

### Crystallization-driven amendments

When `.harness/*.governance.jsonl` (or `.factory/governance.jsonl` / `.fractal/governance.jsonl`)
accumulates feedback that cannot be classified under the existing §5 taxonomy,
a new category MAY be proposed:

1. Open an issue titled `harness: propose category [{new-category}]` with:
   - ≥3 governance log entries that demonstrate the gap
   - Proposed category name and definition
   - Which checkpoint type it applies to (code-producing / decomposition / integration)
2. PR the taxonomy change to HARNESS.md with version bump (minor).
3. PR MUST be human-reviewed. Agents MUST NOT auto-merge HARNESS.md changes.

### Protocol-level amendments

Changes to §2 (evaluation model), §3 (checkpoint protocol), or §4 (placement rules)
are breaking changes (major version bump). These require:

1. A spec in `specs/` describing the rationale and migration path.
2. All existing skills must be updated to comply before merge.
3. Explicit human approval.

### Non-breaking changes

Clarifications, examples, and wording improvements (patch) can be merged via
normal PR process but MUST still bump the version.
```

### 1c. Add §11 — Template and Portability

```markdown
## §11 — Template and Portability

This document is designed to be portable across projects. The Synodic CLI provides
`synodic init` to scaffold a Harness-governed project (see CLI documentation).

When adopting HARNESS.md in a new project:

1. Copy this file to the project root.
2. Run `synodic init` to generate the directory structure and helper scripts.
3. Customize §5 taxonomy if the project's domain requires additional categories.
4. Existing categories MUST NOT be redefined — only extended.
5. Update `AGENTS.md` (or equivalent) to reference HARNESS.md.

The governance protocol (§2–§4) is project-agnostic. The taxonomy (§5) has a
universal base that projects extend. The persistence model (§6) and crystallization
process (§7) are implementation-specific and may be adapted.
```

-----

## Part 2: `synodic init` Command

### Overview

Add a `synodic init` subcommand to `packages/cli` that scaffolds Harness governance infrastructure in any project directory. Think of it as `git init` but for agent governance.

### Usage

```bash
# Initialize in current directory
synodic init

# Initialize in a specific directory
synodic init /path/to/project

# Initialize with options
synodic init --topology factory,fractal --rules-dir .harness/rules
```

### What `synodic init` Does

**Step 1 — Create directory structure:**

```
.harness/
├── rules/                       # Empty — will be populated by crystallization
│   └── .gitkeep
├── templates/                   # Decomposition templates (future)
│   └── .gitkeep
└── README.md                    # Brief explanation of this directory
```

**Step 2 — Copy HARNESS.md template:**

Copy the HARNESS.md template to the project root. The template is bundled with the CLI (source of truth: `packages/cli/templates/HARNESS.md`).

If HARNESS.md already exists, do NOT overwrite. Print a message:

```
HARNESS.md already exists. Skipping. Use --force to overwrite.
```

**Step 3 — Create governance log files:**

Based on `--topology` flag (default: all known topologies):

```
.harness/factory.governance.jsonl    # if factory topology enabled
.harness/fractal.governance.jsonl    # if fractal topology enabled
```

Each file is created empty (0 bytes) with a comment header isn’t possible in JSONL, so just create the empty file.

**Step 4 — Create helper scripts:**

```
.harness/scripts/
├── static_gate.sh               # Detect languages in diff, run appropriate checkers
├── decompose_gate.py            # Orthogonality/coverage check for decomposition
└── aggregate_governance.py      # Aggregate governance logs for crystallization analysis
```

These are starter implementations:

**`static_gate.sh`** — detects which languages were modified in a git diff, runs the appropriate checkers (cargo check, tsc, eslint, ruff, clippy), and outputs a JSON report of failures. Exit code 0 = all pass, non-zero = failures found.

**`decompose_gate.py`** — takes child scopes as JSON input, computes keyword overlap (Jaccard similarity) between each pair, checks coverage against parent spec terms, and outputs flags as JSON. Pure function, no LLM calls.

**`aggregate_governance.py`** — reads `*.governance.jsonl` files, aggregates feedback items by category, computes frequency counts, and outputs a summary report. Used for manual crystallization analysis. Reports patterns that appear in ≥3 runs as “crystallization candidates.”

**Step 5 — Update .gitignore:**

Append the following to `.gitignore` (if not already present):

```gitignore
# Harness — per-run manifests are local, governance logs are tracked
.factory/*/
.fractal/*/

# Do NOT ignore:
# .harness/           (governance infrastructure)
# .harness/rules/     (crystallized rules)
# .harness/*.governance.jsonl  (learning logs)
```

**Step 6 — Update AGENTS.md (if exists):**

If `AGENTS.md` exists in the project root, append the governance reference block:

```markdown
## Governance

All agent operations in this repository are subject to the governance protocol
defined in [HARNESS.md](./HARNESS.md). Before executing any skill, read HARNESS.md
to understand checkpoint requirements, feedback classification, and escalation rules.
```

If AGENTS.md doesn’t exist, skip this step (don’t create it — the project may use CLAUDE.md or another convention).

**Step 7 — Print summary:**

```
✓ Synodic Harness initialized

  Created:
    HARNESS.md              — Governance protocol (edit §5 to customize taxonomy)
    .harness/rules/         — Static rules directory (populated by crystallization)
    .harness/scripts/       — Helper scripts for governance checkpoints
    .harness/*.governance.jsonl — Governance learning logs

  Next steps:
    1. Read HARNESS.md and customize §5 taxonomy if needed
    2. Add governance checkpoints to your skills (see §9 compliance checklist)
    3. Run your skills — governance logs accumulate automatically
    4. After 10+ runs: python .harness/scripts/aggregate_governance.py
```

### CLI Implementation Notes

- The CLI is in `packages/cli` (TypeScript).
- Templates should live in `packages/cli/templates/`:
  - `HARNESS.md` — the canonical template (derived from the project root HARNESS.md but with placeholder sections for project-specific customization)
  - `static_gate.sh`
  - `decompose_gate.py`
  - `aggregate_governance.py`
  - `harness-readme.md` — the .harness/README.md content
- The `init` command should be idempotent — running it twice should not duplicate content or overwrite existing files (unless `--force`).
- The `--topology` flag accepts a comma-separated list. Known values: `factory`, `fractal`. Unknown values are accepted (creates the governance.jsonl file but no topology-specific scripts).
- The `--rules-dir` flag allows customizing the rules directory path (default: `.harness/rules`).

### Template Relationship

The HARNESS.md in the project root of Synodic is the **canonical source**. The template bundled with the CLI is derived from it but may lag behind. Add a version check:

```
# In synodic init output, if the template version < project HARNESS.md version:
⚠ Template HARNESS.md is v0.1.0 but latest canonical version is v0.2.0.
  Run `synodic update-harness` to pull the latest template.
```

The `update-harness` command is a future addition — just document it as a TODO in the CLI for now.

-----

## Part 3: HARNESS.md as a Publishable Spec

Long-term, HARNESS.md should be publishable as a standalone specification that other agent orchestration projects can adopt, even without using Synodic’s CLI or runtime. To support this:

### Add a `specs/harness-spec/` directory

```
specs/harness-spec/
├── README.md                    # The Harness Governance Specification v0.1
├── schema/
│   ├── governance-event.json    # JSON Schema for governance log entries
│   ├── manifest.json            # JSON Schema for run manifests
│   └── rule.json                # JSON Schema for crystallized rules
└── examples/
    ├── factory-run.jsonl         # Example governance log from a factory run
    └── crystallized-rule.json    # Example of a crystallized rule
```

This makes the spec machine-readable and testable. Other projects can validate their governance log format against the schema without adopting Synodic’s runtime.

Don’t implement this fully now — just create the directory structure and README with a note that schemas will be added as the format stabilizes after real-world usage.

-----

## Execution Order

1. Add §10 and §11 to existing HARNESS.md (self-governance + portability)
1. Add version frontmatter to HARNESS.md
1. Create `.harness/` directory structure with helper scripts
1. Implement `synodic init` command in `packages/cli`
1. Create `specs/harness-spec/` stub directory
1. Migrate governance artifacts from `.factory/` to `.harness/` (the recommended rename from the previous prompt)

## Acceptance Criteria

- [ ] HARNESS.md has version frontmatter with semver
- [ ] §10 (Amending This Document) exists with crystallization-driven and protocol-level amendment processes
- [ ] §11 (Template and Portability) exists
- [ ] Agent auto-merge of HARNESS.md changes is explicitly forbidden in §10
- [ ] `.harness/` directory structure created with rules/, templates/, scripts/
- [ ] `static_gate.sh` detects languages and runs appropriate checkers
- [ ] `decompose_gate.py` computes orthogonality/coverage flags
- [ ] `aggregate_governance.py` reports crystallization candidates
- [ ] `synodic init` command scaffolds all of the above in any project directory
- [ ] `synodic init` is idempotent (safe to run twice)
- [ ] `specs/harness-spec/` directory created with README stub
- [ ] .gitignore updated to track governance logs but ignore per-run manifests