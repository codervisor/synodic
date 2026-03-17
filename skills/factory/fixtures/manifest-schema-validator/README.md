---
status: planned
created: 2026-03-16
priority: medium
tags:
- fixture
- tooling
- node
parent: 044-factory-skill-mvp
depends_on: []
---

# Add manifest.json schema validator script

> **Status**: planned · **Priority**: medium · **Created**: 2026-03-16

## Overview

The factory skill produces `.factory/{work-id}/manifest.json` files, and
there is a JSON Schema at `skills/factory/references/manifest.schema.json`.
But nothing actually validates manifests against the schema. A validation
script would catch drift between the Rust `WorkItem` struct and the
skill's schema, and could be used in CI or as a post-factory health check.

## Design

Create a standalone Node.js script at
`scripts/validate-manifest.ts` that:

1. Accepts a path to a manifest.json file as a CLI argument.
2. Reads the manifest and the schema.
3. Validates the manifest against the schema using `ajv` (JSON Schema
   validator, already available via npm).
4. Prints a success message or lists validation errors with paths.
5. Exits 0 on valid, 1 on invalid, 2 on usage error.

The script must work with `pnpm tsx scripts/validate-manifest.ts <path>`.

Additionally, create a test manifest fixture at
`skills/factory/fixtures/manifest-schema-validator/valid-manifest.json`
that conforms to the schema, and an invalid one at
`skills/factory/fixtures/manifest-schema-validator/invalid-manifest.json`
that has deliberate schema violations (missing required fields).

## Plan

- [ ] Install `ajv` as a devDependency: `pnpm add -D ajv`
- [ ] Create `scripts/validate-manifest.ts` with the validation logic
- [ ] Create `skills/factory/fixtures/manifest-schema-validator/valid-manifest.json` (valid fixture)
- [ ] Create `skills/factory/fixtures/manifest-schema-validator/invalid-manifest.json` (invalid fixture — missing `id` and `status` fields)
- [ ] Verify: `pnpm tsx scripts/validate-manifest.ts skills/factory/fixtures/manifest-schema-validator/valid-manifest.json` exits 0
- [ ] Verify: `pnpm tsx scripts/validate-manifest.ts skills/factory/fixtures/manifest-schema-validator/invalid-manifest.json` exits 1
- [ ] Verify: `pnpm tsx scripts/validate-manifest.ts` (no args) exits 2

## Test

- [ ] `pnpm tsx scripts/validate-manifest.ts` with valid fixture exits 0
- [ ] `pnpm tsx scripts/validate-manifest.ts` with invalid fixture exits 1 and prints error details
- [ ] `pnpm tsx scripts/validate-manifest.ts` with no args exits 2 with usage message
